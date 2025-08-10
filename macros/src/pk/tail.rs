use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromQuery;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, QueryExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn fn_def(entity_name: &Ident, entity_type: &Type, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("tail");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &ReadTransaction, n: usize) -> Result<Vec<#entity_type>, AppError> {
            let table_pk_12 = tx.open_table(#table)?;
            let Some((key_guard, _)) = table_pk_12.last()? else {
                return Ok(Vec::new());
            };
            let key = key_guard.value();
            let until = key.next_index();
            let from = until.rollback_or_init(n as u32);
            let range = from..until;
            let mut iter = table_pk_12.range(range)?;
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
                .map(|pk| Self::compose(tx, &pk))
                .collect::<Result<Vec<#entity_type>, AppError>>()
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let n: usize = 2;
            let entities = #entity_name::#fn_name(&read_tx, n).expect("Failed to tail entities");
            let mut expected_entities = #entity_type::sample_many(3);
            expected_entities.remove(0);
            assert_eq!(entities, expected_entities, "Expected to take last 2 entities");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let db = DB.clone();
            let read_tx = db.begin_read().expect("Failed to begin read transaction");
            let n: usize = 2;
            b.iter(|| {
                #entity_name::#fn_name(&read_tx, n).expect("Failed to tail entities");
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
            params: vec![FromQuery(QueryExpr {
                ty: syn::parse_quote!(TailQuery),
                extraction: quote! { extract::Query(query): extract::Query<TailQuery> },
                samples: quote! { vec![TailQuery::sample()] },
            })],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
               Result<AppJson<Vec<#entity_type>>, AppError> {
                    state.db.begin_read().map_err(AppError::from).and_then(|tx| #entity_name::#fn_name(&tx, query.tail)).map(AppJson)
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
