use anyhow::Result;
use btc::block_persistence::BtcBlockPersistence;
use btc::block_provider::BtcBlockProvider;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(BtcBlockProvider::new()?, BtcBlockPersistence::new, None, None).await?;
    Ok(())
}
