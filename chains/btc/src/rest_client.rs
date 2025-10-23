use crate::model_v1::{BlockHash, Height};
use chain::api::SizeLike;
use redbit::retry::{retry_with_delay_async, retry_with_delay_sync};
use reqwest::{blocking, Client};
use std::sync::Arc;
use std::time::Duration;
use chain::ChainError;
use chain::block_stream::RestClient;
use crate::{BitcoinConfig, ExplorerError};

pub struct BtcCBOR {
    pub raw: Vec<u8>,
    pub height: Height, // bip34 hack
}

impl SizeLike for BtcCBOR {
    fn size(&self) -> usize {
        self.raw.len()
    }
}

#[derive(Clone)]
pub struct BtcClient {
    http_client: Arc<Client>,
    base_url: String,
}

#[async_trait::async_trait]
impl RestClient<BtcCBOR> for BtcClient {
    async fn get_block_by_height(&self, height: u32) -> Result<BtcCBOR, ChainError> {
        let cbor = self.get_block_by_height(Height(height)).await?;
        Ok(cbor)
    }
}

impl BtcClient {
    pub fn new(bitcoin_config: &BitcoinConfig) -> Result<Self, ExplorerError> {
        let http_client = Arc::new(Client::new());
        Ok(BtcClient {
            http_client,
            base_url: bitcoin_config.api_host.clone(),
        })
    }

    fn blocking_client(&self) -> blocking::Client {
        blocking::Client::builder()
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .timeout(Duration::from_secs(10))
            .build().unwrap()
    }


    fn block_url_bin(base_url: &str, hash_hex: &str) -> String {
        format!("{}/rest/block/{}.bin", base_url, hash_hex)
    }

    fn block_url_json(base_url: &str, hash_hex: &str) -> String {
        format!("{}/rest/block/{}.json", base_url, hash_hex)
    }

    fn parse_height(json: &serde_json::Value) -> Result<Height, ExplorerError> {
        json["height"]
            .as_u64()
            .map(|h| Height(h as u32))
            .ok_or_else(|| ExplorerError::Custom("missing height".into()))
    }

    fn parse_blockhash(json: &serde_json::Value) -> Result<String, ExplorerError> {
        json["blockhash"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ExplorerError::Custom("missing blockhash".into()))
    }

    pub async fn get_best_block(&self) -> Result<BtcCBOR, ExplorerError> {
        retry_with_delay_async(3, Duration::from_millis(1000), || async {
            // 1) Get the best block hash from chaininfo
            let chaininfo_url = format!("{}/rest/chaininfo.json", self.base_url);
            let chaininfo: serde_json::Value = self.http_client.get(&chaininfo_url).send().await?.json().await?;
            let hash = chaininfo["bestblockhash"].as_str().ok_or_else(|| ExplorerError::Custom("missing bestblockhash".into()))?;
            // 2) Fetch raw block
            let raw = self.get_block_raw_by_hash_async(hash).await?;
            // 3) Fetch height
            let height_url = format!("{}/rest/block/{}.json", self.base_url, hash);
            let verbose: serde_json::Value = self.http_client.get(&height_url).send().await?.json().await?;
            let height = verbose["height"].as_u64().ok_or_else(|| ExplorerError::Custom("missing height".into()))? as u32;

            Ok(BtcCBOR { raw, height: Height(height) })
        }).await
    }

    async fn get_block_raw_by_hash_async(&self, hash: &str) -> Result<Vec<u8>, ExplorerError> {
        let url = Self::block_url_bin(&self.base_url, hash);
        Ok(self.http_client.get(&url).send().await?.bytes().await?.to_vec())
    }

    fn get_block_raw_by_hash_sync(&self, hash: &BlockHash) -> Result<Vec<u8>, ExplorerError> {
        let url = Self::block_url_bin(&self.base_url, &hex::encode(hash.0));
        Ok(self.blocking_client().get(&url).send()?.bytes()?.to_vec())
    }

    #[allow(dead_code)]
    async fn get_block_height_by_hash_async(&self, hash: &BlockHash) -> Result<Height, ExplorerError> {
        let url = Self::block_url_json(&self.base_url, &hex::encode(hash.0));
        let verbose = self.http_client.get(&url).send().await?.json::<serde_json::Value>().await?;
        Self::parse_height(&verbose)
    }

    fn get_block_height_by_hash_sync(&self, hash: &BlockHash) -> Result<Height, ExplorerError> {
        let url = Self::block_url_json(&self.base_url, &hex::encode(hash.0));
        let verbose = self.blocking_client().get(&url).send()?.json::<serde_json::Value>()?;
        Self::parse_height(&verbose)
    }

    pub fn get_block_by_hash_str_sync(&self, hash: BlockHash) -> Result<BtcCBOR, ExplorerError> {
        retry_with_delay_sync(3, Duration::from_millis(1000), || {
            Ok(BtcCBOR {
                raw: self.get_block_raw_by_hash_sync(&hash)?,
                height: self.get_block_height_by_hash_sync(&hash)?,
            })
        })
    }

    pub async fn get_block_by_height(&self, height: Height) -> Result<BtcCBOR, ExplorerError> {
        retry_with_delay_async(3, Duration::from_millis(1000), || async {
            let url = format!("{}/rest/blockhashbyheight/{}.json", self.base_url, height.0);
            let hash_json = self.http_client.get(&url).send().await?.json::<serde_json::Value>().await?;
            let hash = Self::parse_blockhash(&hash_json)?;
            Ok(BtcCBOR {
                raw: self.get_block_raw_by_hash_async(&hash).await?,
                height,
            })
        }).await
    }

}
