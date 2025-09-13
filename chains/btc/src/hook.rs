use crate::model_v1::*;

pub(crate) fn write_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_index: usize, input_ref: &InputRef) -> Result<Input, AppError> {
    let input =
        match tx_context.transaction_hash_index.get(input_ref.tx_hash)?.next() {
            Some(Ok(tx_pointer)) => {
                let id = TransactionPointer::from_parent(parent, input_index as u16);
                let utxo_pointer = TransactionPointer::from_parent(tx_pointer.value(), input_ref.index as u16);
                Input { id, utxo_pointer }
            },
            _ => {
                let id = TransactionPointer::from_parent(parent, input_index as u16);
                let utxo_pointer = TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0); // genesis of unknown index
                Input { id, utxo_pointer }
            },
        };
    Ok(input)
}
