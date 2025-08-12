use crate::field_parser::KeyDef;
use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::quote;

pub fn init(root_entity_ident: &Ident, key_def: &KeyDef) -> Vec<FunctionDef> {
    match &key_def {
        KeyDef::Pk(f) => {
            let key_ident = &f.name;
            vec![FunctionDef {
                fn_stream: quote! {
                    pub fn init(storage: Arc<Storage>) -> redb::Result<(), AppError> {
                        let sample_block = #root_entity_ident::sample();
                        #root_entity_ident::store_and_commit(Arc::clone(&storage), &sample_block)?;
                        #root_entity_ident::delete_and_commit(Arc::clone(&storage), &sample_block.#key_ident)?;
                        Ok(())
                    }
                },
                endpoint: None,
                test_stream: Some(quote! {
                    #[test]
                    fn init_storage() {
                        let storage = random_storage();
                        let result = #root_entity_ident::init(Arc::clone(&storage));
                        assert!(result.is_ok());
                    }
                }),
                bench_stream: None,
            }]
        },
        _ => vec![],
    }
}
