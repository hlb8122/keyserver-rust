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
use tower::builder::ServiceBuilder;

use crate::{
    db::{services::*, Database},
    server::MakeKeyserver,
    settings::Settings,
};

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

    // Create services
    let getter = ServiceBuilder::new().service(MetadataGetter::new(db.clone()));
    let putter = ServiceBuilder::new().service(MetadataPutter::new(db));

    // Start server
    tracing::info!(message = "starting server", addr = %SETTINGS.bind);
    let addr = SETTINGS.bind.parse()?;

    let make_keyserver = MakeKeyserver::new(getter, putter);
    let server = Server::bind(&addr).serve(make_keyserver);
    server.await?;

    Ok(())
}
