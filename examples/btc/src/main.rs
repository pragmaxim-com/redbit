use anyhow::Result;
use btc::block_provider::BtcBlockProvider;
use btc::model_v1::BlockChain;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(BtcBlockProvider::new()?, BlockChain::new, None, None).await?;
    Ok(())
}
