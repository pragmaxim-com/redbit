use crate::model_v1::*;

pub(crate) fn load_from_input_refs(tx_context: &TransactionWriteTxContext, input_refs: Vec<BoxId>) -> Result<Vec<Input>, AppError> {
    let mut inputs = Vec::with_capacity(input_refs.len());
    for box_id in input_refs {
        match tx_context.utxos.utxo_box_id_index.get(box_id)?.next() {
            Some(Ok(utxo_pointer_guard)) => {
                let utxo_pointer = utxo_pointer_guard.value();
                inputs.push(Input { id: TransactionPointer::from_parent(utxo_pointer.parent, utxo_pointer.index) })
            },
            _ => inputs.push(Input { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
        }

    }
    Ok(inputs)
}
