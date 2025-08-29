use anyhow::Result;
use btc::block_chain::BlockChain;
use btc::block_provider::BtcBlockProvider;
use chain::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(BtcBlockProvider::new()?, BlockChain::new, None, None).await?;
    Ok(())
}
