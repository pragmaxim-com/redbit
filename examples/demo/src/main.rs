use anyhow::Result;
use demo::storage::empty_temp_storage;
use demo::*;
use redbit::*;
use std::net::SocketAddr;
use std::sync::Arc;
use syncer::settings::HttpSettings;
use syncer::{combine, launcher};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> Result<()> {
    let storage = empty_temp_storage("redbit", 1);
    let extra_routes = OpenApiRouter::new().routes(utoipa_axum::routes!(routes::test_json_nl_stream));
    let http_settings = HttpSettings {
        enable: true,
        bind_address: SocketAddr::from(([127,0,0,1], 8000)),
    };
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let indexing_f = run::with_db(Arc::clone(&storage));
    let server_f = launcher::maybe_run_server(http_settings, Arc::clone(&storage), Some(extra_routes), None, shutdown_rx.clone());

    combine::futures(indexing_f, server_f, shutdown_tx).await;
    Ok(())
}