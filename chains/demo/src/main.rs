use anyhow::Result;
use demo::block_provider::DemoBlockProvider;
use redbit::*;
use chain::launcher;
use demo::manual_chain::build_block_chain_auto;
use demo::routes;

#[tokio::main]
async fn main() -> Result<()> {
    let extra_routes = OpenApiRouter::new().routes(utoipa_axum::routes!(routes::test_json_nl_stream));
    launcher::launch_sync(DemoBlockProvider::new, build_block_chain_auto, Some(extra_routes), None).await?;
    Ok(())
}
