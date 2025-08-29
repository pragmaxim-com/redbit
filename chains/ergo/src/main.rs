use anyhow::Result;
use ergo::block_provider::ErgoBlockProvider;
use chain::launcher;
use ergo::block_chain::BlockChain;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(ErgoBlockProvider::new()?, BlockChain::new, None, None).await?;
    Ok(())
}
