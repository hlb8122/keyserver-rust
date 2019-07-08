pub mod errors;
pub mod payments;
pub mod token;

use actix_web::{web, HttpResponse};
use bytes::Bytes;
use prost::Message;

use crate::{crypto::address::*, db::KeyDB, models::AddressMetadata};
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
    body: Bytes,
    data: web::Data<State>,
) -> Result<HttpResponse, ServerError> {
    // Convert address
    let addr = Address::decode(&addr_str)?;

    // Decode metadata
    let metadata = AddressMetadata::decode(body)?;

    // Put to database
    data.0.put(&addr, &metadata)?;

    // Respond
    Ok(HttpResponse::Ok().finish())
}
