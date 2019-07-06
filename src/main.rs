pub mod authentication;
pub mod crypto;
pub mod db;
pub mod jsonrpc_client;
pub mod net;
pub mod token;

use actix_web::{middleware::Logger, web, App, HttpServer};

use crate::{db::KeyDB, net::rest_server::*};

pub mod models {
    include!(concat!(env!("OUT_DIR"), "/models.rs"));
}

const BIND_ADDR: &str = "127.0.0.1:8080";

fn main() {
    println!("starting server @ {}", BIND_ADDR);

    // Init logging
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    // Open DB
    let key_db = KeyDB::try_default().unwrap();

    // Init REST server
    HttpServer::new(move || {
        let key_db_inner = key_db.clone();
        App::new()
            .data(State(key_db_inner))
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .service(
                web::scope("/keys").service(
                    web::resource("/{addr}")
                        .route(web::get().to(get_key))
                        .route(web::put().to(put_key)),
                ),
            )
            .service(actix_files::Files::new("/", "./static/").index_file("index.html"))
    })
    .bind(BIND_ADDR)
    .expect("Can not bind to port 8080")
    .run()
    .unwrap();
}
