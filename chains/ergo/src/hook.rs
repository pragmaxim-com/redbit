use crate::model_v1::*;
use redbit::storage::table_writer::WriterCommand;

pub(crate) fn write_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_refs: Vec<BoxId>) -> Result<(), AppError> {
    let ids_sender      = tx_context.inputs.input_id.sender();
    let utxo_pointer_sender     = tx_context.inputs.input_utxo_pointer_by_id.sender();
    tx_context.utxos.utxo_box_id_index.get_any_for_index(input_refs, move |out| {
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
        let _ = ids_sender.send(WriterCommand::InsertMany(ids));
        let _ = utxo_pointer_sender.send(WriterCommand::InsertMany(pointers));
    })
}
