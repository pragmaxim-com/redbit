use crate::model_v1::*;

pub(crate) fn load_from_input_refs(tx_context: &TransactionWriteTxContext, input_refs: Vec<InputRef>) -> Result<Vec<Input>, AppError> {
    let mut inputs = Vec::with_capacity(input_refs.len());
    for transient_input in input_refs {
        match tx_context.transaction_hash_index.get(&transient_input.tx_hash)?.next() {
            Some(Ok(tx_pointer)) => inputs.push(Input { id: TransactionPointer::from_parent(tx_pointer.value(), transient_input.index as u16) }),
            _ => inputs.push(Input { id: TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0) }),
        }
    }
    Ok(inputs)
}
