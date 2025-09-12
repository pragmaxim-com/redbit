use crate::model_v1::*;

pub(crate) fn load_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_refs: Vec<BoxId>) -> Result<Vec<Input>, AppError> {
    let mut inputs = Vec::with_capacity(input_refs.len());
    for (index, box_id) in input_refs.iter().enumerate() {
        match tx_context.utxos.utxo_box_id_index.get(box_id)?.next() {
            Some(Ok(utxo_pointer_guard)) => {
                let id = TransactionPointer::from_parent(parent, index as u16);
                let utxo_pointer = utxo_pointer_guard.value();
                let utxo_ref = TransactionPointer::from_parent(utxo_pointer.parent, utxo_pointer.index);
                inputs.push(Input { id, utxo_ref })
            },
            _ => {
                let id = TransactionPointer::from_parent(parent, index as u16);
                let utxo_ref = TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0); // genesis of unknown index
                inputs.push(Input { id, utxo_ref })
            },
        }

    }
    Ok(inputs)
}
