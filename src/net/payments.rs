use std::{
    pin::Pin,
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
use bitcoincash_addr::{Address, HashType};
use bytes::BytesMut;
use futures::{
    future::{err, ok, ready, Ready},
    prelude::*,
    task::{Context, Poll},
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

const PAYMENT_PATH: &str = "/payments";
pub const VALID_DURATION: u64 = 30;

#[derive(Deserialize)]
pub struct TokenQuery {
    code: String,
}

/// Payment handler
pub async fn payment_handler(
    req: HttpRequest,
    mut payload: web::Payload,
    data: web::Data<(BitcoinClient, WalletState)>,
) -> Result<HttpResponse, ServerError> {
    // Check headers
    let headers = req.headers();
    if headers.get(CONTENT_TYPE)
        != Some(&HeaderValue::from_str("application/bitcoincash-payment").unwrap())
    {
        return Err(PaymentError::Accept.into());
    }
    if headers.get(ACCEPT)
        != Some(&HeaderValue::from_str("application/bitcoincash-paymentack").unwrap())
    {
        return Err(PaymentError::Content.into());
    }

    // Read and parse payment proto
    let mut payment_raw = BytesMut::new();
    while let Some(item) = payload.next().await {
        payment_raw.extend_from_slice(&item.map_err(|_| ServerError::PayloadDecode)?);
    }
    let payment = Payment::decode(&payment_raw[..]).map_err(|_| PaymentError::Decode)?;

    // Parse tx
    let tx_raw = match payment.transactions.get(0) {
        Some(some) => some,
        None => return Err(PaymentError::NoTx.into()),
    };

    // Assume first tx
    let tx = Transaction::deserialize(tx_raw).map_err(|err| PaymentError::from(err))?;

    // Check outputs
    let wallet_data = &data.1;
    if !wallet_data.check_outputs(tx) {
        return Err(ServerError::Payment(PaymentError::InvalidOutputs));
    }

    // Send tx
    let bitcoin_client = &data.0;
    bitcoin_client
        .send_tx(tx_raw)
        .await
        .map_err(|_| PaymentError::InvalidTx)?;

    // Create payment ack
    let memo = Some("Thanks for your custom!".to_string());
    let payment_ack = PaymentAck { payment, memo };

    // Encode payment ack
    let mut raw_ack = Vec::with_capacity(payment_ack.encoded_len());
    payment_ack.encode(&mut raw_ack).unwrap();

    // Get merchant data
    let merchant_data = payment_ack
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
    let mut redirect_url =
        Url::parse(str::from_utf8(&merchant_data).map_err(|_| PaymentError::InvalidMerchantDat)?)
            .map_err(|_| PaymentError::InvalidMerchantDat)?;
    redirect_url.set_query(Some(&format!("code={}", token)));

    // Generate response
    Ok(HttpResponse::Accepted()
        .header(LOCATION, redirect_url.into_string())
        .header(AUTHORIZATION, format!("POP {}", token))
        .header(PRAGMA, "no-cache")
        .body(raw_ack))
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
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CheckPaymentMiddleware {
            service,
            client: self.0.clone(),
            wallet_state: self.1.clone(),
        }))
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
    S::Response: 'static,
    S::Future: 'static,
{
    type Request = ServiceRequest;
    type Response = ServiceResponse<Body>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        // Only pay for put
        match *req.method() {
            Method::PUT => (),
            _ => return Box::pin(self.service.call(req)),
        }

        // Get request data
        let conn_info = req.connection_info().clone();
        let scheme = conn_info.scheme().to_owned();
        let host = conn_info.host().to_owned();

        // Grab token query from authorization header then query string
        let token_str: String = match req.headers().get(AUTHORIZATION) {
            Some(some) => match some.to_str() {
                Ok(auth_str) => {
                    if auth_str.len() >= 4 && &auth_str[0..4] == "POP " {
                        auth_str[4..].to_string()
                    } else {
                        return Box::pin(err(
                            ServerError::Payment(PaymentError::InvalidAuth).into()
                        ));
                    }
                }
                Err(_) => {
                    return Box::pin(err(ServerError::Payment(PaymentError::InvalidAuth).into()))
                }
            },
            None => {
                // If no token found then generate invoice

                // Valid interval
                let current_time = SystemTime::now();
                let expiry_time = current_time + Duration::from_secs(VALID_DURATION);

                // Get new addr and add to wallet
                let wallet_state_inner = self.wallet_state.clone();
                let client_inner = self.client.clone();
                let new_addr = async move {
                    let addr_opt = client_inner.get_new_addr().await;
                    match addr_opt {
                        Ok(addr_str) => {
                            let addr =
                                Address::decode(&addr_str).map_err(|(cash_err, base58_err)| {
                                    ServerError::Address(cash_err, base58_err)
                                })?;
                            let network: Network = addr.network.clone().into();
                            if network != SETTINGS.network || addr.hash_type != HashType::Key {
                                return Err(ServerError::Payment(PaymentError::MismatchedNetwork))?;
                                // TODO: Finer grained error here
                            }
                            let addr_raw = addr.into_body();
                            wallet_state_inner.add(addr_raw.clone());
                            Ok(addr_raw)
                        }
                        Err(_e) => Err(ServerError::Payment(PaymentError::AddrFetchFailed).into()),
                    }
                };

                // Decode put address
                let uri = req.uri();
                let put_addr_path = uri.path();
                let put_addr_str = &put_addr_path[6..]; // TODO: This is super hacky
                let put_addr = match Address::decode(put_addr_str) {
                    Ok(ok) => ok,
                    Err((cash_err, base58_err)) => {
                        return Box::pin(err(ServerError::Address(cash_err, base58_err).into()))
                    }
                };

                // Generate merchant URL
                let base_url = format!("{}://{}", scheme, host);
                let merchant_url = format!("{}{}", base_url, put_addr_path);

                let response = new_addr.and_then(move |addr_raw| {
                    // Generate outputs
                    let outputs = generate_outputs(addr_raw, &base_url, put_addr.into_body());

                    // Collect payment details
                    let payment_url = Some(format!("{}{}", base_url, PAYMENT_PATH));
                    let payment_details = PaymentDetails {
                        network: Some(SETTINGS.network.to_string()),
                        time: current_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        expires: Some(expiry_time.duration_since(UNIX_EPOCH).unwrap().as_secs()),
                        memo: None,
                        merchant_data: Some(merchant_url.as_bytes().to_vec()),
                        outputs,
                        payment_url,
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
                    let mut payment_invoice_raw = Vec::with_capacity(payment_invoice.encoded_len());
                    payment_invoice.encode(&mut payment_invoice_raw).unwrap();

                    HttpResponse::PaymentRequired()
                        .content_type("application/bitcoincash-paymentrequest")
                        .header("Content-Transfer-Encoding", "binary")
                        .body(payment_invoice_raw)
                });

                // Respond
                return Box::pin(response.map_ok(move |resp| req.into_response(resp)));
            }
        };

        // Decode token
        let url_safe_config = base64::Config::new(base64::CharacterSet::UrlSafe, false);
        let token = match base64::decode_config(&token_str, url_safe_config) {
            Ok(some) => some,
            Err(_) => {
                return Box::pin(ok(req.into_response(
                    ServerError::Payment(PaymentError::InvalidAuth).error_response(),
                )))
            }
        };

        // Generate merchant URL
        let uri = req.uri();
        let merchant_url = format!("{}://{}{}", scheme, host, uri.path());

        // Validate
        if !validate_token(merchant_url.as_bytes(), SETTINGS.secret.as_bytes(), &token) {
            Box::pin(ok(req.into_response(
                ServerError::Payment(PaymentError::InvalidAuth).error_response(),
            )))
        } else {
            Box::pin(self.service.call(req))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use actix_web::{http::StatusCode, test, web, App};
    use bigdecimal::BigDecimal;
    use bitcoincash_addr::HashType;
    use futures::TryFutureExt;
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

    async fn generate_raw_tx(recv_addr: Vec<u8>, _data: Vec<u8>) -> Vec<u8> {
        let client = JsonClient::new(
            format!("http://{}:{}", SETTINGS.node_ip.clone(), SETTINGS.rpc_port),
            SETTINGS.rpc_username.clone(),
            SETTINGS.rpc_password.clone(),
        );

        // Get unspent output
        let unspent_req = client.build_request("listunspent".to_string(), vec![]);
        let utxos = client
            .send_request(&unspent_req)
            .await
            .unwrap()
            .into_result::<Vec<Outpoint>>()
            .unwrap();
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
            .await
            .unwrap()
            .into_result::<String>()
            .unwrap();

        // Sign raw transaction
        let signed_raw_req = client.build_request(
            "signrawtransactionwithwallet".to_string(),
            vec![json!(unsigned_raw_tx)],
        );
        let signed_raw_tx = client
            .send_request(&signed_raw_req)
            .await
            .unwrap()
            .into_result::<SignedHex>()
            .unwrap()
            .hex;
        hex::decode(signed_raw_tx).unwrap()
    }

    #[actix_rt::test]
    async fn test_put_no_token() {
        // Init db
        let key_db = KeyDB::try_new("./test_db/no_token").unwrap();

        // Init wallet
        let wallet_state = WalletState::default();

        // Init Bitcoin client
        let bitcoin_client = BitcoinClient::new(
            format!("http://{}:{}", SETTINGS.node_ip.clone(), SETTINGS.rpc_port),
            SETTINGS.rpc_username.clone(),
            SETTINGS.rpc_password.clone(),
        );

        // Init testing app
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .wrap(CheckPayment::new(bitcoin_client, wallet_state)) // Apply payment check to put key
                .route("/keys/{addr}", web::put().to(put_key)),
        )
        .await;

        // Put key with no token
        let (address_base58, metadata_raw) = generate_address_metadata();
        let key_path = &format!("http://localhost:8080/keys/{}", address_base58);
        let req = test::TestRequest::put()
            .uri(key_path)
            .set_payload(metadata_raw)
            .to_request();
        let mut resp = test::call_service(&mut app, req).await;

        // Check status
        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);

        // Get invoice
        let mut payload = resp.take_body();
        let mut invoice_raw = BytesMut::new();
        while let Some(item) = payload.next().await {
            invoice_raw.extend_from_slice(&item.unwrap());
        }
        let invoice = PaymentRequest::decode(invoice_raw).unwrap();

        // Check invoice is valid
        let payment_details =
            PaymentDetails::decode(&invoice.serialized_payment_details[..]).unwrap();
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
            "http://localhost:8080/payments".to_string()
        );
        assert_eq!(payment_details.merchant_data.unwrap(), key_path.as_bytes())
    }

    #[actix_rt::test]
    async fn test_put_payment() {
        // Init db
        let key_db = KeyDB::try_new("./test_db/payment").unwrap();

        // Init wallet
        let wallet_state = WalletState::default();

        // Init Bitcoin client
        let bitcoin_client = BitcoinClient::new(
            format!("http://{}:{}", SETTINGS.node_ip.clone(), SETTINGS.rpc_port),
            SETTINGS.rpc_username.clone(),
            SETTINGS.rpc_password.clone(),
        );

        // Init testing app
        let mut app = test::init_service(
            App::new()
                .service(
                    web::scope("/keys").service(
                        web::resource("/{addr}")
                            .data(key_db)
                            .wrap(CheckPayment::new(
                                bitcoin_client.clone(),
                                wallet_state.clone(),
                            )) // Apply payment check to put key
                            .route(web::put().to(put_key)),
                    ),
                )
                .service(
                    web::resource("/payments")
                        .data((bitcoin_client, wallet_state))
                        .route(web::post().to(payment_handler)),
                ),
        )
        .await;

        // Put key with no token
        let (address_base58, metadata_raw) = generate_address_metadata();
        let key_url = &format!("http://localhost:8080/keys/{}", address_base58);
        let req = test::TestRequest::put()
            .uri(key_url)
            .set_payload(metadata_raw.clone())
            .to_request();
        let mut resp = test::call_service(&mut app, req).await;

        // Create payment
        let mut payload = resp.take_body();
        let mut invoice_raw = BytesMut::new();
        while let Some(item) = payload.next().await {
            invoice_raw.extend_from_slice(&item.unwrap());
        }
        let invoice = PaymentRequest::decode(invoice_raw).unwrap();
        let payment_details =
            PaymentDetails::decode(&invoice.serialized_payment_details[..]).unwrap();
        let p2pkh = payment_details.outputs.get(0).unwrap();
        let addr = p2pkh.script[3..23].to_vec();
        let tx = generate_raw_tx(addr, vec![]).await; // TODO: Add op_return
        let payment = Payment {
            merchant_data: payment_details.merchant_data,
            memo: None,
            refund_to: vec![],
            transactions: vec![tx],
        };
        let mut payment_raw = Vec::with_capacity(payment.encoded_len());
        payment.encode(&mut payment_raw).unwrap();

        // Check content-type header is enforced
        let payment_url = payment_details.payment_url.unwrap();
        let req = test::TestRequest::post()
            .uri(&payment_url)
            .set_payload(payment_raw.clone())
            .header(ACCEPT, "application/bitcoincash-paymentack")
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_ACCEPTABLE);

        // Check accept header is enforced
        let req = test::TestRequest::post()
            .uri(&payment_url)
            .set_payload(payment_raw.clone())
            .header(CONTENT_TYPE, "application/bitcoincash-payment")
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

        // Check payment ack and a token
        let req = test::TestRequest::post()
            .uri(&payment_url)
            .set_payload(payment_raw.clone())
            .header(CONTENT_TYPE, "application/bitcoincash-payment")
            .header(ACCEPT, "application/bitcoincash-paymentack")
            .to_request();
        let mut resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let mut payload = resp.take_body();
        let mut payment_ack_raw = BytesMut::new();
        while let Some(item) = payload.next().await {
            payment_ack_raw.extend_from_slice(&item.unwrap());
        }
        let payment_ack = PaymentAck::decode(&payment_ack_raw[..]).unwrap();
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
        let req = test::TestRequest::put()
            .uri(loc.as_str())
            .set_payload(metadata_raw)
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
