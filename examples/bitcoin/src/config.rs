use config::{Config, ConfigError, Environment, File};
use dotenv::dotenv;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct BitcoinConfig {
    pub api_host: String,
    pub api_username: String,
    pub api_password: String,
}

impl BitcoinConfig {
    pub fn new(path: &str) -> Result<Self, ConfigError> {
        match dotenv() {
            Ok(_) => {
                let builder = Config::builder()
                    .add_source(File::with_name(path).required(true))
                    .add_source(Environment::with_prefix("BITCOIN").try_parsing(true).separator("__"));
                let config = builder.build()?.try_deserialize();
                println!("{:#?}", config);
                config
            }
            Err(_) => panic!("Error loading .env file"),
        }
    }
}
