use anyhow::Result;
use bch::block_provider::BchBlockProvider;
use bch::model_v1::BlockChain;
use chain::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch_sync(BchBlockProvider::new, BlockChain::new, None, None).await?;
    Ok(())
}
