use config::{Config, ConfigError, Environment, File};
use dotenv::dotenv;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::sync::Once;
use redbit::info;

static DOTENV_ONCE: Once = Once::new();

fn ensure_dotenv_loaded() {
    DOTENV_ONCE.call_once(|| {
        match dotenv() {
            Ok(_) => info!("Config loaded including .env file."),
            Err(_) => info!("Config loaded without .env file."),
        }
    });
}

pub fn load_config<T>(path: &str, prefix: &str) -> Result<T, ConfigError>
where
    T: DeserializeOwned + Debug,
{
    ensure_dotenv_loaded();

    let builder = Config::builder()
        .add_source(File::with_name(path).required(true))
        .add_source(
            Environment::with_prefix(prefix)
                .try_parsing(true)
                .separator("__"),
        );

    let cfg = builder.build()?.try_deserialize::<T>()?;
    info!("{:#?}", cfg);
    Ok(cfg)
}
