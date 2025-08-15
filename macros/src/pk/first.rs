use crate::endpoint::EndpointDef;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("first");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &StorageReadTx) -> Result<Option<#entity_type>, AppError> {
            let table_pk_7 = tx.open_table(#table)?;
            if let Some((k, _)) = table_pk_7.first()? {
                return Self::compose(&tx, &k.value()).map(Some);
            }
            Ok(None)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let read_tx = storage.begin_read().expect("Failed to begin read transaction");
            let entity = #entity_name::first(&read_tx).expect("Failed to get first entity by PK").expect("Expected first entity to exist");
            let expected_enity = #entity_type::sample();
            assert_eq!(entity, expected_enity, "First entity does not match expected");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let read_tx = storage.begin_read().expect("Failed to begin read transaction");
            b.iter(|| {
                #entity_name::first(&read_tx).expect("Failed to get first entity by PK").expect("Expected first entity to exist");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    let read_tx = state.storage.begin_read()?;
                    let result: Vec<#entity_type> = #entity_name::#fn_name(&read_tx).map(|r| r.into_iter().collect())?;
                    Ok(AppJson(result))
                }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = Vec<#entity_type>),
                    (status = 500, content_type = "application/json", body = ErrorResponse),
                )
            },
            endpoint: format!("/{}/{}", entity_name.to_string().to_lowercase(), fn_name),
        }.to_endpoint()),
        test_stream,
        bench_stream,
    }
}
