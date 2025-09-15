use std::ops::Index;
use crate::model_v1::*;

pub(crate) fn write_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_refs: Vec<InputRef>) -> Result<Vec<Input>, AppError> {
    let mut inputs = Vec::with_capacity(input_refs.len());
    let tx_hashes = input_refs.iter().map(|input_ref| input_ref.tx_hash).collect::<Vec<_>>();
    let tx_pointer_buffers = tx_context.transaction_hash_index.get_head_for_index(tx_hashes)?;
    for (index, tx_pointer_buf_opt) in tx_pointer_buffers.iter().enumerate() {
        match tx_pointer_buf_opt {
            Some(tx_pointer_buf) => {
                let tx_pointer = tx_pointer_buf.as_value();
                let id = TransactionPointer::from_parent(parent, index as u16);
                let utxo_pointer = TransactionPointer::from_parent(tx_pointer, input_refs.index(index).index as u16);
                inputs.push(Input { id, utxo_pointer })
            }
            _ => {
                let id = TransactionPointer::from_parent(parent, index as u16);
                let utxo_pointer = TransactionPointer::from_parent(BlockPointer::from_parent(Height(0), 0), 0); // genesis of unknown index
                inputs.push(Input { id, utxo_pointer })
            },
        }
    }
    Ok(inputs)
}
