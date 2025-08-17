use anyhow::Result;
use ergo::block_chain::ErgoBlockChain;
use ergo::block_provider::ErgoBlockProvider;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(ErgoBlockProvider::new()?, ErgoBlockChain::new, None, None).await?;
    Ok(())
}
