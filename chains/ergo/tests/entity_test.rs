#[cfg(all(test, not(feature = "integration")))]
mod tests {
    use ergo::model_v1::*;
    use redbit::storage::init::StorageOwner;

    async fn init_temp_storage(name: &str, db_cache_size_gb: u8) -> (Vec<Block>, StorageOwner, Arc<Storage>) {
        let (storage_owner, storage) = StorageOwner::temp(name, db_cache_size_gb, true).await.unwrap();
        let blocks = Block::sample_many(3);
        let ctx = Block::begin_write_ctx(&storage, Durability::None).unwrap();
        ctx.two_phase_commit_or_rollback_and_close_with(|tx_context| {
            Block::store_many(&tx_context, blocks.clone(), true)?;
            Ok(())
        }).expect("Failed to use write context");
        (blocks, storage_owner, storage)
    }

    #[tokio::test]
    async fn debug_io() {
        let (blocks, _storage_owner, storage) = init_temp_storage("db_test", 0).await;
        println!("SAMPLE");

        for tx in blocks.last().unwrap().transactions.clone() {
            println!("");
            // println!("sample {:?}", tx.hash);
            for i in tx.inputs.iter() {
                print!(" id: {:?} |", i.id.url_encode());
            }
            println!("");
            for i in tx.inputs.iter() {
                print!(" po: {:?} |", i.utxo_pointer.url_encode());
            }
            println!("");
        }

        println!("PERSISTED");
        let block_read_ctx = Block::begin_read_ctx(&storage).unwrap();
        for tx in Block::last(&block_read_ctx).unwrap().unwrap().transactions {
            println!("");
            // println!("persisted {:?}", tx.hash);
            for i in tx.inputs.iter() {
                print!(" id: {:?} |", i.id.url_encode());
            }
            println!("");
            for i in tx.inputs.iter() {
                print!(" po: {:?} |", i.utxo_pointer.url_encode());
            }
            println!("");
        }
    }
}