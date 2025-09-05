use chain::settings::Parallelism;
use config::{Config, ConfigError, Environment, File};
use dotenv::dotenv;
use redbit::info;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ErgoConfig {
    pub api_host: String,
    pub api_key: String,
    pub fetching_parallelism: Parallelism,
}

impl ErgoConfig {
    pub fn new(path: &str) -> Result<Self, ConfigError> {
        match dotenv() {
            Ok(_) => {
                let builder = Config::builder()
                    .add_source(File::with_name(path).required(true))
                    .add_source(Environment::with_prefix("ERGO").try_parsing(true).separator("__"));
                let config = builder.build()?.try_deserialize::<ErgoConfig>()?;
                info!("{:#?}", config);
                Ok(config)
            }
            Err(_) => panic!("Error loading .env file"),
        }
    }
}
