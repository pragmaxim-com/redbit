use demo::block_provider::DemoBlockProvider;
use demo::model_v1::{Height, Header};
use chain::err::ChainError;

#[tokio::test]
async fn demo_provider_tip_height_matches_target() -> Result<(), ChainError> {
    let target_height = 50u32;
    let provider = DemoBlockProvider::for_height(target_height, 1024)?;
    let tip: Header = provider.get_chain_tip().await?;
    assert_eq!(tip.height, Height(target_height));
    Ok(())
}
