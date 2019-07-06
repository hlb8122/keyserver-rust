pub mod authentication;
pub mod crypto;
pub mod db;
pub mod jsonrpc_client;
pub mod net;
pub mod token;

use actix_web::{web, App, HttpServer};

use crate::{db::KeyDB, net::rest_server::*};

pub mod models {
    include!(concat!(env!("OUT_DIR"), "/models.rs"));
}

fn main() {
    let key_db = KeyDB::try_default().unwrap();
    HttpServer::new(move || {
        let key_db_inner = key_db.clone();
        App::new()
            .data(State(key_db_inner))
            .route("/keys/", web::get().to(keys_index))
            .route("/keys/{addr}", web::get().to(get_key))
    })
    .bind("127.0.0.1:8000")
    .expect("Can not bind to port 8000")
    .run()
    .unwrap();
}
