use anyhow::Result;
use btc::block_chain::BtcBlockChain;
use btc::block_provider::BtcBlockProvider;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(BtcBlockProvider::new()?, BtcBlockChain::new, None, None).await?;
    Ok(())
}
