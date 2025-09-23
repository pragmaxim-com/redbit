use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Query;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::{parse_quote, Type};

pub fn fn_def(entity_name: &Ident, entity_type: &Type, tx_context_ty: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("take");
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, n: usize) -> Result<Vec<#entity_type>, AppError> {
            let mut iter = tx_context.#table.iter()?;
            let mut results = Vec::new();
            let mut count: usize = 0;
            if n > 100 {
                Err(AppError::Internal("Cannot take more than 100 entities at once".into()))
            } else {
                while let Some(entry_res) = iter.next() {
                    if count >= n {
                        break;
                    }
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&tx_context, pk)?);
                    count += 1;
                }
                Ok(results)
            }
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let n: usize = 2;
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, n).expect("Failed to take entities");
            let expected_entities = #entity_type::sample_many(n);
            assert_eq!(entities, expected_entities, "Expected to take 2 entities");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let n: usize = 2;
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, n).expect("Failed to take entities");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(parse_quote!{ Vec<#entity_type> }),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Query(QueryExpr {
                ty: syn::parse_quote!(TakeQuery),
                extraction: quote! { extract::Query(query): extract::Query<TakeQuery> },
                samples: quote! { vec![TakeQuery::sample()] },
            })],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    let tx_context = #entity_name::begin_read_ctx(&state.storage)?;
                    let result = #entity_name::#fn_name(&tx_context, query.take)?;
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
