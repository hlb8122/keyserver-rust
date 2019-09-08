use clap::App;
use config::{Config, ConfigError, File};
use serde_derive::Deserialize;

use crate::bitcoin::Network;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bind: String,
    pub zmq_addr: String,
    pub secret: String,
    pub db_path: String,
    pub payment_server_url: String,
    pub network: Network,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();

        // Set defaults
        let yaml = load_yaml!("cli.yml");
        let matches = App::from_yaml(yaml).get_matches();
        let home_dir = match dirs::home_dir() {
            Some(some) => some,
            None => return Err(ConfigError::Message("no home directory".to_string())),
        };
        s.set_default("bind", "127.0.0.1:8080").unwrap();
        s.set_default("zmq_addr", "tcp://localhost:28332").unwrap();
        s.set_default("secret", "secret").unwrap();
        let mut default_db = home_dir.clone();
        default_db.push(".keyserver-rust/db");
        s.set_default("db_path", default_db.to_str()).unwrap();
        s.set_default("payment_server_url", "http://127.0.0.1:8900")
            .unwrap();
        s.set_default("network", "regnet").unwrap();

        // Load config from file
        let mut default_config = home_dir.clone();
        default_config.push(".keyserver-rust/config");
        let default_config_str = default_config.to_str().unwrap();
        let config_path = matches.value_of("config").unwrap_or(default_config_str);
        s.merge(File::with_name(config_path).required(false))?;

        // Set bind address from cmd line
        if let Some(bind) = matches.value_of("bind") {
            s.set("bind", bind)?;
        }

        // Set ZMQ address from cmd line
        if let Ok(node_zmq_port) = value_t!(matches, "zmq-addr", i64) {
            s.set("zmq_addr", node_zmq_port)?;
        }

        // Set secret from cmd line
        if let Some(secret) = matches.value_of("secret") {
            s.set("secret", secret)?;
        }

        // Set DB from cmd line
        if let Some(db_path) = matches.value_of("db-path") {
            s.set("db_path", db_path)?;
        }

        // Set payment server URL from cmd line
        if let Some(db_path) = matches.value_of("payment-server-url") {
            s.set("payment_server_url", db_path)?;
        }

        // Set the bitcoin network
        if let Some(db_path) = matches.value_of("network") {
            s.set("network", db_path)?;
        }

        s.try_into()
    }
}
