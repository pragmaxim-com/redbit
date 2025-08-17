use anyhow::Result;
use demo::block_chain::DemoBlockChain;
use demo::block_provider::DemoBlockProvider;
use demo::*;
use redbit::*;
use syncer::launcher;

#[tokio::main]
async fn main() -> Result<()> {
    info!("Running a showcase");
    run::showcase().await?;

    info!("Syncing with demo chain");
    let extra_routes = OpenApiRouter::new().routes(utoipa_axum::routes!(routes::test_json_nl_stream));
    launcher::launch(DemoBlockProvider::new(1005)?, DemoBlockChain::new, Some(extra_routes), None).await?;
    Ok(())
}
