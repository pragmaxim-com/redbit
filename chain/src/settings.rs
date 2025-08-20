use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Parallelism {
    Off,
    Low,
    Mild,
    High,
}

impl FromStr for Parallelism {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Parallelism::Off),
            "low" => Ok(Parallelism::Low),
            "mild" => Ok(Parallelism::Mild),
            "high" => Ok(Parallelism::High),
            _ => Err(format!("Invalid value for Parallelism: {}", s)),
        }
    }
}

impl From<Parallelism> for usize {
    fn from(parallelism: Parallelism) -> Self {
        match parallelism {
            Parallelism::Off => 1,
            Parallelism::Low => num_cpus::get() / 8,
            Parallelism::Mild => num_cpus::get() / 4,
            Parallelism::High => num_cpus::get() / 2,
        }
    }
}

impl<'de> serde::Deserialize<'de> for Parallelism {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Parallelism::from_str(&s).map_err(serde::de::Error::custom)
    }
}

fn duration_from_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let secs = u64::deserialize(deserializer)?;
    Ok(Duration::from_secs(secs))
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub indexer: IndexerSettings,
    pub http: HttpSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct IndexerSettings {
    pub name: String,
    pub enable: bool,
    pub db_path: String,
    #[serde(deserialize_with = "duration_from_secs")]
    pub sync_interval_s: Duration,
    pub fork_detection_heights: u8,
    pub min_batch_size: usize,
    pub db_cache_size_gb: u8,
    pub processing_parallelism: Parallelism,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpSettings {
    pub enable: bool,
    pub bind_address: SocketAddr,
}

impl AppConfig {
    pub fn new(path: &str) -> Result<Self, ConfigError> {
        let builder =
            Config::builder()
                .add_source(File::with_name(path).required(true))
                .add_source(Environment::with_prefix("REDBIT").try_parsing(true).separator("__"));
        let config = builder.build()?.try_deserialize();
        println!("{:#?}", config);
        config
    }
}
