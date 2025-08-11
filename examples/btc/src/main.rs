use anyhow::Result;
use syncer::api::{BlockPersistence, BlockProvider};
use syncer::scheduler::Scheduler;
use syncer::settings::{AppConfig, HttpSettings, IndexerSettings};
use syncer::{combine, info};
use futures::future::ready;
use redbit::*;
use std::env;
use std::sync::Arc;
use tokio::sync::watch;
use tower_http::cors;
use btc::block_persistence::BtcBlockPersistence;
use btc::block_provider::BtcBlockProvider;
use btc::btc_client::{BtcBlock, BtcClient};
use btc::config::BitcoinConfig;
use btc::model_v1::Block;
use btc::storage;

async fn maybe_run_server(http_conf: HttpSettings, storage: Arc<Storage>, shutdown: watch::Receiver<bool>) -> () {
    if http_conf.enable {
        info!("Starting http server at {}", http_conf.bind_address);
        let cors = cors::CorsLayer::new()
            .allow_origin(cors::Any) // or use a specific origin: `AllowOrigin::exact("http://localhost:5173".parse().unwrap())`
            .allow_methods(cors::Any)
            .allow_headers(cors::Any);
        serve(RequestState { storage: Arc::clone(&storage) }, http_conf.bind_address, None, Some(cors), shutdown).await
    } else {
        ready(()).await
    }
}

async fn maybe_run_indexing(index_config: IndexerSettings, scheduler: Scheduler<BtcBlock, Block>, shutdown: watch::Receiver<bool>) -> () {
    if index_config.enable {
        info!("Starting indexing process");
        scheduler.schedule(index_config, shutdown).await
    } else {
        ready(()).await
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let app_config = AppConfig::new("config/settings")?;
    let btc_config = BitcoinConfig::new("config/bitcoin")?;
    let db_path: String = format!("{}/{}/{}", app_config.indexer.db_path, "main", "btc");
    let full_db_path = env::home_dir().unwrap().join(&db_path);
    let storage = storage::get_storage(full_db_path, app_config.indexer.db_cache_size_gb)?;
    let fetching_par: usize = app_config.indexer.fetching_parallelism.clone().into();

    let btc_client = Arc::new(BtcClient::new(&btc_config)?);
    let block_provider: Arc<dyn BlockProvider<BtcBlock, Block>> = Arc::new(BtcBlockProvider::new(btc_client, fetching_par)?);
    let block_persistence: Arc<dyn BlockPersistence<Block>> = Arc::new(BtcBlockPersistence { storage: Arc::clone(&storage) });
    let scheduler: Scheduler<BtcBlock, Block> = Scheduler::new(block_provider, block_persistence);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let indexing_f = maybe_run_indexing(app_config.indexer, scheduler, shutdown_rx.clone());
    let server_f = maybe_run_server(app_config.http, Arc::clone(&storage), shutdown_rx.clone());
    combine::futures(indexing_f, server_f, shutdown_tx).await;
    Ok(())
}
