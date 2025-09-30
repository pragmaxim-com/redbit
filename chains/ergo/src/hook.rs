use crate::model_v1::*;

pub(crate) fn write_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_refs: Vec<BoxId>) -> Result<Vec<Input>, AppError> {
    let mut inputs = Vec::with_capacity(input_refs.len());
    let tx_pointer_buffers = tx_context.utxos.utxo_box_id_index.get_any_for_index(input_refs)?;
    for (index, tx_pointer_buf_opt) in tx_pointer_buffers.into_iter().enumerate() {
        match tx_pointer_buf_opt {
            Some(tx_pointer_buf) => {
                let id = TransactionPointer::from_parent(parent, index as u16);
                let utxo_pointer = tx_pointer_buf.as_value();
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

