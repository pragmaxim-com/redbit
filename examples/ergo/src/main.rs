#![feature(test)]
extern crate test;

mod codec;
mod block_persistence;
mod block_provider;
mod config;
mod ergo_client;
mod model;
mod storage;

use crate::block_persistence::ErgoBlockPersistence;
use crate::block_provider::ErgoBlockProvider;
use crate::config::ErgoConfig;
use crate::model::Block;
use anyhow::Result;
use syncer::api::{BlockPersistence, BlockProvider};
use syncer::scheduler::Scheduler;
use syncer::settings::{AppConfig, HttpSettings, IndexerSettings};
use syncer::{combine, info};
use futures::future::ready;
use redbit::redb::Database;
use redbit::*;
use std::env;
use std::sync::Arc;
use ergo_lib::chain::block::FullBlock;
use tokio::sync::watch;
use tower_http::cors;

async fn maybe_run_server(http_conf: HttpSettings, db: Arc<Database>, shutdown: watch::Receiver<bool>) -> () {
    if http_conf.enable {
        info!("Starting http server at {}", http_conf.bind_address);
        let cors = cors::CorsLayer::new()
            .allow_origin(cors::Any) // or use a specific origin: `AllowOrigin::exact("http://localhost:5173".parse().unwrap())`
            .allow_methods(cors::Any)
            .allow_headers(cors::Any);
        serve(RequestState { db: Arc::clone(&db) }, http_conf.bind_address, None, Some(cors), shutdown).await
    } else {
        ready(()).await
    }
}

async fn maybe_run_indexing(index_config: IndexerSettings, scheduler: Scheduler<FullBlock, Block>, shutdown: watch::Receiver<bool>) -> () {
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
    let ergo_config = ErgoConfig::new("config/ergo")?;
    let db_path: String = format!("{}/{}/{}", app_config.indexer.db_path, "main", "ergo");
    let full_path = env::home_dir().unwrap().join(&db_path);
    let db = Arc::new(storage::get_db(full_path, app_config.indexer.db_cache_size_gb)?);
    let fetching_par: usize = app_config.indexer.fetching_parallelism.clone().into();
    
    let block_provider: Arc<dyn BlockProvider<FullBlock, Block>> = Arc::new(ErgoBlockProvider::new(&ergo_config, fetching_par));
    let block_persistence: Arc<dyn BlockPersistence<Block>> = Arc::new(ErgoBlockPersistence { db: Arc::clone(&db) });
    let scheduler: Scheduler<FullBlock, Block> = Scheduler::new(block_provider, block_persistence);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let indexing_f = maybe_run_indexing(app_config.indexer, scheduler, shutdown_rx.clone());
    let server_f = maybe_run_server(app_config.http, Arc::clone(&db), shutdown_rx.clone());
    combine::futures(indexing_f, server_f, shutdown_tx).await;
    Ok(())
}
