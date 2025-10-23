use anyhow::Result;
use ergo::block_provider::ErgoBlockProvider;
use chain::launcher;
use ergo::model_v1::BlockChain;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch_sync(ErgoBlockProvider::new, BlockChain::new, None, None).await?;
    Ok(())
}
