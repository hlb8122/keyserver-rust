use actix_web::{web, HttpRequest, HttpResponse};
use prost::Message;

use crate::{crypto::bitcoin_addr::BitcoinAddress, crypto::Address, db::KeyDB};

pub struct State(pub KeyDB);

pub fn keys_index() -> String {
    "You have found a keytp server.".to_string()
}

pub fn get_key(req: HttpRequest, data: web::Data<State>) -> HttpResponse {    
    let addr_str: String = req.match_info().get("addr").unwrap().parse().unwrap();
    let addr_raw = match hex::decode(addr_str).map_err(|_| ()) {
        Ok(ok) => ok,
        Err(_) => return HttpResponse::BadRequest().body("non-hex address"),
    };

    let addr = match BitcoinAddress::deserialize(&addr_raw).map_err(|_| ()) {
        Ok(ok) => ok,
        Err(_) => return HttpResponse::BadRequest().body("invalid address"),
    };

    match data.0.get(&addr) {
        Ok(ok) => match ok {
            Some(some) => {
                let mut raw_payload = Vec::with_capacity(some.encoded_len());
                some.encode(&mut raw_payload).unwrap();
                HttpResponse::Ok().body(hex::encode(raw_payload))
            }
            None => HttpResponse::NotFound().body("missing key address"),
        },
        Err(_) => HttpResponse::InternalServerError().body("database read failure"),
    }
}
