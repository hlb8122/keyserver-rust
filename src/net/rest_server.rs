use actix_web::{web, HttpRequest, HttpResponse};
use prost::Message;

use super::errors::ServerError;
use crate::{crypto::bitcoin_addr::BitcoinAddress, crypto::*, db::KeyDB};

pub struct State(pub KeyDB);

pub fn keys_index() -> String {
    "You have found a keytp server.".to_string()
}

pub fn get_key(req: HttpRequest, data: web::Data<State>) -> Result<HttpResponse, ServerError> {
    // Convert address
    let addr_hex: String = req.match_info().get("addr").unwrap().parse().unwrap();
    let addr = BitcoinAddress::from_hex(addr_hex)?;

    // Grab metadata from DB
    let metadata = data
        .0
        .get(&addr)
        .map_err(|_| ServerError::DB)?
        .ok_or(ServerError::NotFound)?;

    // Encode metadata as hex
    let mut raw_payload = Vec::with_capacity(metadata.encoded_len());
    metadata.encode(&mut raw_payload).unwrap();

    // Respond
    Ok(HttpResponse::Ok().body(hex::encode(raw_payload)))
}

// pub fn put_key(req: HttpRequest, data: web::Data<State>) -> HttpResponse {
//     let addr_hex: String = req.match_info().get("addr").unwrap().parse().unwrap();
//     let addr = match BitcoinAddress::from_hex(addr_hex) {
//         Ok(ok) => ok,
//         Err(e) => {
//             return match e {
//                 CryptoError::NonHexAddress => HttpResponse::BadRequest().body("non-hex address"),
//                 CryptoError::Deserialization => HttpResponse::BadRequest().body("invalid address"),
//                 _ => unreachable!(),
//             }
//         }
//     };

// }
