use crate::config::BitcoinConfig;
use crate::model_v1::{BlockHash, ExplorerError, Height};
use bitcoin::hashes::Hash;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use chain::api::SizeLike;
use redbit::retry::retry_with_delay_sync;
use std::sync::Arc;
use std::time::Duration;

pub struct BtcCBOR {
    pub hex: String,
    pub height: Height // bip34 hack
}

impl SizeLike for BtcCBOR {
    fn size(&self) -> usize {
        self.hex.len()
    }
}

pub struct BtcClient {
    rpc_client: Arc<Client>,
}

impl BtcClient {
    pub fn new(bitcoin_config: &BitcoinConfig) -> Result<Self, ExplorerError> {
        let user_pass = Auth::UserPass(
            bitcoin_config.api_username.clone(),
            bitcoin_config.api_password.clone(),
        );

        let url = bitcoin_config.api_host.clone(); // e.g. "http://example.com:8332"
        let client = Client::new(&url, user_pass)?;
        let rpc_client = Arc::new(client);
        Ok(BtcClient { rpc_client })
    }
}

impl BtcClient {
    pub fn get_best_block(&self) -> Result<BtcCBOR, ExplorerError> {
        retry_with_delay_sync(3, Duration::from_millis(1000), || {
            let best_block_hash = self.rpc_client.get_best_block_hash()?;
            let best_block_hex = self.rpc_client.get_block_hex(&best_block_hash)?;
            let verbose_block = self.rpc_client.get_block_info(&best_block_hash)?;
            let height = Height(verbose_block.height as u32);
            Ok(BtcCBOR { height, hex: best_block_hex })
        })

    }

    pub fn get_block_by_hash(&self, hash: BlockHash) -> Result<BtcCBOR, ExplorerError> {
        let bitcoin_hash = bitcoin::BlockHash::from_raw_hash(Hash::from_byte_array(hash.0));
        retry_with_delay_sync(3, Duration::from_millis(1000), || {
            let block_hex = self.rpc_client.get_block_hex(&bitcoin_hash)?;
            let verbose_block = self.rpc_client.get_block_info(&bitcoin_hash)?;
            let height = Height(verbose_block.height as u32);
            Ok(BtcCBOR { height, hex: block_hex })
        })
    }

    pub fn get_block_by_height(&self, height: Height) -> Result<BtcCBOR, ExplorerError> {
        retry_with_delay_sync(3, Duration::from_millis(1000), || {
            let block_hash = self.rpc_client.get_block_hash(height.0 as u64)?;
            let block_hex = self.rpc_client.get_block_hex(&block_hash)?;
            Ok(BtcCBOR{ height, hex: block_hex })
        })
    }
}
