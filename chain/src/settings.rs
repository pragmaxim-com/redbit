use serde::Deserialize;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use sysinfo::System;

#[derive(Clone, Copy)]
pub struct Parallelism(pub usize);
impl Parallelism {
    pub fn with_ratio(ratio: Ratio) -> Self {
        let parallelism: usize =
            match ratio {
                Ratio::Off => 1,
                Ratio::Tiny => System::new_all().cpus().len() / 16,
                Ratio::Low => System::new_all().cpus().len() / 8,
                Ratio::Mild => System::new_all().cpus().len() / 4,
                Ratio::High => System::new_all().cpus().len() / 2,
                Ratio::Ultra => System::new_all().cpus().len(),
            };
        Parallelism(core::cmp::max(1, parallelism))
    }
}
impl Debug for Parallelism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} threads", self.0)
    }
}

#[derive(Clone, Copy)]
pub struct DbCacheSize(pub u8);

impl DbCacheSize {
    pub fn with_ratio(ratio: Ratio) -> Self {
        let floor_ratio: u64 = 64;
        let ram_ratio: u64 =
            match ratio {
                Ratio::Off => 0,
                Ratio::Tiny => 64,
                Ratio::Low => 32,
                Ratio::Mild => 16,
                Ratio::High => 8,
                Ratio::Ultra => 4,
            };
        if ram_ratio == 0 {
            DbCacheSize(0)
        } else {
            let bytes = System::new_all().total_memory();
            let total_gib = bytes / (1024 * 1024 * 1024);
            let target = total_gib / ram_ratio;
            let floor = (total_gib / floor_ratio).max(1);
            let gib = core::cmp::max(target, floor);
            DbCacheSize(if gib > u8::MAX as u64 { u8::MAX } else { gib as u8 })
        }
    }
}

impl Debug for DbCacheSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} GB", self.0)
    }
}

#[derive(Debug, Clone)]
pub enum Ratio {
    Off,
    Tiny,
    Low,
    Mild,
    High,
    Ultra,
}

impl FromStr for Ratio {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "off" => Ok(Ratio::Off),
            "tiny" => Ok(Ratio::Tiny),
            "low" => Ok(Ratio::Low),
            "mild" => Ok(Ratio::Mild),
            "high" => Ok(Ratio::High),
            "ultra" => Ok(Ratio::Ultra),
            _ => Err(format!("Invalid value for Parallelism: {}", s)),
        }
    }
}

impl<'de> serde::Deserialize<'de> for DbCacheSize {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let ratio = Ratio::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(DbCacheSize::with_ratio(ratio))
    }
}

impl<'de> serde::Deserialize<'de> for Parallelism {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let ratio = Ratio::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(Parallelism::with_ratio(ratio))
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
    pub node_sync_interval_s: Duration,
    pub fork_detection_heights: u8,
    pub min_entity_batch_size: usize,
    pub non_durable_batches: usize,
    pub db_cache_size_gb: DbCacheSize,
    pub processing_parallelism: Parallelism,
    pub validation_from_height: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpSettings {
    pub enable: bool,
    pub bind_address: SocketAddr,
}
