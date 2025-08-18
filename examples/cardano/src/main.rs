use cardano::block_provider::CardanoBlockProvider;
use anyhow::Result;
use cardano::model_v1::BlockChain;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    launcher::launch(CardanoBlockProvider::new().await, BlockChain::new, None, None).await?;
    Ok(())
}
