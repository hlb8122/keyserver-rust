#[macro_use]
extern crate clap;

pub mod authentication;
pub mod bitcoin;
pub mod crypto;
pub mod db;
pub mod server;
pub mod settings;

use hyper::Server;
use lazy_static::lazy_static;

use crate::{db::Database, settings::Settings};

lazy_static! {
    pub static ref SETTINGS: Settings = Settings::new().expect("couldn't load config");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Maybe fix this error type
    // Init logging
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .init();

    // Open DB
    let key_db = Database::try_new(&SETTINGS.db_path)?;

    // Init ZMQ
    // let (tx_stream, connection) =
    //     tx_stream::get_tx_stream(&format!("tcp://{}:{}", SETTINGS.node_ip, SETTINGS.zmq_port));
    // let key_stream = tx_stream::extract_details(tx_stream);
    // tokio::spawn(connection.map_err(|e| error!("{:?}", e)));

    // Init server
    tracing::info!(message = "starting server", addr = %SETTINGS.bind);
    let addr = SETTINGS.bind.parse()?;

    let keyserver = server::Keyserver::new(key_db);

    let server = Server::bind(&addr).serve(keyserver);

    Ok(())
}
