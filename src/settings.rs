use clap::App;
use config::{Config, ConfigError, File};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bind: String,
    pub node_ip: String,
    pub node_rpc_port: u16,
    pub node_username: String,
    pub node_password: String,
    pub node_zmq_port: u16,
    pub secret: String,
    pub db_path: String,
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
        s.set_default("bind", "0.0.0.0:8080").unwrap();
        s.set_default("node_ip", "127.0.0.1").unwrap();
        s.set_default("node_rpc_port", "8332").unwrap();
        s.set_default("node_username", "").unwrap();
        s.set_default("node_password", "").unwrap();
        s.set_default("node_zmq_port", "8332").unwrap();
        s.set_default("secret", "b").unwrap();
        let mut default_db = home_dir.clone();
        default_db.push(".keyserver-rust/db");
        s.set_default("db_path", default_db.to_str()).unwrap();

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

        // Set node IP from cmd line
        if let Some(node_ip) = matches.value_of("node-ip") {
            s.set("node-ip", node_ip)?;
        }

        // Set rpc port from cmd line
        if let Ok(node_rpc_port) = value_t!(matches, "rpc-port", i64) {
            s.set("node_rpc_port", node_rpc_port)?;
        }

        // Set rpc username from cmd line
        if let Some(node_rpc_username) = matches.value_of("rpc-username") {
            s.set("rpc-username", node_rpc_username)?;
        }

        // Set rpc password from cmd line
        if let Some(node_rpc_password) = matches.value_of("rpc-password") {
            s.set("rpc-password", node_rpc_password)?;
        }

        // Set zmq port from cmd line
        if let Ok(node_zmq_port) = value_t!(matches, "zmq-port", i64) {
            s.set("node_zmq_port", node_zmq_port)?;
        }

        // Set secret from cmd line
        if let Some(secret) = matches.value_of("secret") {
            s.set("secret", secret)?;
        }

        // Set db from cmd line
        if let Some(db_path) = matches.value_of("db-path") {
            s.set("db_path", db_path)?;
        }
        s.try_into()
    }
}
