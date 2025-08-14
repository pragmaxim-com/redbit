use crate::api::{BlockLike, BlockPersistence, BlockProvider};
use crate::scheduler::Scheduler;
use crate::settings::{AppConfig, HttpSettings, IndexerSettings};
use crate::{combine, info};
use futures::future::ready;
use redbit::storage::Storage;
use redbit::{serve, AppError, OpenApiRouter, RequestState};
use std::env;
use std::sync::Arc;
use tokio::sync::watch;
use tower_http::cors;
use tower_http::cors::CorsLayer;

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
        ready(()).await
    }
}

pub async fn maybe_run_syncing<FB: Send + Sync + 'static, TB: BlockLike + 'static>(
    index_config: IndexerSettings,
    scheduler: Scheduler<FB, TB>,
    shutdown: watch::Receiver<bool>,
) -> () {
    if index_config.enable {
        if index_config.sync_interval_s.is_zero() {
            info!("Syncing initiated");
            scheduler.sync(index_config).await;
            info!("Syncing completed");
        } else {
            info!("Scheduling initiated");
            scheduler.schedule(index_config, shutdown).await
        }
    } else {
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

pub async fn launch<FB: Send + Sync + 'static, TB: BlockLike + 'static, F>(
    block_provider: Arc<dyn BlockProvider<FB, TB>>,
    make_persistence: F,
    extras: Option<OpenApiRouter<RequestState>>,
    cors: Option<CorsLayer>,
) -> Result<(), AppError>
where
    F: FnOnce(Arc<Storage>) -> Arc<dyn BlockPersistence<TB>>,
{
    maybe_console_init();
    let config = AppConfig::new("config/settings").expect("Failed to load app config");
    let db_path: String = format!("{}/{}/{}", config.indexer.db_path, "main", config.indexer.name);
    let full_path = env::home_dir().unwrap().join(&db_path);
    let storage: Arc<Storage> = Storage::init(full_path, config.indexer.db_cache_size_gb)?;
    let block_persistence: Arc<dyn BlockPersistence<TB>> = make_persistence(Arc::clone(&storage));
    let scheduler: Scheduler<FB, TB> = Scheduler::new(block_provider, block_persistence);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let indexing_f = maybe_run_syncing(config.indexer, scheduler, shutdown_rx.clone());
    let server_f = maybe_run_server(config.http, Arc::clone(&storage), extras, cors, shutdown_rx.clone());
    Ok(combine::futures(indexing_f, server_f, shutdown_tx).await)
}
