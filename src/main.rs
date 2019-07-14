#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

pub mod authentication;
pub mod bitcoin;
pub mod crypto;
pub mod db;
pub mod net;
pub mod settings;

use actix_web::{middleware::Logger, web, App, HttpServer};
use lazy_static::lazy_static;

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

fn main() {
    // Init config

    println!("starting server @ {}", SETTINGS.bind);

    // Init logging
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    // Open DB
    let key_db = KeyDB::try_new(&SETTINGS.dbpath).unwrap();

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
