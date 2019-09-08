pub mod errors;
pub mod peer;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use actix_web::{http::header::AUTHORIZATION, web, HttpRequest, HttpResponse};
use bytes::BytesMut;
use futures::{
    future::{err, Future},
    stream::Stream,
};
use prost::Message;
use reqwest::r#async::Client as HttpClient;
use serde_derive::Deserialize;

use crate::{
    crypto::{authentication::validate, ecdsa::Secp256k1, Address, token::validate_token},
    db::KeyDB,
    models::*,
    SETTINGS,
};

use errors::*;

pub const VALID_DURATION: u64 = 30;
pub const PRICE: u64 = 5;

pub fn get_key(
    addr_str: web::Path<String>,
    db_data: web::Data<KeyDB>,
) -> Result<HttpResponse, ServerError> {
    // Convert address
    let addr = Address::decode(&addr_str)?;

    // Grab metadata from DB
    let metadata = db_data.get(&addr)?.ok_or(ServerError::NotFound)?;

    // Encode metadata as hex
    let mut raw_payload = Vec::with_capacity(metadata.encoded_len());
    metadata.encode(&mut raw_payload).unwrap();

    // Respond
    Ok(HttpResponse::Ok().body(raw_payload))
}

#[derive(Deserialize)]
pub struct TokenQuery {
    code: String,
}

pub fn put_key(
    req: HttpRequest,
    addr_str: web::Path<String>,
    payload: web::Payload,
    db_data: web::Data<KeyDB>,
    http_client: web::Data<HttpClient>,
) -> Box<dyn Future<Item = HttpResponse, Error = ServerError>> {
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
                    return Box::new(err(PaymentError::InvalidAuth.into()));
                }
            }
            Err(_) => return Box::new(err(PaymentError::InvalidAuth.into())),
        },
        None => match web::Query::<TokenQuery>::from_query(req.query_string()) {
            Ok(query) => query.code.clone(), // TODO: Remove clone?
            Err(_) => {
                // Decode put address
                let uri = req.uri();
                let put_addr_path = uri.path();
                let put_addr_str = &put_addr_path[6..]; // TODO: This is super hacky
                if let Err(e) = Address::decode(put_addr_str) {
                    return Box::new(err(ServerError::Address(e)));
                }

                // Payment interval
                let current_time = SystemTime::now();
                let expiry_time = current_time + Duration::from_secs(VALID_DURATION);

                // Generate merchant URL
                let base_url = format!("{}://{}", scheme, host);
                let merchant_url = format!("{}{}", base_url, put_addr_path);

                // Construct invoice request
                let invoice_request = InvoiceRequest {
                    network: SETTINGS.network.to_string(),
                    amount: PRICE,
                    time: current_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    expires: expiry_time.duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    ack_memo: "Thanks for the custom!".to_string(),
                    req_memo: String::new(),
                    tokenize: true,
                    tx_data: vec![],
                    merchant_data: merchant_url.as_bytes().to_vec(),
                    callback_url: String::new(),
                };
                let mut serialized_invoice_request =
                    Vec::with_capacity(invoice_request.encoded_len());
                invoice_request
                    .encode(&mut serialized_invoice_request)
                    .unwrap();

                // Send InvoiceRequest to BIP70 server
                let bip70_response = http_client
                    .post(&SETTINGS.bip70_server_url)
                    .body(serialized_invoice_request)
                    .send()
                    .map_err(PaymentError::Bip70Server);

                // Extract PaymentDetails
                let fut_invoice_response = bip70_response.and_then(|resp| {
                    resp.into_body()
                        .map_err(|_| PaymentError::Payload)
                        .fold(BytesMut::new(), move |mut body, chunk| {
                            body.extend_from_slice(&chunk);
                            Ok::<_, PaymentError>(body)
                        })
                        .and_then(|payment_raw| {
                            InvoiceResponse::decode(payment_raw).map_err(|_| PaymentError::Decode)
                        })
                });

                // Create response
                let response = fut_invoice_response
                    .and_then(|invoice_response| {
                        let payment_request = match invoice_response.payment_request {
                            Some(some) => some,
                            None => return Err(PaymentError::Decode),
                        };
                        let mut serialized_payment_request =
                            Vec::with_capacity(payment_request.encoded_len());
                        payment_request
                            .encode(&mut serialized_payment_request)
                            .unwrap();

                        Ok(HttpResponse::PaymentRequired()
                            .content_type("application/bitcoincash-paymentrequest")
                            .header("Content-Transfer-Encoding", "binary")
                            .body(serialized_payment_request))
                    })
                    .map_err(ServerError::Payment);

                return Box::new(response);
            }
        },
    };

    // Decode token
    let url_safe_config = base64::Config::new(base64::CharacterSet::UrlSafe, false);
    let token = match base64::decode_config(&token_str, url_safe_config) {
        Ok(some) => some,
        Err(_) => return Box::new(err(ServerError::Payment(PaymentError::InvalidAuth))),
    };

    // Generate PUT URL
    let uri = req.uri();
    let merchant_url = format!("{}://{}{}", scheme, host, uri.path());

    if !validate_token(merchant_url.as_bytes(), SETTINGS.secret.as_bytes(), &token) {}
    // Decode metadata
    let body_raw = payload.map_err(|_| ServerError::MetadataDecode).fold(
        BytesMut::new(),
        move |mut body, chunk| {
            body.extend_from_slice(&chunk);
            Ok::<_, ServerError>(body)
        },
    );
    let metadata = body_raw.and_then(|metadata_raw| {
        AddressMetadata::decode(metadata_raw).map_err(|_| ServerError::MetadataDecode)
    });

    let response = metadata.and_then(move |metadata| {
        // Convert address
        let addr = Address::decode(&addr_str)?;

        // TODO: Support Schnorr
        match metadata.scheme {
            1 => validate::<Secp256k1>(&addr, &metadata).map_err(ServerError::Validation)?,
            _ => return Err(ServerError::UnsupportedSigScheme),
        }

        // Check age
        db_data.check_timestamp(&addr, &metadata)??;

        // Put to database
        db_data.put(&addr, &metadata)?;

        // Respond
        Ok(HttpResponse::Ok().finish())
    });
    Box::new(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{ecdsa::Secp256k1PublicKey, *},
        models::*,
        SETTINGS,
    };
    use actix_service::Service;
    use actix_web::{http::StatusCode, test, web, App};
    use bitcoin_hashes::{sha256d, Hash};
    use bitcoincash_addr::HashType;
    use secp256k1::{rand, Secp256k1};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn generate_address_metadata() -> (String, Vec<u8>) {
        // Generate public key
        let secp = Secp256k1::new();
        let (sk, pk) = secp.generate_keypair(&mut rand::thread_rng());
        let public_key = Secp256k1PublicKey(pk);
        let pubkey_raw = public_key.serialize();
        let address_raw = public_key.to_raw_address();

        // Generate address
        let address_base58 =
            Base58Codec::encode(&address_raw, HashType::Key, SETTINGS.network.clone().into())
                .unwrap();

        // Construct header
        let headers = vec![Header {
            name: "Type".to_string(),
            value: "EgoBoost".to_string(),
        }];

        // Construct metadata field
        let entries = vec![Entry {
            kind: "text_utf8".to_string(),
            headers,
            entry_data: "This is going to be so much faster than Go"
                .as_bytes()
                .to_vec(),
        }];

        // Construct payload
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let ttl = 3000;
        let payload = Payload {
            timestamp,
            ttl,
            entries,
        };

        // Construct signature
        let mut raw_payload = Vec::with_capacity(payload.encoded_len());
        payload.encode(&mut raw_payload).unwrap();
        let payload_digest = &sha256d::Hash::hash(&raw_payload)[..];
        let signature = secp.sign(
            &secp256k1::Message::from_slice(&payload_digest).unwrap(),
            &sk,
        );

        // Construct metadata
        let metadata = AddressMetadata {
            pub_key: pubkey_raw,
            payload: Some(payload),
            signature: signature.serialize_compact().to_vec(),
            scheme: 1,
        };
        let mut metadata_raw = Vec::with_capacity(metadata.encoded_len());
        metadata.encode(&mut metadata_raw).unwrap();

        (address_base58, metadata_raw)
    }

    #[test]
    fn test_index_put_ok() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_ok").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::put().to(put_key)),
        );

        let (address_base58, metadata_raw) = generate_address_metadata();

        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", address_base58))
            .set_payload(metadata_raw)
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_index_put_malformed_payload() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_malformed").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::put().to(put_key)),
        );

        let (address_base58, _) = generate_address_metadata();

        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", address_base58))
            .set_payload(vec![2, 3, 5])
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_index_put_invalid_address() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_invalid_addr").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::put().to(put_key)),
        );

        let (_, metadata_raw) = generate_address_metadata();

        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", "invalid"))
            .set_payload(metadata_raw)
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_index_get_invalid_address() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/get_invalid_addr").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::get().to(get_key)),
        );

        let req = test::TestRequest::get()
            .uri(&format!("/keys/{}", "invalid"))
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_index_get_not_found() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/get_not_found").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::get().to(get_key)),
        );

        let (address_base58, _) = generate_address_metadata();

        let req = test::TestRequest::get()
            .uri(&format!("/keys/{}", address_base58))
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_index_put_get() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_get").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::get().to(get_key))
                .route("/keys/{addr}", web::put().to(put_key)),
        );

        let (address_base58, metadata_raw) = generate_address_metadata();

        // Put metadata
        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", address_base58))
            .set_payload(metadata_raw.clone())
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Get metadata
        let req = test::TestRequest::get()
            .uri(&format!("/keys/{}", address_base58))
            .to_request();
        let resp = test::block_on(app.call(req)).unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = test::read_body(resp);
        assert_eq!(&body[..], &metadata_raw[..]);
    }
}
