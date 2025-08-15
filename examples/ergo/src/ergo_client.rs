use crate::model_v1::{BlockHash, ExplorerError, Height};
use ergo_lib::chain::block::FullBlock;
use redbit::retry::retry_with_delay;
use reqwest::{
    blocking, header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE}, Client,
    Url,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Default, Serialize, Deserialize, PartialEq, Eq, Debug, Clone)]
#[repr(C)]
pub struct NodeInfo {
    pub name: String,
    #[serde(rename = "appVersion")]
    pub app_version: String,
    #[serde(rename = "fullHeight")]
    pub full_height: u32,
}

pub struct ErgoClient {
    node_url: Url,
    http: Client,
    headers: HeaderMap,
}

impl ErgoClient {
    pub fn new(node_url: Url, api_key: String) -> Result<Self, ExplorerError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let api_key = HeaderValue::from_str(&api_key)?;
        headers.insert("api_key", api_key.clone());

        let http = Client::builder()
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Some(Duration::from_secs(30)))
            //.http2_adaptive_window(true)
            .timeout(Duration::from_secs(10))
            .default_headers(headers.clone())
            .build()?;


        Ok(Self { node_url, http, headers: headers.clone() })
    }

    pub(crate) async fn get_block_by_height_retry_async(&self, height: Height) -> Result<FullBlock, ExplorerError> {
        retry_with_delay(5, Duration::from_millis(1000), || {
            let height = height.clone();
            async move {
                let block_ids = self.get_block_ids_by_height_async(height).await?;
                self.get_block_by_hash_async(block_ids.first().unwrap()).await
            }
        }).await
    }

    pub(crate) async fn get_block_by_height_async(&self, height: Height) -> Result<FullBlock, ExplorerError> {
        let block_ids = self.get_block_ids_by_height_async(height).await?;

        self.get_block_by_hash_async(block_ids.first().unwrap()).await
    }

    pub(crate) async fn get_node_info_async(&self) -> Result<NodeInfo, ExplorerError> {
        let node_info_url: Url = self.node_url.join("info")?;
        let response = self.http.get(node_info_url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_else(|_| String::new());
            let error = format!("Request failed with status {}: {}", status, text);
            Err(ExplorerError::Custom(error))
        } else {
            let result = response.json::<NodeInfo>().await?;
            Ok(result)
        }
    }

    pub(crate) async fn get_best_block_async(&self) -> Result<FullBlock, ExplorerError> {
        let node_info = self.get_node_info_async().await?;
        self.get_block_by_height_async(Height(node_info.full_height)).await
    }

    pub async fn get_block_ids_by_height_async(&self, height: Height) -> Result<Vec<String>, ExplorerError> {
        let block_ids_url = self.node_url.join(&format!("blocks/at/{}", &height.0.to_string()))?;
        let block_ids = self.http.get(block_ids_url)
            .send()
            .await?
            .json::<Vec<String>>()
            .await?;
        Ok(block_ids)
    }

    pub(crate) async fn get_block_by_hash_async(&self, block_hash: &str) -> Result<FullBlock, ExplorerError> {
        let url = self.node_url.join(&format!("blocks/{}", block_hash))?;
        let block = self.http.get(url).send().await?.json::<FullBlock>().await?;
        Ok(block)
    }

    // BLOCKING CLIENT

    fn blocking_client(&self) -> blocking::Client {
        blocking::Client::builder()
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Some(Duration::from_secs(30)))
            .timeout(Duration::from_secs(10))
            .default_headers(self.headers.clone())
            .build().unwrap()
    }

    pub fn get_block_ids_by_height_sync(&self, height: Height) -> Result<Vec<String>, ExplorerError> {
        let block_ids_url = self.node_url.join(&format!("blocks/at/{}", &height.0.to_string()))?;
        let block_ids = self.blocking_client().get(block_ids_url).send()?.json::<Vec<String>>()?;
        Ok(block_ids)
    }

    pub fn get_block_by_hash_sync(&self, hash: BlockHash) -> Result<FullBlock, ExplorerError> {
        let url = self.node_url.join(&format!("blocks/{}", hex::encode(hash.0)))?;
        let block = self.blocking_client().get(url).send()?.json::<FullBlock>()?;
        Ok(block)
    }

}

#[cfg(all(test, not(feature = "ci")))]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_info_async() {
        let node_url = Url::from_str("http://naked:9053").unwrap();
        let ergo_client = ErgoClient::new(node_url.clone(), "".to_string()).unwrap();
        let info_response_async = ergo_client.get_node_info_async().await.unwrap();
        println!("name: {}", info_response_async.name);
    }

    #[tokio::test]
    async fn test_block_ids_async() {
        let node_url = Url::from_str("http://naked:9053").unwrap();
        let ergo_client = ErgoClient::new(node_url.clone(), "".to_string()).unwrap();
        let ids_async = ergo_client.get_block_ids_by_height_async(Height(100)).await.unwrap();
        println!("hash: {}", ids_async.first().unwrap());
    }

    #[test]
    fn test_block_ids_sync() {
        let node_url = Url::from_str("http://naked:9053").unwrap();
        let ergo_client = ErgoClient::new(node_url.clone(), "".to_string()).unwrap();
        let ids_sync = ergo_client.get_block_ids_by_height_sync(Height(100)).unwrap();
        println!("hash: {}", ids_sync.first().unwrap());
    }

    #[tokio::test]
    async fn test_block_async() {
        let node_url = Url::from_str("http://naked:9053").unwrap();
        let ergo_client = ErgoClient::new(node_url.clone(), "".to_string()).unwrap();
        let ids_async = ergo_client.get_block_ids_by_height_async(Height(100)).await.unwrap();
        let block = ergo_client.get_block_by_hash_async(ids_async.first().unwrap()).await.unwrap();
        println!("height: {}", block.header.height);
    }

    #[test]
    fn test_block_sync() {
        let node_url = Url::from_str("http://naked:9053").unwrap();
        let ergo_client = ErgoClient::new(node_url.clone(), "".to_string()).unwrap();
        let ids_sync = ergo_client.get_block_ids_by_height_sync(Height(100)).unwrap();
        let block_hash = BlockHash(hex::decode(ids_sync.first().unwrap()).unwrap().try_into().unwrap());
        let block = ergo_client.get_block_by_hash_sync(block_hash).unwrap();
        println!("height: {}", block.header.height);
    }

    #[test]
    fn test_deserialization() {
        let json_data = r#"
        {
          "currentTime" : 1723784804691,
          "network" : "mainnet",
          "name" : "ergo-mainnet-5.0.22",
          "stateType" : "utxo",
          "difficulty" : 1115063704354816,
          "bestFullHeaderId" : "db5095ab785ea515ec2fc76e1d890bec4d88318c118d9561fb4bb7f6069fbecb",
          "bestHeaderId" : "db5095ab785ea515ec2fc76e1d890bec4d88318c118d9561fb4bb7f6069fbecb",
          "peersCount" : 30,
          "unconfirmedCount" : 4,
          "appVersion" : "5.0.22",
          "eip37Supported" : true,
          "stateRoot" : "15dc211165746cc0625ae9c62ad8f4309c8983b36279a349207e09099beb857619",
          "genesisBlockId" : "b0244dfc267baca974a4caee06120321562784303a8a688976ae56170e4d175b",
          "previousFullHeaderId" : "c45b1984c7ed6e77c7955c22fa074e657dec7bb7141b5044f1d3b5c273c26897",
          "fullHeight" : 1331111,
          "headersHeight" : 1331111,
          "stateVersion" : "db5095ab785ea515ec2fc76e1d890bec4d88318c118d9561fb4bb7f6069fbecb",
          "fullBlocksScore" : 2396498399696617734144,
          "maxPeerHeight" : 1331111,
          "launchTime" : 1723784167386,
          "isExplorer" : false,
          "lastSeenMessageTime" : 1723784781950,
          "eip27Supported" : true,
          "headersScore" : 2396498399696617734144,
          "parameters" : {
            "outputCost" : 214,
            "tokenAccessCost" : 100,
            "maxBlockCost" : 8001091,
            "height" : 1330176,
            "maxBlockSize" : 1271009,
            "dataInputCost" : 100,
            "blockVersion" : 3,
            "inputCost" : 2407,
            "storageFeeFactor" : 1250000,
            "minValuePerByte" : 360
          },
          "isMining" : false
        }"#;

        // Deserialize the JSON data
        let node_info: NodeInfo = redbit::serde_json::from_str(json_data).expect("Failed to deserialize");

        // Expected NodeInfo struct
        let expected_node_info = NodeInfo { name: "ergo-mainnet-5.0.22".to_string(), app_version: "5.0.22".to_string(), full_height: 1331111 };

        // Assert that the deserialized data matches the expected struct
        assert_eq!(node_info, expected_node_info);
    }
}
