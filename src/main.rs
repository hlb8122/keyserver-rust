#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;

pub mod bitcoin;
pub mod crypto;
pub mod db;
pub mod net;
pub mod settings;

use std::io;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web, App, HttpServer};
use env_logger::Env;
use futures::Future;
use lazy_static::lazy_static;

use crate::{
    bitcoin::{tx_stream, BitcoinClient, WalletState},
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

fn main() -> io::Result<()> {
    let sys = actix_rt::System::new("keyserver");

    // Init logging
    env_logger::from_env(Env::default().default_filter_or("actix_web=info,keyserver=info")).init();
    info!("starting server @ {}", SETTINGS.bind);

    // Open DB
    let key_db = KeyDB::try_new(&SETTINGS.db_path).unwrap();

    // Init wallet
    let wallet_state = WalletState::default();

    // Init Bitcoin client
    let bitcoin_client = BitcoinClient::new(
        format!("http://{}:{}", SETTINGS.node_ip.clone(), SETTINGS.rpc_port),
        SETTINGS.rpc_username.clone(),
        SETTINGS.rpc_password.clone(),
    );

    // Init ZMQ
    let (tx_stream, connection) =
        tx_stream::get_tx_stream(&format!("tcp://{}:{}", SETTINGS.node_ip, SETTINGS.zmq_port));
    let key_stream = tx_stream::extract_details(tx_stream);
    actix_rt::Arbiter::current().send(connection.map_err(|e| error!("{:?}", e)));

    // Peer client
    let client = peer::PeerClient::default();

    // Setup peer polling logic
    actix_rt::Arbiter::current().send(client.peer_polling(key_db.clone(), key_stream));

    // Init REST server
    HttpServer::new(move || {
        let key_db_inner = key_db.clone();
        let wallet_state_inner = wallet_state.clone();
        let bitcoin_client_inner = bitcoin_client.clone();

        // Init app
        App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .wrap(Cors::new())
            .service(
                // Key scope
                web::scope("/keys").service(
                    web::resource("/{addr}")
                        .data(key_db_inner)
                        .wrap(CheckPayment::new(
                            bitcoin_client_inner.clone(),
                            wallet_state_inner.clone(),
                        )) // Apply payment check to put key
                        .route(web::get().to(get_key))
                        .route(web::put().to_async(put_key)),
                ),
            )
            .service(
                // Payment endpoint
                web::resource("/payments")
                    .data((bitcoin_client_inner, wallet_state_inner))
                    .route(web::post().to_async(payment_handler)),
            )
            .service(actix_files::Files::new("/", "./static/").index_file("index.html"))
    })
    .bind(&SETTINGS.bind)?
    .start();

    sys.run()
}
