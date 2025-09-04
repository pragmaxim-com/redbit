use crate::api::{BlockChainLike, BlockLike, BlockProvider, SizeLike};
use crate::scheduler::Scheduler;
use crate::settings::{AppConfig, HttpSettings, IndexerSettings};
use crate::{combine, ChainError};
use futures::future::ready;
use redbit::storage::Storage;
use redbit::{error, info, serve, OpenApiRouter, RequestState};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tower_http::cors;
use tower_http::cors::CorsLayer;
use crate::syncer::ChainSyncer;

pub async fn maybe_run_server(
    http_conf: HttpSettings,
    storage: Arc<Storage>,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
    shutdown: watch::Receiver<bool>,
) -> () {
    if http_conf.enable {
        info!("Starting http server at {}", http_conf.bind_address);
        let cors = cors.unwrap_or_else(|| CorsLayer::new()
            .allow_origin(cors::Any) // or use a specific origin: `AllowOrigin::exact("http://localhost:5173".parse().unwrap())`
            .allow_methods(cors::Any)
            .allow_headers(cors::Any));
        serve(RequestState { storage: Arc::clone(&storage) }, http_conf.bind_address, extras, Some(cors), shutdown).await
    } else {
        info!("HTTP server is disabled, skipping");
        ready(()).await
    }
}

pub async fn maybe_run_syncing<FB: SizeLike + 'static, TB: BlockLike + 'static>(
    index_config: &IndexerSettings,
    last_header: Option<TB::Header>,
    syncer: Arc<ChainSyncer<FB, TB>>,
    shutdown: watch::Receiver<bool>,
) -> () {
    if index_config.enable {
        info!("Syncing initiated");
        match syncer.sync(index_config, last_header, shutdown).await {
            Ok(_) => info!("Syncing completed successfully"),
            Err(e) => error!("Syncing failed: {}", e),
        }
    } else {
        info!("Syncing is disabled, skipping");
        ready(()).await
    }
}

pub async fn maybe_run_scheduling<FB: SizeLike + 'static, TB: BlockLike + 'static>(
    index_config: IndexerSettings,
    scheduler: Scheduler<FB, TB>,
    shutdown: watch::Receiver<bool>,
) -> () {
    if index_config.enable && index_config.sync_interval_s.gt(&Duration::ZERO) {
        info!("Scheduling initiated");
        scheduler.schedule(&index_config, shutdown).await
    } else {
        info!("Scheduling is disabled as sync_interval_s = 0, skipping");
        ready(()).await
    }
}

#[cfg(feature = "tracing")]
fn maybe_console_init() {
    info!("Running developer build with console subscriber");
    console_subscriber::init();
}

#[cfg(not(feature = "tracing"))]
fn maybe_console_init() {
    info!("Running production build without console subscriber");
}

pub async fn launch<FB: SizeLike + 'static, TB: BlockLike + 'static, F>(
    block_provider: Arc<dyn BlockProvider<FB, TB>>,
    build_chain: F,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
) -> Result<(), ChainError>
where
    F: FnOnce(Arc<Storage>) -> Arc<dyn BlockChainLike<TB>>,
{
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    maybe_console_init();
    let db_path: String = format!("{}/{}/{}", config.indexer.db_path, "main", config.indexer.name);
    let full_path = env::home_dir().unwrap().join(&db_path);
    let (created, storage) = Storage::init(full_path, config.indexer.db_cache_size_gb)?;
    let chain: Arc<dyn BlockChainLike<TB>> = build_chain(Arc::clone(&storage));
    let unlinked_headers: Vec<TB::Header> =
        if created {
            chain.init()?;
            Vec::new()
        } else {
            info!("Validating chain for being linked");
            chain.validate_chain(config.indexer.validation_from_height).await?
        };
    let syncer: Arc<ChainSyncer<FB, TB>> = Arc::new(ChainSyncer::new(Arc::clone(&block_provider), Arc::clone(&chain)));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    // we sync with the node first
    maybe_run_syncing(&config.indexer, unlinked_headers.first().cloned(), Arc::clone(&syncer), shutdown_rx.clone()).await;
    // then we sync periodically with chain tip and start server
    let indexing_f = maybe_run_scheduling(config.indexer, Scheduler::new(Arc::clone(&syncer)), shutdown_rx.clone());
    let server_f = maybe_run_server(config.http, Arc::clone(&storage), extras, cors, shutdown_rx.clone());
    Ok(combine::futures(indexing_f, server_f, shutdown_tx).await)
}
