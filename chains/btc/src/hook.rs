use crate::model_v1::*;
use std::ops::Index;
use std::sync::Arc;

pub(crate) fn write_from_input_refs_using_hash(tx_context: &TransactionWriteTxContext, input_refs_with_pointer: indexmap::IndexMap<InputRef, BlockPointer>, is_last: bool) -> Result<(), AppError> {
    let ids_router  = tx_context.inputs.input_id.acquire_router();
    let ptrs_router = tx_context.inputs.input_utxo_pointer_by_id.acquire_router();
    let tx_hashes = input_refs_with_pointer.iter().map(|(ir, _)| ir.tx_hash).collect::<Vec<_>>();
    tx_context.transaction_hash_index.router.query_and_write(tx_hashes, is_last, Arc::new(move |last_shards, out| {
        let mut ids = Vec::with_capacity(out.len());
        let mut pointers = Vec::with_capacity(out.len());
        for (index, tx_pointer_buf_opt) in out.into_iter() {
            match tx_pointer_buf_opt {
                Some(tx_pointer_buf) => {
                    let tx_pointer = tx_pointer_buf.as_value();
                    let id = TransactionPointer::from_parent(*input_refs_with_pointer.index(index), index as u16);
                    let utxo_pointer = TransactionPointer::from_parent(tx_pointer, input_refs_with_pointer.index(index).index);
                    ids.push((id, ()));
                    pointers.push((id, utxo_pointer));
                }
                _ => {
                    let id = TransactionPointer::from_parent(*input_refs_with_pointer.index(index), index as u16);
                    let utxo_pointer = TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0); // genesis of unknown index
                    ids.push((id, ()));
                    pointers.push((id, utxo_pointer));
                },
            }
        }
        ids_router.merge_unsorted_inserts(ids, last_shards)?;
        ptrs_router.merge_unsorted_inserts(pointers, last_shards)?;
        Ok(())
    }))
}
