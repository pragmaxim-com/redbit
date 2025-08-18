use anyhow::Result;
use ergo::block_provider::ErgoBlockProvider;
use ergo::model_v1::*;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(ErgoBlockProvider::new()?, BlockChain::new, None, None).await?;
    Ok(())
}
