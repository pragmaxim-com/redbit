use cardano::block_chain::CardanoBlockChain;
use cardano::block_provider::CardanoBlockProvider;

use anyhow::Result;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(CardanoBlockProvider::new().await, CardanoBlockChain::new, None, None).await?;
    Ok(())
}
