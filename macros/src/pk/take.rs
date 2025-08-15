use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Query;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("take");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &StorageReadTx, n: usize) -> Result<Vec<#entity_type>, AppError> {
            let table_pk_6 = tx.open_table(#table)?;
            let mut iter = table_pk_6.iter()?;
            let mut results = Vec::new();
            let mut count: usize = 0;
            if n > 100 {
                return Err(AppError::Internal("Cannot take more than 100 entities at once".into()));
            } else {
                while let Some(entry_res) = iter.next() {
                    if count >= n {
                        break;
                    }
                    let pk = entry_res?.0.value();
                    results.push(Self::compose(&tx, &pk)?);
                    count += 1;
                }
                Ok(results)
            }
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = STORAGE.clone();
            let read_tx = storage.begin_read().expect("Failed to begin read transaction");
            let n: usize = 2;
            let entities = #entity_name::#fn_name(&read_tx, n).expect("Failed to take entities");
            let expected_entities = #entity_type::sample_many(n);
            assert_eq!(entities, expected_entities, "Expected to take 2 entities");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = STORAGE.clone();
            let read_tx = storage.begin_read().expect("Failed to begin read transaction");
            let n: usize = 2;
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, n).expect("Failed to take entities");
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
            params: vec![Query(QueryExpr {
                ty: syn::parse_quote!(TakeQuery),
                extraction: quote! { extract::Query(query): extract::Query<TakeQuery> },
                samples: quote! { vec![TakeQuery::sample()] },
            })],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    let read_tx = state.storage.begin_read()?;
                    let result = #entity_name::#fn_name(&read_tx, query.take)?;
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
