use crate::model_v1::*;
#[cfg(test)]
use redbit::manual_entity::ManualTestScope;
use std::sync::Arc;
use redbit::storage::init::DbDef;
use std::collections::HashSet;

fn dedup_defs(defs: Vec<DbDef>) -> Vec<DbDef> {
    let mut seen = HashSet::new();
    defs.into_iter().filter(|d| seen.insert(d.name.clone())).collect()
}

pub fn build_block_runtime_auto() -> Result<(Arc<redbit::manual_entity::ManualEntityRuntime<Block, Height>>, Vec<DbDef>), redbit::AppError> {
    let rt = Block::manual_runtime_auto()?;
    let mut db_defs = Block::db_defs();
    db_defs.extend(Input::db_defs());
    Ok((rt, dedup_defs(db_defs)))
}

pub fn build_transaction_runtime_auto() -> Result<(Arc<redbit::manual_entity::ManualEntityRuntime<Transaction, BlockPointer>>, Vec<DbDef>), redbit::AppError> {
    let rt = Transaction::manual_runtime_auto()?;
    let mut db_defs = rt.db_defs();
    db_defs.extend(Input::db_defs());
    Ok((rt, dedup_defs(db_defs)))
}

#[tokio::test]
async fn manual_transaction_write_from_roundtrip() -> Result<(), redbit::AppError> {
    let (tx_rt, db_defs) = build_transaction_runtime_auto()?;

    let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;

    let block_pk = BlockPointer::from_parent(Height(1), 0);
    let tx_hash = TxHash([1u8; 32]);
    let tx = Transaction {
        id: block_pk,
        hash: tx_hash,
        input_refs: vec![InputRef { tx_hash, index: 0 }],
        ..Default::default()
    };

    // Use manual writer tree to mirror runtime store/commit (so write_from runs).
    let writers = Transaction::manual_writers_auto(&storage)?;
    {
        let refs = writers.writer_refs_labeled();
        eprintln!("begin_manual writers={}", refs.len());
        for (label, c) in &refs {
            eprintln!("begin writer {}", label);
            for fut in c.begin_async_ref(redbit::Durability::None)? {
                fut.wait()?;
            }
        }
        eprintln!("store_manual");
        tx_rt.store_batch_with_writer_tree(&storage, &writers, std::slice::from_ref(&tx))?;
        eprintln!("commit_manual");
        let mut flushes: Vec<(String, redbit::storage::table_writer_api::FlushFuture)> = Vec::new();
        for (label, c) in &refs {
            eprintln!("flush writer {}", label);
            for f in c.commit_with_ref()? {
                flushes.push((label.to_string(), f));
            }
        }
        for (idx, (lbl, f)) in flushes.into_iter().enumerate() {
            eprintln!("waiting flush {} ({})", idx, lbl);
            let _ = f.wait()?;
            eprintln!("flush {} ({}) done", idx, lbl);
        }
        eprintln!("commit_done");
    }
    eprintln!("stop_manual");
    let stops = writers.stop_async()?;
    for s in stops {
        s.wait()?;
    }

    eprintln!("open read ctx");

    let read_ctx = Transaction::begin_read_ctx(&storage)?;
    let input_id = TransactionPointer::from_parent(block_pk, 0);
    let guard = read_ctx.inputs.input_utxo_pointer_by_id.get_value(input_id)?;
    assert!(guard.is_some(), "write_from hook should populate input_utxo_pointer_by_id");

    owner.assert_last_refs();
    let _ = std::fs::remove_dir_all(path);
    Ok(())
}

#[tokio::test]
async fn manual_block_roundtrip_with_cascades() -> Result<(), redbit::AppError> {
    let (block_rt, mut db_defs) = build_block_runtime_auto()?;
    db_defs.extend(block_rt.db_defs());
    let db_defs = dedup_defs(db_defs);

    let (owner, storage, path) = ManualTestScope::temp_storage(db_defs).await?;

    let block_pk = Height(2);
    let tx_hash = TxHash([9u8; 32]);
    let block = Block {
        height: block_pk,
        header: Header { height: block_pk, hash: BlockHash([7u8; 32]), prev_hash: BlockHash([6u8; 32]), timestamp: Timestamp(42), ..Default::default() },
        transactions: vec![Transaction {
            id: BlockPointer::from_parent(block_pk, 0),
            hash: tx_hash,
            input_refs: vec![InputRef { tx_hash, index: 0 }],
            ..Default::default()
        }],
    };

    block_rt.store(&storage, &block)?;

    let loaded = block_rt.compose(&storage, block_pk)?.expect("block present");
    assert_eq!(loaded.header.hash, block.header.hash);
    assert_eq!(loaded.transactions.len(), 1);
    let tx_read = Transaction::begin_read_ctx(&storage)?;
    let input_id = TransactionPointer::from_parent(BlockPointer::from_parent(block_pk, 0), 0);
    assert!(tx_read.inputs.input_id.get_value(input_id)?.is_some(), "input id should be present");
    assert!(tx_read.inputs.input_utxo_pointer_by_id.get_value(input_id)?.is_some(), "input utxo pointer should be present");

    owner.assert_last_refs();
    let _ = std::fs::remove_dir_all(path);
    Ok(())
}
