use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub fn bootstrap_storage(root_entity_ident: &Ident, key_ident: &Ident) -> TokenStream {
    quote! {
        pub fn init(storage: Arc<Storage>) -> redb::Result<(), AppError> {
            let sample_block = #root_entity_ident::sample();
            #root_entity_ident::store_and_commit(Arc::clone(&storage), &sample_block)?;
            #root_entity_ident::delete_and_commit(Arc::clone(&storage), &sample_block.#key_ident)?;
            Ok(())
        }
    }
}
