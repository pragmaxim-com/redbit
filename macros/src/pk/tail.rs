use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::HttpParams::Query;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::parse_quote;

pub fn fn_def(entity_def: &EntityDef, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("tail");
    let entity_name = &entity_def.entity_name;
    let entity_type = &entity_def.entity_type;
    let read_ctx_type = &entity_def.read_ctx_type;
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_type, n: usize) -> Result<Vec<#entity_type>, AppError> {
            let Some((key_guard, _)) = tx_context.#table.last()? else {
                return Ok(Vec::new());
            };
            let key = key_guard.value();
            let until = key.next_index();
            let from = until.rollback_or_init(n as u32);
            let range = from..until;
            let iter = tx_context.#table.range(range)?;
            let mut queue = VecDeque::with_capacity(n);

            for entry_res in iter {
                let pk = entry_res?.0.value();
                if queue.len() == n {
                    queue.pop_front(); // remove oldest
                }
                queue.push_back(pk);
            }

            queue
                .into_iter()
                .map(|pk| Self::compose(tx_context, pk))
                .collect::<Result<Vec<#entity_type>, AppError>>()
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let n: usize = 2;
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entities = #entity_name::#fn_name(&tx_context, n).expect("Failed to tail entities");
            let mut expected_entities = #entity_type::sample_many(3);
            expected_entities.remove(0);
            assert_eq!(entities, expected_entities, "Expected to take last 2 entities");
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
                #entity_name::#fn_name(&tx_context, n).expect("Failed to tail entities");
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
                ty: syn::parse_quote!(TailQuery),
                extraction: quote! { extract::Query(query): extract::Query<TailQuery> },
                samples: quote! { vec![TailQuery::sample()] },
            })],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    let tx_context = #entity_name::begin_read_ctx(&state.storage)?;
                    let result = #entity_name::#fn_name(&tx_context, query.tail)?;
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
