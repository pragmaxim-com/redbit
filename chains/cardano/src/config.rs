use config::{Config, ConfigError, Environment, File};
use dotenv::dotenv;
use redbit::info;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct CardanoConfig {
    pub api_host: String,
    pub socket_path: String,
    pub stream_buffer_size: usize,
}

impl CardanoConfig {
    pub fn new(path: &str) -> Result<Self, ConfigError> {
        match dotenv() {
            Ok(_) => {
                let builder = Config::builder()
                    .add_source(File::with_name(path).required(true))
                    .add_source(Environment::with_prefix("CARDANO").try_parsing(true).separator("__"));
                let config = builder.build()?.try_deserialize::<CardanoConfig>()?;
                info!("{:#?}", config);
                Ok(config)

            }
            Err(_) => panic!("Error loading .env file"),
        }
    }
}
