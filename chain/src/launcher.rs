use crate::api::{BlockChainLike, BlockLike, BlockProvider, SizeLike};
use crate::scheduler::Scheduler;
use crate::settings::{AppConfig, DbCacheSize, HttpSettings, IndexerSettings};
use crate::{chain_config, combine, ChainError};
use futures::future::ready;
use redbit::storage::init::{Storage, StorageOwner};
use redbit::{error, info, AppError, OpenApiRouter, RequestState, WriteTxContext};
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
        redbit::rest::serve(RequestState { storage: Arc::clone(&storage) }, http_conf.bind_address, extras, Some(cors), shutdown).await
    } else {
        info!("HTTP server is disabled, skipping");
        ready(()).await
    }
}

async fn run_initial_sync_phase<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static>(
    indexer_settings: IndexerSettings,
    last_header: Option<TB::Header>,
    syncer: Arc<ChainSyncer<FB, TB, CTX>>,
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
) -> Flow {
    let sync_f = {
        let syncer = Arc::clone(&syncer);
        let rx_in_task = shutdown_rx.clone(); // the task consumes this clone
        let tx_in_task = shutdown_tx.clone(); // the task consumes this clone
        async move {
            if indexer_settings.enable {
                info!("Syncing initiated");
                match syncer.sync(&indexer_settings, last_header, rx_in_task.clone()).await {
                    Ok(_) => info!("Syncing completed successfully"),
                    Err(e) => {
                        error!("Syncing failed: {}", e);
                        let _ = tx_in_task.send_replace(true); // ensure Phase 2 is skipped
                    }
                }
            } else {
                info!("Syncing is disabled, skipping");
                futures::future::ready(()).await
            }
        }
    };

    let ready_f = futures::future::ready(());
    let _ = combine::futures(sync_f, ready_f, shutdown_tx.clone()).await;

    if *shutdown_rx.borrow() { Flow::Stop } else { Flow::Continue }
}

fn teardown<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static>(
    storage_view: Arc<Storage>,
    chain: Arc<dyn BlockChainLike<TB, CTX>>,
    syncer: Arc<ChainSyncer<FB, TB, CTX>>,
    storage_owner: StorageOwner,
) {
    drop(storage_view);
    drop(chain);
    drop(syncer);
    storage_owner.assert_last_refs();
    drop(storage_owner);
    info!("Shutdown complete");
}

pub async fn maybe_run_scheduling<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static>(
    index_config: IndexerSettings,
    scheduler: Scheduler<FB, TB, CTX>,
    shutdown: watch::Receiver<bool>,
) -> () {
    if index_config.enable && index_config.node_sync_interval_s.gt(&Duration::ZERO) {
        info!("Scheduling initiated");
        scheduler.schedule(&index_config, shutdown).await
    } else {
        info!("Scheduling is disabled as node_sync_interval_s = 0, skipping");
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

pub async fn build_storage(config: &AppConfig) -> Result<(bool, StorageOwner, Arc<Storage>), AppError>  {
    let db_path: String = format!("{}/{}/{}", config.indexer.db_path, "main", config.indexer.name);
    let full_path = env::home_dir().unwrap().join(&db_path);
    let db_cache_size_gb: DbCacheSize = config.indexer.db_cache_size_gb;
    StorageOwner::build_storage(full_path, db_cache_size_gb.0).await
}

enum Flow { Continue, Stop }

// ----------------- shared core implementation -----------------
async fn launch_with_provider<
    FB: SizeLike + 'static,
    TB: BlockLike + 'static,
    CTX: WriteTxContext + 'static,
>(
    block_provider: Arc<dyn BlockProvider<FB, TB>>,
    build_chain: impl FnOnce(Arc<Storage>) -> Arc<dyn BlockChainLike<TB, CTX>>,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
) -> Result<(), ChainError>
{
    // load config and init
    let config: AppConfig = chain_config::load_config("config/settings", "REDBIT")?;
    maybe_console_init();
    let (created, storage_owner, storage_view) = build_storage(&config).await?;
    let chain: Arc<dyn BlockChainLike<TB, CTX>> = build_chain(Arc::clone(&storage_view));

    let unlinked_headers: Vec<TB::Header> = if created {
        chain.init()?;
        Vec::new()
    } else {
        info!("Validating chain for being linked");
        chain.validate_chain(config.indexer.validation_from_height).await?
    };

    let syncer: Arc<ChainSyncer<FB, TB, CTX>> = Arc::new(ChainSyncer::new(Arc::clone(&block_provider), Arc::clone(&chain)));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    match run_initial_sync_phase(config.indexer.clone(), unlinked_headers.first().cloned(), Arc::clone(&syncer), shutdown_tx.clone(), shutdown_rx.clone()).await {
        Flow::Stop => {
            teardown::<FB, TB, CTX>(storage_view, chain, syncer, storage_owner);
            Ok(())
        }
        Flow::Continue => {
            let indexing_f = maybe_run_scheduling(config.indexer, Scheduler::new(Arc::clone(&syncer)), shutdown_rx.clone());
            let server_f   = maybe_run_server(config.http, Arc::clone(&storage_view), extras, cors, shutdown_rx.clone());
            let res = combine::futures(indexing_f, server_f, shutdown_tx).await;

            teardown::<FB, TB, CTX>(storage_view, chain, syncer, storage_owner);
            Ok(res)
        }
    }
}

/// For callers that have a synchronous factory `FnOnce(AppConfig) -> Arc<dyn BlockProvider<...>>`
pub async fn launch_sync<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static, CFN, PFN>(
    block_provider_factory: PFN,
    build_chain: CFN,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
) -> Result<(), ChainError>
where
    CFN: FnOnce(Arc<Storage>) -> Arc<dyn BlockChainLike<TB, CTX>>,
    PFN: FnOnce(AppConfig) -> Result<Arc<dyn BlockProvider<FB, TB>>, ChainError>,
{
    let config: AppConfig = chain_config::load_config("config/settings", "REDBIT")?;
    let provider = block_provider_factory(config)?;
    launch_with_provider::<FB, TB, CTX>(provider, build_chain, extras, cors).await
}

/// For callers that have an async factory `FnOnce(AppConfig) -> impl Future<Output = Arc<...>>`
pub async fn launch_async<FB: SizeLike + 'static, TB: BlockLike + 'static, CTX: WriteTxContext + 'static, CFN, PFN, PFut>(
    block_provider_factory: PFN,
    build_chain: CFN,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
) -> Result<(), ChainError>
where
    CFN: FnOnce(Arc<Storage>) -> Arc<dyn BlockChainLike<TB, CTX>>,
    PFN: FnOnce(AppConfig) -> PFut,
    PFut: Future<Output = Arc<dyn BlockProvider<FB, TB>>> + Send,
{
    let config: AppConfig = chain_config::load_config("config/settings", "REDBIT")?;
    let provider = block_provider_factory(config).await;
    launch_with_provider::<FB, TB, CTX>(provider, build_chain, extras, cors).await
}