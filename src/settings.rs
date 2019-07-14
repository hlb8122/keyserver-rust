use clap::App;
use config::{Config, ConfigError, File};
use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub bind: String,
    pub peers: Vec<String>,
    pub secret: String,
    pub dbpath: String,
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
        s.set_default("peers", Vec::<String>::new()).unwrap();
        s.set_default("secret", "b").unwrap();
        let mut default_db = home_dir.clone();
        default_db.push(".keyserver-rust/db");
        s.set_default("dbpath", default_db.to_str()).unwrap();


        // Load config from file
        let mut default_config = home_dir.clone();
        default_config.push(".keyserver-rust/config");
        let default_config_str = default_config.to_str().unwrap();
        let config_path = matches
            .value_of("config")
            .unwrap_or(default_config_str);
        s.merge(File::with_name(config_path).required(false))?;

        // Set bind address from cmd line
        if let Some(bind) = matches.value_of("bind") {
            s.set("bind", bind)?;
        }

        // Set peers from cmd line
        if let Ok(peers) = values_t!(matches, "peers", String) {
            s.set("peers", peers)?;
        }

        // Set secret from cmd line
        if let Some(secret) = matches.value_of("secret") {
            s.set("secret", secret)?;
        }

        // Set db from cmd line
        if let Some(dbpath) = matches.value_of("dbpath") {
            s.set("dbpath", dbpath)?;
        }
        s.try_into()
    }
}
