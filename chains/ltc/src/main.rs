use anyhow::Result;
use ltc::block_provider::LtcBlockProvider;
use ltc::model_v1::BlockChain;
use chain::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch_sync(LtcBlockProvider::new, BlockChain::new, None, None).await?;
    Ok(())
}
