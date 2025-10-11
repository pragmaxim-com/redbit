use crate::model_v1::*;

pub(crate) fn write_from_input_refs_using_utxos(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_refs: Vec<BoxId>) -> Result<(), AppError> {
    let ids_router  = tx_context.inputs.input_id.router();
    let ptrs_router = tx_context.inputs.input_utxo_pointer_by_id.router();
    tx_context.utxos.utxo_box_id_index.router.query_and_write(input_refs, Arc::new(move |out| {
        let mut ids = Vec::with_capacity(out.len());
        let mut pointers = Vec::with_capacity(out.len());
        for (index, tx_pointer_buf_opt) in out.into_iter() {
            match tx_pointer_buf_opt {
                Some(tx_pointer_buf) => {
                    let id = TransactionPointer::from_parent(parent, index as u16);
                    let utxo_pointer = tx_pointer_buf.as_value();
                    ids.push((id, ()));
                    pointers.push((id, utxo_pointer));
                }
                _ => {
                    let id = TransactionPointer::from_parent(parent, index as u16);
                    let utxo_pointer = TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0); // genesis of unknown index
                    ids.push((id, ()));
                    pointers.push((id, utxo_pointer));
                },
            }
        }
        ids_router.append_sorted_inserts(ids)?;
        ptrs_router.merge_unsorted_inserts(pointers)?;
        Ok(())
    }))
}
