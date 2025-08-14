use cardano::block_persistence::CardanoBlockPersistence;
use cardano::block_provider::CardanoBlockProvider;

use anyhow::Result;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(CardanoBlockProvider::new().await, CardanoBlockPersistence::new, None, None).await?;
    Ok(())
}
