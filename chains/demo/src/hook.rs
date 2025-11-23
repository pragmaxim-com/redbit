use crate::model_v1::*;

pub(crate) fn write_from_input_refs_using_hash(tx_context: &TransactionWriteTxContext, input_refs: Vec<(InputRef, (BlockPointer, usize))>, is_last: bool) -> Result<(), AppError> {
    let ids_router  = tx_context.inputs.input_id.acquire_router();
    let ptrs_router = tx_context.inputs.input_utxo_pointer_by_id.acquire_router();
    let tx_hashes = input_refs.iter().map(|(ir, _)| ir.tx_hash).collect::<Vec<_>>();
    tx_context.transaction_hash_index.query_and_write(tx_hashes, is_last, Arc::new(move |last_shards, out| {
        let mut ids = Vec::with_capacity(out.len());
        let mut pointers = Vec::with_capacity(out.len());
        for (index, tx_pointer_buf_opt) in out.into_iter() {
            let (input_ref, (id_pointer, id_index)) = match input_refs.get(index) {
                Some(e) => e,
                None => return Err(AppError::Custom(format!("internal error: no input_ref exists for index {}", index))),
            };
            let id = TransactionPointer::from_parent(*id_pointer, *id_index as u16);
            let utxo_pointer = match tx_pointer_buf_opt {
                Some(tx_pointer_buf) => TransactionPointer::from_parent(tx_pointer_buf.as_value(), input_ref.index),
                None => {
                    warn!("missing tx_pointer for input_ref {:?}", input_ref);
                    TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0)
                }
            };
            ids.push((id, ()));
            pointers.push((id, utxo_pointer));
        }
        ids_router.merge_unsorted_inserts(ids, last_shards)?;
        ptrs_router.merge_unsorted_inserts(pointers, last_shards)?;
        Ok(())
    }))
}

/// Manual hook that reuses already-begun writers passed from the manual runtime.
pub(crate) fn write_from_input_refs_using_hash_manual(
    hash_batch: &redbit::manual_entity::IndexBatch<Transaction, BlockPointer, TxHash>,
    child_writers: Option<&dyn redbit::manual_entity::ChildWriters>,
    input_refs: Vec<(InputRef, (BlockPointer, usize))>,
    is_last: bool,
) -> Result<(), AppError> {
    // Downcast child writers to locate routers for input_id and input_utxo_pointer_by_id.
    let (id_router, ptr_router) = {
        if let Some(tree) = child_writers.and_then(|cw| cw.as_any().downcast_ref::<redbit::manual_entity::RuntimeWritersWithChildren<Input, TransactionPointer>>()) {
            let ptr_batch = tree.self_writers.column_batches.get(0).and_then(|b| b.as_ref());
            let ptr_router = ptr_batch
                .and_then(|b| b.as_any().downcast_ref::<redbit::manual_entity::PlainBatch<Input, TransactionPointer, TransactionPointer>>())
                .map(|pb| pb.writer.acquire_router())
                .ok_or_else(|| AppError::Custom("write_from: missing input_utxo_pointer_by_id writer".into()))?;
            (tree.self_writers.pk_writer.acquire_router(), ptr_router)
        } else if let Some(flat) = child_writers.and_then(|cw| cw.as_any().downcast_ref::<redbit::manual_entity::RuntimeWriters<Input, TransactionPointer>>()) {
            let ptr_batch = flat.column_batches.get(0).and_then(|b| b.as_ref());
            let ptr_router = ptr_batch
                .and_then(|b| b.as_any().downcast_ref::<redbit::manual_entity::PlainBatch<Input, TransactionPointer, TransactionPointer>>())
                .map(|pb| pb.writer.acquire_router())
                .ok_or_else(|| AppError::Custom("write_from: missing input_utxo_pointer_by_id writer".into()))?;
            (flat.pk_writer.acquire_router(), ptr_router)
        } else {
            return Ok(()); // no child writers available; skip
        }
    };

    // Hashes to lookup (use router to stay within the begun batch).
    let tx_hashes: Vec<_> = input_refs.iter().map(|(ir, _)| ir.tx_hash).collect();
    hash_batch.writer.query_and_write(tx_hashes, is_last, std::sync::Arc::new(move |last_shards, out| {
        let mut ids = Vec::with_capacity(out.len());
        let mut pointers = Vec::with_capacity(out.len());
        for (index, tx_pointer_buf_opt) in out.into_iter() {
            let (input_ref, (id_pointer, id_index)) = match input_refs.get(index) {
                Some(e) => e,
                None => return Err(AppError::Custom(format!("internal error: no input_ref exists for index {}", index))),
            };
            let id = TransactionPointer::from_parent(*id_pointer, *id_index as u16);
            let utxo_pointer = match tx_pointer_buf_opt {
                Some(tx_pointer_buf) => TransactionPointer::from_parent(tx_pointer_buf.as_value(), input_ref.index),
                None => {
                    warn!("missing tx_pointer for input_ref {:?}", input_ref);
                    TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0)
                }
            };
            ids.push((id, ()));
            pointers.push((id, utxo_pointer));
        }
        let ids_last = last_shards.map(|_| id_router.shards());
        let ptrs_last = last_shards.map(|_| ptr_router.shards());
        id_router.merge_unsorted_inserts(ids, ids_last)?;
        ptr_router.merge_unsorted_inserts(pointers, ptrs_last)?;
        Ok(())
    }))
}
