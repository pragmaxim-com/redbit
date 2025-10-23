use anyhow::Result;
use btc::block_provider::BtcBlockProvider;
use btc::model_v1::BlockChain;
use chain::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch_sync(BtcBlockProvider::new, BlockChain::new, None, None).await?;
    Ok(())
}
