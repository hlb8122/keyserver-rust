pub mod errors;
pub mod payment;

use actix_web::{web, HttpResponse};
use bytes::Bytes;
use prost::Message;

use crate::{crypto::bitcoin_addr::BitcoinAddress, crypto::*, db::KeyDB, models::AddressMetadata};
use errors::ServerError;

pub struct State(pub KeyDB);

pub fn get_key(
    addr_hex: web::Path<String>,
    data: web::Data<State>,
) -> Result<HttpResponse, ServerError> {
    // Convert address
    let addr = BitcoinAddress::from_hex(addr_hex.to_string())?;

    // Grab metadata from DB
    let metadata = data.0.get(&addr)?.ok_or(ServerError::NotFound)?;

    // Encode metadata as hex
    let mut raw_payload = Vec::with_capacity(metadata.encoded_len());
    metadata.encode(&mut raw_payload).unwrap();

    // Respond
    Ok(HttpResponse::Ok().body(hex::encode(raw_payload)))
}

pub fn put_key(
    addr_hex: web::Path<String>,
    body: Bytes,
    data: web::Data<State>,
) -> Result<HttpResponse, ServerError> {
    // Convert address
    let addr = BitcoinAddress::from_hex(addr_hex.to_string())?;

    // Decode metadata
    let metadata = AddressMetadata::decode(body)?;

    // Put to database
    data.0.put(&addr, &metadata)?;

    // Respond
    Ok(HttpResponse::Ok().finish())
}
