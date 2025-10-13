use crate::field_parser::EntityDef;
use crate::rest::FunctionDef;
use proc_macro2::Ident;
use quote::{format_ident, quote};

pub fn fn_def(entity_def: &EntityDef, table_var: &Ident) -> FunctionDef {
    let EntityDef { key_def, write_ctx_type, ..} = &entity_def;
    let fn_name = format_ident!("pk_range");
    let key_def = key_def.field_def();
    let pk_type = &key_def.tpe;

    let fn_stream = quote! {
        fn #fn_name(tx_context: &#write_ctx_type, from: #pk_type, until: #pk_type) -> Result<Vec<#pk_type>, AppError> {
            let entries = tx_context.#table_var.router.range(from, until)?;
            let mut results = Vec::new();
            for (key, _) in entries {
                let pointer: #pk_type = key.as_value();
                results.push(pointer);
            }
            Ok(results)
        }
    };

    FunctionDef {
        fn_stream,
        endpoint: None,
        test_stream: None,
        bench_stream: None
    }

}