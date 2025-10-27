use crate::model_v1::*;

pub(crate) fn write_from_input_refs_using_utxos(tx_context: &TransactionWriteTxContext, input_refs: Vec<(BoxId, (BlockPointer, usize))>, is_last: bool) -> Result<(), AppError> {
    let ids_router  = tx_context.inputs.input_id.acquire_router();
    let ptrs_router = tx_context.inputs.input_utxo_pointer_by_id.acquire_router();
    let tx_hashes = input_refs.iter().map(|(box_id, _)| *box_id).collect::<Vec<_>>();
    tx_context.utxos.utxo_box_id_index.router.query_and_write(tx_hashes, is_last, Arc::new(move |last_shards, out| {
        let mut ids = Vec::with_capacity(out.len());
        let mut pointers = Vec::with_capacity(out.len());
        for (index, tx_pointer_buf_opt) in out.into_iter() {
            let (_, (id_pointer, id_index)) = match input_refs.get(index) {
                Some(e) => e,
                None => return Err(AppError::Custom(format!("internal error: no input_ref exists for index {}", index))),
            };
            let id = TransactionPointer::from_parent(*id_pointer, *id_index as u16);
            let utxo_pointer = match tx_pointer_buf_opt {
                Some(tx_pointer_buf) => tx_pointer_buf.as_value(),
                None => TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0)
            };
            ids.push((id, ()));
            pointers.push((id, utxo_pointer));
        }
        ids_router.merge_unsorted_inserts(ids, last_shards)?;
        ptrs_router.merge_unsorted_inserts(pointers, last_shards)?;
        Ok(())
    }))
}
