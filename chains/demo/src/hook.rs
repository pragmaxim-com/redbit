use crate::model_v1::*;

pub(crate) fn load_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_refs: Vec<InputRef>) -> Result<Vec<Input>, AppError> {
    let mut inputs = Vec::with_capacity(input_refs.len());
    for (index, transient_input) in input_refs.iter().enumerate() {
        match tx_context.transaction_hash_index.get(&transient_input.tx_hash)?.next() {
            Some(Ok(tx_pointer)) => {
                let id = TransactionPointer::from_parent(parent, index as u16);
                let utxo_ref = TransactionPointer::from_parent(tx_pointer.value(), transient_input.index as u16);
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
