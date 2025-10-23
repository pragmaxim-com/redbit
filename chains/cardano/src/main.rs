use cardano::block_provider::CardanoBlockProvider;
use anyhow::Result;
use cardano::model_v1::BlockChain;
use chain::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch_async(CardanoBlockProvider::new, BlockChain::new, None, None).await?;
    Ok(())
}
