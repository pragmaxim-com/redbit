use ergo::block_persistence::ErgoBlockPersistence;
use ergo::block_provider::ErgoBlockProvider;
use anyhow::Result;
use std::sync::Arc;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(ErgoBlockProvider::new(), ErgoBlockPersistence::new, None, None).await?;
    Ok(())
}
