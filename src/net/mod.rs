pub mod errors;
pub mod payments;
pub mod peer;
pub mod token;
pub mod tx_stream;

use actix_web::{web, HttpResponse};
use bytes::BytesMut;
use futures::{future::Future, stream::Stream};
use prost::Message;

use crate::{
    crypto::{authentication::validate, ecdsa::Secp256k1, Address},
    db::KeyDB,
    models::AddressMetadata,
};
use errors::ServerError;

pub struct State(pub KeyDB);

pub fn get_key(
    addr_str: web::Path<String>,
    data: web::Data<State>,
) -> Result<HttpResponse, ServerError> {
    // Convert address
    let addr = Address::decode(&addr_str)?;

    // Grab metadata from DB
    let metadata = data.0.get(&addr)?.ok_or(ServerError::NotFound)?;

    // Encode metadata as hex
    let mut raw_payload = Vec::with_capacity(metadata.encoded_len());
    metadata.encode(&mut raw_payload).unwrap();

    // Respond
    Ok(HttpResponse::Ok().body(raw_payload))
}

pub fn put_key(
    addr_str: web::Path<String>,
    payload: web::Payload,
    data: web::Data<State>,
) -> Box<Future<Item = HttpResponse, Error = ServerError>> {
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

        validate::<Secp256k1>(&addr, &metadata).map_err(ServerError::Validation)?;

        // Put to database
        data.0.put(&addr, &metadata)?;

        // Respond
        Ok(HttpResponse::Ok().finish())
    });
    Box::new(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bitcoin::Network,
        crypto::{ecdsa::Secp256k1PublicKey, *},
        models::*,
    };
    use actix_service::Service;
    use actix_web::{http::StatusCode, test, web, App};
    use secp256k1::{rand, Secp256k1};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn generate_address_metadata() -> (String, Vec<u8>) {
        // Generate public key
        let secp = Secp256k1::new();
        let public_key = Secp256k1PublicKey(secp.generate_keypair(&mut rand::thread_rng()).1);
        let pubkey_raw = public_key.serialize();
        let address_raw = public_key.to_raw_address();

        // Generate address
        let address_base58 = Base58Codec::encode(&address_raw, Network::Mainnet).unwrap();

        // Construct header
        let headers = vec![Header {
            name: "Type".to_string(),
            value: "EgoBoost".to_string(),
        }];

        // Construct metadata field
        let rows = vec![MetadataField {
            headers,
            metadata: "This is going to be so much faster than Go"
                .as_bytes()
                .to_vec(),
        }];

        // Construct payload
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let payload = Some(Payload { timestamp, rows });

        // Construct metadata
        let metadata = AddressMetadata {
            pub_key: pubkey_raw,
            payload,
            signature: vec![],
            r#type: 0,
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
                .data(State(key_db))
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
                .data(State(key_db))
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
                .data(State(key_db))
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
                .data(State(key_db))
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
                .data(State(key_db))
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
                .data(State(key_db))
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
