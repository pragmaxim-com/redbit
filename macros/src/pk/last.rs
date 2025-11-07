use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::parse_quote;

pub fn fn_def(entity_def: &EntityDef, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("last");
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let read_ctx_type = &entity_def.read_ctx_type;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_type) -> Result<Option<#entity_type>, AppError> {
            if let Some((k, _)) = tx_context.#table.last_key()? {
                return Self::compose(&tx_context, k.value()).map(Some);
            }
            Ok(None)
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() -> Result<(), AppError> {
            let (storage_owner, storage) = &*STORAGE;
            let entity_count: usize = 3;
            let tx_context = #entity_name::begin_read_ctx(&storage)?;
            let entity = #entity_name::last(&tx_context)?.expect("Expected last entity to exist");
            let expected_entity = #entity_type::sample_many(Default::default(), entity_count).last().expect("Expected at least one entity").clone();
            assert_eq!(entity, expected_entity, "Last entity does not match expected");
            Ok(())
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::last(&tx_context).expect("Failed to get last entity by PK").expect("Expected last entity to exist");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(parse_quote! { Vec<#entity_type> }),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    let tx_context = #entity_name::begin_read_ctx(&state.storage)?;
                    let result: Vec<#entity_type> = #entity_name::#fn_name(&tx_context).map(|r| r.into_iter().collect())?;
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