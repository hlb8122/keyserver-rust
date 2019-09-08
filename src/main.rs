#[macro_use]
extern crate clap;

pub mod bitcoin;
pub mod crypto;
pub mod db;
pub mod net;
pub mod settings;

use std::io;

use actix_web::{middleware::Logger, web, App, HttpServer};
use env_logger::Env;
use futures::Future;
use lazy_static::lazy_static;
use log::{error, info};

use crate::{bitcoin::tx_stream, db::KeyDB, net::*, settings::Settings};

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

    // Http client
    let http_client = reqwest::r#async::Client::new();

    // Init ZMQ
    let (tx_stream, connection) =
        tx_stream::get_tx_stream(&format!("tcp://{}:{}", SETTINGS.node_ip, SETTINGS.zmq_port));
    let key_stream = tx_stream::extract_details(tx_stream);
    actix_rt::Arbiter::current().send(connection.map_err(|e| error!("{:?}", e)));

    // Peer client
    let peer_client = peer::PeerClient::default();

    // Setup peer polling logic
    actix_rt::Arbiter::current().send(peer_client.peer_polling(key_db.clone(), key_stream));

    // Init REST server
    HttpServer::new(move || {
        let key_db_inner = key_db.clone();
        let http_client_inner = http_client.clone();

        // Init app
        App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .service(
                // Key scope
                web::scope("/keys").service(
                    web::resource("/{addr}")
                        .data(key_db_inner) // Apply payment check to put key
                        .data(http_client_inner)
                        .route(web::get().to(get_key))
                        .route(web::put().to_async(put_key)),
                ),
            )
            .service(actix_files::Files::new("/", "./static/").index_file("index.html"))
    })
    .bind(&SETTINGS.bind)?
    .start();

    sys.run()
}
