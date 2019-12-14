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

use crate::{db::Database, settings::Settings, server::MakeKeyserver};

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
    let db = Database::try_new(&SETTINGS.db_path)?;

    // Init server
    tracing::info!(message = "starting server", addr = %SETTINGS.bind);
    let addr = SETTINGS.bind.parse()?;
    let server = Server::bind(&addr).serve(MakeKeyserver::new(db));

    Ok(())
}
