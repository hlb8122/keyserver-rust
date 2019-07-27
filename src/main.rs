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
use env_logger::Env;
use futures::{
    future::{ok, Either},
    Future, Stream,
};
use lazy_static::lazy_static;
use log::{error, info, warn};
use prost::Message;

use crate::{
    authentication::validate,
    crypto::ecdsa::Secp256k1,
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

fn main() {
    let sys = actix_rt::System::new("keyserver");

    // Init logging
    env_logger::from_env(Env::default().default_filter_or("actix_web=info,keyserver=info")).init();
    info!("starting server @ {}", SETTINGS.bind);

    // Open DB
    let key_db = KeyDB::try_new(&SETTINGS.db_path).unwrap();

    // Init ZMQ
    let (tx_stream, connection) = tx_stream::get_tx_stream(&format!(
        "tcp://{}:{}",
        SETTINGS.node_ip, SETTINGS.node_zmq_port
    ));
    let key_stream = tx_stream::extract_details(tx_stream);
    actix_rt::Arbiter::current().send(connection.map_err(|e| {
        error!("{:?}", e);
        ()
    }));

    // Peer client
    let client = Arc::new(peer::PeerClient::new());

    // Setup peer polling logic
    let key_db_inner = key_db.clone();
    let peer_polling = key_stream.for_each(move |(peer_addr, bitcoin_addr, meta_digest)| {
        let bitcoin_addr_str = match bitcoin_addr.encode() {
            Ok(ok) => ok,
            Err(e) => {
                warn!("{}", e);
                return Either::A(ok(()));
            }
        };

        // Get metadata from peer
        let metadata_fut = client.clone().get_metadata(&peer_addr, &bitcoin_addr_str);

        let key_db_inner = key_db_inner.clone();
        Either::B(metadata_fut.then(move |metadata| {
            let metadata = match metadata {
                Ok(ok) => ok,
                Err(e) => {
                    warn!("{:?}", e);
                    return ok(());
                }
            };

            // Check digest matches
            let mut metadata_raw = Vec::with_capacity(metadata.encoded_len());
            metadata.encode(&mut metadata_raw).unwrap();
            let actual_digest = &hash160::Hash::hash(&metadata_raw)[..];
            if actual_digest != &meta_digest[..] {
                warn!("found fraudulent metadata");
                return ok(());
            }

            if let Err(e) = validate::<Secp256k1>(&bitcoin_addr, &metadata) {
                warn!("peer supplied invalid metadata {:?}", e);
                return ok(());
            }

            if let Err(e) = key_db_inner.clone().put(&bitcoin_addr, &metadata) {
                error!("failed to put peer metadata {}", e);
            };
            ok(())
        }))
    });
    actix_rt::Arbiter::current().send(peer_polling.map_err(|e| {
        error!("{:?}", e);
        ()
    }));

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
    .start();

    sys.run().expect("failed to run");
}
