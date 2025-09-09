use anyhow::Result;
use demo::block_provider::DemoBlockProvider;
use redbit::*;
use chain::launcher;
use demo::model_v1::BlockChain;
use demo::routes;

#[tokio::main]
async fn main() -> Result<()> {
    let extra_routes = OpenApiRouter::new().routes(utoipa_axum::routes!(routes::test_json_nl_stream));
    launcher::launch(DemoBlockProvider::new(1005)?, BlockChain::new, Some(extra_routes), None).await?;
    Ok(())
}
