use std::{
    str,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use actix_service::{Service, Transform};
use actix_web::{
    dev::{Body, ServiceRequest, ServiceResponse},
    http::{
        header::{HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE, LOCATION, PRAGMA},
        Method,
    },
    web, Error, HttpRequest, HttpResponse, ResponseError,
};
use bitcoin::{util::psbt::serialize::Deserialize, Transaction};
use bitcoincash_addr::HashType;
use bytes::BytesMut;
use futures::{
    future::{err, ok, Either, Future, FutureResult},
    stream::Stream,
    Poll,
};
use prost::Message;
use serde_derive::Deserialize;
use url::Url;

use crate::{
    bitcoin::*,
    crypto::{AddressCodec, CashAddrCodec},
    models::*,
    SETTINGS,
};

use super::errors::*;

use crate::crypto::token::*;

const PAYMENT_URL: &str = "/payments";
const VALID_DURATION: u64 = 30;

#[derive(Deserialize)]
pub struct TokenQuery {
    code: String,
}

///Payment handler
pub fn payment_handler(
    req: HttpRequest,
    payload: web::Payload,
    data: web::Data<(BitcoinClient, WalletState)>,
) -> Box<Future<Item = HttpResponse, Error = ServerError>> {
    // Check headers
    let headers = req.headers();
    if headers.get(ACCEPT) != Some(&HeaderValue::from_str("application/bitcoin-payment").unwrap()) {
        return Box::new(err(PaymentError::Accept.into()));
    }
    if headers.get(CONTENT_TYPE)
        != Some(&HeaderValue::from_str("application/bitcoin-paymentack").unwrap())
    {
        return Box::new(err(PaymentError::Content.into()));
    }

    // Read and parse payment proto
    let body_raw =
        payload
            .map_err(|_| PaymentError::Payload)
            .fold(BytesMut::new(), move |mut body, chunk| {
                body.extend_from_slice(&chunk);
                Ok::<_, PaymentError>(body)
            });
    let payment = body_raw
        .and_then(|payment_raw| Payment::decode(payment_raw).map_err(|_| PaymentError::Decode));

    let payment_ack = payment
        .and_then(move |payment| {
            // Parse tx
            let tx_raw = match payment.transactions.get(0) {
                Some(some) => some,
                None => return Either::A(err(PaymentError::NoTx)),
            }; // Assume first tx
            let tx = match Transaction::deserialize(tx_raw) {
                Ok(ok) => ok,
                Err(e) => return Either::A(err(PaymentError::from(e))),
            };

            // Check outputs
            let wallet_data = &data.1;
            if !wallet_data.check_outputs(tx) {
                return Either::A(err(PaymentError::InvalidOutputs));
            }

            // Send tx
            let bitcoin_client = &data.0;
            Either::B(
                bitcoin_client
                    .send_tx(tx_raw)
                    .and_then(|_txid| {
                        // Create payment ack
                        let memo = Some("Thanks for your custom!".to_string());
                        Ok(PaymentAck { payment, memo })
                    })
                    .map_err(|_| PaymentError::InvalidTx),
            )
        })
        .map_err(ServerError::Payment);

    let response = payment_ack.and_then(|ack| {
        // Encode payment ack
        let mut raw_ack = Vec::with_capacity(ack.encoded_len());
        ack.encode(&mut raw_ack).unwrap();

        // Get merchant data
        let merchant_data = ack
            .payment
            .merchant_data
            .ok_or(PaymentError::NoMerchantDat)?;

        // Generate token
        let url_safe_config = base64::Config::new(base64::CharacterSet::UrlSafe, false);
        let token = base64::encode_config(
            &generate_token(&merchant_data, SETTINGS.secret.as_bytes()),
            url_safe_config,
        );

        // Generate paymentredirect
        let mut redirect_url = Url::parse(
            str::from_utf8(&merchant_data).map_err(|_| PaymentError::InvalidMerchantDat)?,
        )
        .map_err(|_| PaymentError::InvalidMerchantDat)?;
        redirect_url.set_query(Some(&format!("code={}", token)));

        // Generate response
        Ok(HttpResponse::Found()
            .header(LOCATION, redirect_url.into_string())
            .header(AUTHORIZATION, format!("POP {}", token))
            .header(PRAGMA, "no-cache")
            .body(raw_ack))
    });

    Box::new(response)
}

/*
Payment middleware
*/
pub struct CheckPayment(BitcoinClient, WalletState);

impl CheckPayment {
    pub fn new(client: BitcoinClient, wallet_state: WalletState) -> Self {
        CheckPayment(client, wallet_state)
    }
}

impl<S> Transform<S> for CheckPayment
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<Body>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<Body>;
    type Error = Error;
    type InitError = ();
    type Transform = CheckPaymentMiddleware<S>;
    type Future = FutureResult<Self::Transform, Self::InitError>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CheckPaymentMiddleware {
            service,
            client: self.0.clone(),
            wallet_state: self.1.clone(),
        })
    }
}
pub struct CheckPaymentMiddleware<S> {
    service: S,
    client: BitcoinClient,
    wallet_state: WalletState,
}

impl<S> Service for CheckPaymentMiddleware<S>
where
    S: Service<Request = ServiceRequest, Response = ServiceResponse<Body>, Error = Error>,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<Body>;
    type Error = Error;
    type Future = Box<dyn Future<Item = Self::Response, Error = Self::Error>>;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        self.service.poll_ready()
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        // Only pay for put
        match *req.method() {
            Method::PUT => (),
            _ => return Box::new(self.service.call(req)),
        }

        // Grab token query from authorization header then query string
        let token_str: String = match req.headers().get(AUTHORIZATION) {
            Some(some) => match some.to_str() {
                Ok(auth_str) => {
                    if auth_str.len() >= 4 && &auth_str[0..4] == "POP " {
                        auth_str[4..].to_string()
                    } else {
                        return Box::new(err(
                            ServerError::Payment(PaymentError::InvalidAuth).into()
                        ));
                    }
                }
                Err(_) => {
                    return Box::new(err(ServerError::Payment(PaymentError::InvalidAuth).into()))
                }
            },
            None => match web::Query::<TokenQuery>::from_query(req.query_string()) {
                Ok(query) => query.code.clone(), // TODO: Remove this copy
                // Found no token
                Err(_) => {
                    // Valid interval
                    let current_time = SystemTime::now();
                    let expiry_time = current_time + Duration::from_secs(VALID_DURATION);

                    // Get new addr and add to wallet
                    let wallet_state_inner = self.wallet_state.clone();
                    let new_addr =
                        self.client
                            .get_new_addr()
                            .then(move |addr_opt| match addr_opt {
                                Ok(addr_str) => {
                                    let addr = CashAddrCodec::decode(&addr_str)
                                        .map_err(ServerError::Address)?;
                                    let network: Network = addr.network.clone().into();
                                    if network != SETTINGS.network
                                        || addr.hash_type != HashType::Key
                                    {
                                        return Err(ServerError::Payment(
                                            PaymentError::MismatchedNetwork,
                                        ))?; // TODO: Finer grained error here
                                    }
                                    let addr_raw = addr.into_body();
                                    wallet_state_inner.add(addr_raw.clone());
                                    Ok(addr_raw)
                                }
                                Err(_e) => {
                                    Err(ServerError::Payment(PaymentError::AddrFetchFailed).into())
                                }
                            });

                    // Generate URI
                    let uri = req.uri();
                    let url = match (uri.scheme_str(), uri.authority_part()) {
                        (Some(scheme), Some(authority)) => {
                            format!("{}://{}{}", scheme, authority, uri.path())
                        }
                        (None, None) => format!("http://{}{}", SETTINGS.bind, uri.path()),
                        (_, _) => {
                            return Box::new(err(
                                ServerError::Payment(PaymentError::URIMalformed).into()
                            ))
                        }
                    };

                    let response = new_addr.and_then(move |addr_raw| {
                        // Generate outputs
                        let outputs = generate_outputs(addr_raw);

                        // Collect payment details
                        let payment_details = PaymentDetails {
                            network: Some(SETTINGS.network.to_string()),
                            time: current_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                            expires: Some(
                                expiry_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                            ),
                            memo: None,
                            merchant_data: Some(url.as_bytes().to_vec()),
                            outputs,
                            payment_url: Some(PAYMENT_URL.to_string()),
                        };
                        let mut serialized_payment_details =
                            Vec::with_capacity(payment_details.encoded_len());
                        payment_details
                            .encode(&mut serialized_payment_details)
                            .unwrap();

                        // Generate payment invoice
                        let pki_type = Some("none".to_string());
                        let payment_invoice = PaymentRequest {
                            pki_type,
                            pki_data: None,
                            payment_details_version: Some(1),
                            serialized_payment_details,
                            signature: None,
                        };
                        let mut payment_invoice_raw =
                            Vec::with_capacity(payment_invoice.encoded_len());
                        payment_invoice.encode(&mut payment_invoice_raw).unwrap();

                        HttpResponse::PaymentRequired().body(payment_invoice_raw)
                    });

                    // Respond
                    return Box::new(response.map(|resp| req.into_response(resp)));
                }
            },
        };

        // Decode token
        let url_safe_config = base64::Config::new(base64::CharacterSet::UrlSafe, false);
        let token = match base64::decode_config(&token_str, url_safe_config) {
            Ok(some) => some,
            Err(_) => {
                return Box::new(ok(req.into_response(
                    ServerError::Payment(PaymentError::InvalidAuth).error_response(),
                )))
            }
        };

        // Validate
        let uri = req.uri();
        let url = match (uri.scheme_str(), uri.authority_part()) {
            (Some(scheme), Some(authority)) => format!("{}://{}{}", scheme, authority, uri.path()),
            (None, None) => format!("http://{}{}", SETTINGS.bind, uri.path()),
            (_, _) => {
                return Box::new(ok(req.into_response(
                    ServerError::Payment(PaymentError::URIMalformed).error_response(),
                )))
            }
        };
        if !validate_token(url.as_bytes(), SETTINGS.secret.as_bytes(), &token) {
            Box::new(ok(req.into_response(
                ServerError::Payment(PaymentError::InvalidAuth).error_response(),
            )))
        } else {
            Box::new(self.service.call(req))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use actix_web::{http::StatusCode, test, web, App};
    use bigdecimal::BigDecimal;
    use bitcoincash_addr::HashType;
    use serde_json::json;

    use crate::{
        bitcoin::PRICE,
        crypto::Base58Codec,
        db::KeyDB,
        models::PaymentRequest,
        net::{jsonrpc_client::JsonClient, tests::generate_address_metadata, *},
    };

    use super::*;

    #[derive(Deserialize)]
    struct Outpoint {
        pub txid: String,
        pub vout: u32,
        pub amount: BigDecimal,
        pub address: String,
    }

    #[derive(Deserialize)]
    struct SignedHex {
        pub hex: String,
    }

    fn generate_raw_tx(recv_addr: Vec<u8>, _data: Vec<u8>) -> Vec<u8> {
        let client = JsonClient::new(
            format!(
                "http://{}:{}",
                SETTINGS.node_ip.clone(),
                SETTINGS.node_rpc_port
            ),
            SETTINGS.node_username.clone(),
            SETTINGS.node_password.clone(),
        );

        // Get unspent output
        let unspent_req = client.build_request("listunspent".to_string(), vec![]);
        let utxos = client
            .send_request(&unspent_req)
            .and_then(|resp| resp.into_result::<Vec<Outpoint>>());
        let utxos = actix_web::test::block_on(utxos).unwrap();
        let utxo = utxos.get(0).unwrap();

        let inputs_json = json!(
            [{
                "txid": utxo.txid,
                "vout": utxo.vout
            }]
        );

        let bitcoin_amount = BigDecimal::from(PRICE) / 100_000_000;
        let fee = BigDecimal::from(1) / 100_000_000;
        let change = &utxo.amount - &bitcoin_amount - fee;
        let outputs_json = json!([
            {
                Base58Codec::encode(&recv_addr, HashType::Key, SETTINGS.network.clone().into())
                    .unwrap(): bitcoin_amount
            },
            { &utxo.address: change } // { "data": hex::encode(data[1..]) } // TODO: Add op_return data
        ]);

        // Get raw transaction
        let raw_tx_req = client.build_request(
            "createrawtransaction".to_string(),
            vec![inputs_json, outputs_json],
        );
        let unsigned_raw_tx = client
            .send_request(&raw_tx_req)
            .and_then(|resp| resp.into_result::<String>());
        let unsigned_raw_tx = actix_web::test::block_on(unsigned_raw_tx).unwrap();

        // Sign raw transaction
        let signed_raw_req = client.build_request(
            "signrawtransactionwithwallet".to_string(),
            vec![json!(unsigned_raw_tx)],
        );
        let signed_raw_tx = client
            .send_request(&signed_raw_req)
            .and_then(|resp| resp.into_result::<SignedHex>())
            .map(|signed_hex| signed_hex.hex);
        let signed_raw_tx = actix_web::test::block_on(signed_raw_tx).unwrap();
        hex::decode(signed_raw_tx).unwrap()
    }

    #[test]
    fn test_put_no_token() {
        // Init db
        let key_db = KeyDB::try_new("./test_db/no_token").unwrap();

        // Init wallet
        let wallet_state = WalletState::default();

        // Init Bitcoin client
        let bitcoin_client = BitcoinClient::new(
            format!(
                "http://{}:{}",
                SETTINGS.node_ip.clone(),
                SETTINGS.node_rpc_port
            ),
            SETTINGS.node_username.clone(),
            SETTINGS.node_password.clone(),
        );

        // Init testing app
        let mut app = test::init_service(
            App::new()
                .data(DBState(key_db))
                .wrap(CheckPayment::new(bitcoin_client, wallet_state)) // Apply payment check to put key
                .route("/keys/{addr}", web::put().to(put_key)),
        );

        // Put key with no token
        let (address_base58, metadata_raw) = generate_address_metadata();
        let key_path = &format!("http://localhost:8080/keys/{}", address_base58);
        let req = test::TestRequest::put()
            .uri(key_path)
            .set_payload(metadata_raw)
            .to_request();
        let mut resp = test::call_service(&mut app, req);

        // Check status
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);

        // Get invoice
        let body_vec = resp.take_body().collect().wait().unwrap();
        let invoice_raw = body_vec.get(0).unwrap();
        let invoice = PaymentRequest::decode(invoice_raw).unwrap();

        // Check invoice is valid
        let payment_details = PaymentDetails::decode(invoice.serialized_payment_details).unwrap();
        assert_eq!(payment_details.network.unwrap(), "regnet".to_string());
        assert!(payment_details.expires.unwrap() > payment_details.time);
        assert!(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                >= payment_details.time
        );
        assert_eq!(
            payment_details.payment_url.unwrap(),
            "/payments".to_string()
        );
        assert_eq!(payment_details.merchant_data.unwrap(), key_path.as_bytes())
    }

    #[test]
    fn test_put_payment() {
        // Init db
        let key_db = KeyDB::try_new("./test_db/payment").unwrap();

        // Init wallet
        let wallet_state = WalletState::default();

        // Init Bitcoin client
        let bitcoin_client = BitcoinClient::new(
            format!(
                "http://{}:{}",
                SETTINGS.node_ip.clone(),
                SETTINGS.node_rpc_port
            ),
            SETTINGS.node_username.clone(),
            SETTINGS.node_password.clone(),
        );

        // Init testing app
        let mut app = test::init_service(
            App::new()
                .service(
                    web::scope("/keys").service(
                        web::resource("/{addr}")
                            .data(DBState(key_db))
                            .wrap(CheckPayment::new(
                                bitcoin_client.clone(),
                                wallet_state.clone(),
                            )) // Apply payment check to put key
                            .route(web::put().to_async(put_key)),
                    ),
                )
                .service(
                    web::resource("/payments")
                        .data((bitcoin_client, wallet_state))
                        .route(web::post().to_async(payment_handler)),
                ),
        );

        // Put key with no token
        let (address_base58, metadata_raw) = generate_address_metadata();
        let key_url = &format!("http://localhost:8080/keys/{}", address_base58);
        let req = test::TestRequest::put()
            .uri(key_url)
            .set_payload(metadata_raw)
            .to_request();
        let mut resp = test::call_service(&mut app, req);

        // Create payment
        let body_vec = resp.take_body().collect().wait().unwrap();
        let invoice_raw = body_vec.get(0).unwrap();
        let invoice = PaymentRequest::decode(invoice_raw).unwrap();
        let payment_details = PaymentDetails::decode(invoice.serialized_payment_details).unwrap();
        let p2pkh = payment_details.outputs.get(0).unwrap();
        let addr = p2pkh.script[3..23].to_vec();
        let tx = generate_raw_tx(addr, vec![]); // TODO: Add op_return
        let payment = Payment {
            merchant_data: payment_details.merchant_data,
            memo: None,
            refund_to: vec![],
            transactions: vec![tx],
        };
        let mut payment_raw = Vec::with_capacity(payment.encoded_len());
        payment.encode(&mut payment_raw).unwrap();

        // Check accept header is enforced
        let payment_path = payment_details.payment_url.unwrap();
        let req = test::TestRequest::post()
            .uri(&payment_path)
            .set_payload(payment_raw.clone())
            .to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);

        // Check content-type header is enforced
        let req = test::TestRequest::post()
            .uri(&payment_path)
            .set_payload(payment_raw.clone())
            .header(ACCEPT, "application/bitcoin-payment")
            .to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        // Check payment ack and a token
        let req = test::TestRequest::post()
            .uri(&payment_path)
            .set_payload(payment_raw.clone())
            .header(ACCEPT, "application/bitcoin-payment")
            .header(CONTENT_TYPE, "application/bitcoin-paymentack")
            .to_request();
        let mut resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::FOUND);
        let body_vec = resp.take_body().collect().wait().unwrap();
        let payment_ack_raw = body_vec.get(0).unwrap();
        let payment_ack = PaymentAck::decode(payment_ack_raw).unwrap();
        assert_eq!(payment, payment_ack.payment);

        // Check token
        let auth = resp.headers().get(AUTHORIZATION).unwrap();
        let loc = Url::parse(resp.headers().get(LOCATION).unwrap().to_str().unwrap()).unwrap();
        let loc_noquery = format!(
            "{}://{}:{}{}",
            loc.scheme(),
            loc.host().unwrap(),
            loc.port().unwrap(),
            loc.path()
        );
        assert_eq!(&loc_noquery, key_url);
        let pair = loc.query_pairs().next().unwrap();
        assert_eq!("code", pair.0);
        assert_eq!(&format!("POP {}", pair.1), auth.to_str().unwrap());

        // TODO: More detail here
        // Check token works with code
        let req = test::TestRequest::put().uri(loc.as_str()).to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);

        // TODO: More detail here
        // Check token works with POP token
        let req = test::TestRequest::put()
            .uri(loc.as_str())
            .header(AUTHORIZATION, auth.to_str().unwrap())
            .to_request();
        let resp = test::call_service(&mut app, req);
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
    }
}
