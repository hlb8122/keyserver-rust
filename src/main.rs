pub mod authentication;
pub mod crypto;
pub mod db;
pub mod jsonrpc_client;

pub mod models {
    include!(concat!(env!("OUT_DIR"), "/models.rs"));
}

fn main() {}
