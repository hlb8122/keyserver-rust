pub mod errors;
pub mod jsonrpc_client;
pub mod payments;
pub mod peer;

use actix_web::{web, HttpResponse};
use bytes::BytesMut;
use futures::prelude::*;
use prost::Message;

use crate::{
    crypto::{authentication::validate, ecdsa::Secp256k1, Address},
    db::KeyDB,
    models::{AddressMetadata, Payload},
};

use errors::ServerError;

pub async fn get_key(
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

pub async fn put_key(
    addr_str: web::Path<String>,
    mut payload: web::Payload,
    db_data: web::Data<KeyDB>,
) -> Result<HttpResponse, ServerError> {
    // Decode metadata
    let mut metadata_raw = BytesMut::new();
    while let Some(item) = payload.next().await {
        metadata_raw.extend_from_slice(&item.map_err(|_| ServerError::MetadataDecode)?);
    }
    let metadata =
        AddressMetadata::decode(&metadata_raw[..]).map_err(|_| ServerError::MetadataDecode)?;

    // Convert address
    let addr = Address::decode(&addr_str)?;

    // TODO: Support Schnorr
    match metadata.scheme {
        1 => validate::<Secp256k1>(&addr, &metadata).map_err(ServerError::Validation)?,
        _ => return Err(ServerError::UnsupportedSigScheme),
    }

    // Decode payload
    let raw_payload = &metadata.serialized_payload;
    let payload = Payload::decode(&raw_payload[..]).map_err(|_| ServerError::PayloadDecode)?;

    // Check age
    db_data.check_timestamp(&addr, &payload)??;

    // Put to database
    db_data.put(&addr, &metadata)?;

    // Respond
    Ok(HttpResponse::Ok().finish())
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
    use bitcoin_hashes::{sha256, Hash};
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
        let payload_digest = &sha256::Hash::hash(&raw_payload)[..];
        let signature = secp.sign(
            &secp256k1::Message::from_slice(&payload_digest).unwrap(),
            &sk,
        );

        // Construct metadata
        let metadata = AddressMetadata {
            pub_key: pubkey_raw,
            serialized_payload: raw_payload,
            signature: signature.serialize_compact().to_vec(),
            scheme: 1,
        };
        let mut metadata_raw = Vec::with_capacity(metadata.encoded_len());
        metadata.encode(&mut metadata_raw).unwrap();

        (address_base58, metadata_raw)
    }

    #[actix_rt::test]
    async fn test_index_put_ok() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_ok").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::put().to(put_key)),
        )
        .await;

        let (address_base58, metadata_raw) = generate_address_metadata();

        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", address_base58))
            .set_payload(metadata_raw)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_rt::test]
    async fn test_index_put_malformed_payload() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_malformed").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::put().to(put_key)),
        )
        .await;

        let (address_base58, _) = generate_address_metadata();

        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", address_base58))
            .set_payload(vec![2, 3, 5])
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_rt::test]
    async fn test_index_put_invalid_address() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_invalid_addr").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::put().to(put_key)),
        )
        .await;

        let (_, metadata_raw) = generate_address_metadata();

        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", "invalid"))
            .set_payload(metadata_raw)
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_rt::test]
    async fn test_index_get_invalid_address() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/get_invalid_addr").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::get().to(get_key)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/keys/{}", "invalid"))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_rt::test]
    async fn test_index_get_not_found() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/get_not_found").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::get().to(get_key)),
        )
        .await;

        let (address_base58, _) = generate_address_metadata();

        let req = test::TestRequest::get()
            .uri(&format!("/keys/{}", address_base58))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[actix_rt::test]
    async fn test_index_put_get() {
        // Init routes
        let key_db = KeyDB::try_new("./test_db/put_get").unwrap();
        let mut app = test::init_service(
            App::new()
                .data(key_db)
                .route("/keys/{addr}", web::get().to(get_key))
                .route("/keys/{addr}", web::put().to(put_key)),
        )
        .await;

        let (address_base58, metadata_raw) = generate_address_metadata();

        // Put metadata
        let req = test::TestRequest::put()
            .uri(&format!("/keys/{}", address_base58))
            .set_payload(metadata_raw.clone())
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Get metadata
        let req = test::TestRequest::get()
            .uri(&format!("/keys/{}", address_base58))
            .to_request();
        let resp = app.call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = test::read_body(resp).await;
        assert_eq!(&body[..], &metadata_raw[..]);
    }
}
