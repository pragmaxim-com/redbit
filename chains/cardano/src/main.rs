use cardano::block_provider::CardanoBlockProvider;
use anyhow::Result;
use cardano::block_chain::BlockChain;
use chain::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(CardanoBlockProvider::new().await, BlockChain::new, None, None).await?;
    Ok(())
}
