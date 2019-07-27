#[macro_use]
extern crate clap;

pub mod authentication;
pub mod bitcoin;
pub mod crypto;
pub mod db;
pub mod net;
pub mod settings;

use std::sync::Arc;

use actix_web::{middleware::Logger, web, App, HttpServer};
use bitcoin_hashes::{hash160, Hash};
use futures::{
    future::{ok, Either},
    Future, Stream,
};
use lazy_static::lazy_static;
use prost::Message;

use crate::{
    db::KeyDB,
    net::{payments::*, *},
    settings::Settings,
};

pub mod models {
    include!(concat!(env!("OUT_DIR"), "/models.rs"));
}

lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new().expect("couldn't load config");
}

const node_addr: &str = "127.0.0.1";
const node_rpc_port: u16 = 8332;
const node_zmq_port: u16 = 18332;

fn main() {
    println!("starting server @ {}", SETTINGS.bind);

    // Init logging
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    // Open DB
    let key_db = KeyDB::try_new(&SETTINGS.dbpath).unwrap();

    // Init ZMQ
    let (tx_stream, broker) = tx_stream::get_tx_stream(&format!("{}:{}", node_addr, node_zmq_port));
    let key_stream = tx_stream::extract_details(tx_stream);

    // Peer client
    let client = Arc::new(peer::PeerClient::new());

    // Setup peer polling logic
    let key_db_inner = key_db.clone();
    let peer_polling = key_stream.for_each(move |(peer_addr, pkhash, meta_digest)| {
        let bitcoin_addr = match pkhash.encode() {
            Ok(ok) => ok,
            Err(_e) => {
                // TODO: Log encoding error here
                return Either::A(ok(()));
            }
        };

        // Get metadata from peer
        let metadata_fut = client.clone().get_metadata(&peer_addr, &bitcoin_addr);

        let key_db_inner = key_db_inner.clone();
        Either::B(metadata_fut.then(move |metadata| {
            let metadata = match metadata {
                Ok(ok) => ok,
                Err(_e) => {
                    // TODO: Log client error
                    return ok(());
                }
            };
            // Check digest matches
            let mut metadata_raw = Vec::with_capacity(metadata.encoded_len());
            metadata.encode(&mut metadata_raw).unwrap();
            let actual_digest = &hash160::Hash::hash(&metadata_raw)[..];
            if actual_digest != &meta_digest[..] {
                // TODO: Log fake metadata
                return ok(());
            }

            if let Err(_e) = key_db_inner.clone().put(&pkhash, &metadata) {
                // TODO: Log error
            };
            ok(())
        }))
    });

    // Init REST server
    HttpServer::new(move || {
        let key_db_inner = key_db.clone();
        App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .service(
                web::scope("/keys").service(
                    web::resource("/{addr}")
                        .data(State(key_db_inner))
                        .wrap(CheckPayment) // Apply payment check to put key
                        .route(web::get().to(get_key))
                        .route(web::put().to_async(put_key)),
                ),
            )
            .service(web::resource("/payments").route(web::post().to_async(payment_handler)))
            .service(actix_files::Files::new("/", "./static/").index_file("index.html"))
    })
    .bind(&SETTINGS.bind)
    .unwrap_or_else(|_| panic!("failed to bind to {}", SETTINGS.bind))
    .run()
    .unwrap();
}
