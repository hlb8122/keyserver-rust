pub mod errors;
pub mod payments;
pub mod token;

use actix_web::{web, HttpResponse};
use futures::{
    future::{err, ok, Future},
    stream::Stream,
};
use prost::Message;

use crate::{crypto::Address, db::KeyDB, models::AddressMetadata};
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
        web::BytesMut::new(),
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

        // Put to database
        data.0.put(&addr, &metadata)?;

        // Respond
        Ok(HttpResponse::Ok().finish())
    });
    Box::new(response)
}
