use crate::model_v1::*;
use redbit::storage::table_writer::WriterCommand;
use std::ops::Index;

pub(crate) fn write_from_input_refs(tx_context: &TransactionWriteTxContext, parent: BlockPointer, input_refs: Vec<InputRef>) -> Result<(), AppError> {
    let tx_hashes = input_refs.iter().map(|input_ref| input_ref.tx_hash).collect::<Vec<_>>();
    let ids_sender      = tx_context.inputs.input_id.sender();
    let utxo_pointer_sender     = tx_context.inputs.input_utxo_pointer_by_id.sender();
    tx_context.transaction_hash_index.get_any_for_index(tx_hashes, move |out| {
        let mut ids = Vec::with_capacity(out.len());
        let mut pointers = Vec::with_capacity(out.len());
        for (index, tx_pointer_buf_opt) in out.into_iter() {
            match tx_pointer_buf_opt {
                Some(tx_pointer_buf) => {
                    let tx_pointer = tx_pointer_buf.as_value();
                    let id = TransactionPointer::from_parent(parent, index as u16);
                    let utxo_pointer = TransactionPointer::from_parent(tx_pointer, input_refs.index(index).index as u16);
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
