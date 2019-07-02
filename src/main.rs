pub mod db;
pub mod crypto;
pub mod models {
    include!(concat!(env!("OUT_DIR"), "/models.rs"));
}

fn main() {}
