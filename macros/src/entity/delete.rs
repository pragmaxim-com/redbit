use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::FromPath;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

pub fn delete_def(pk_type: &Type, delete_statements: &Vec<TokenStream>) -> FunctionDef {
    let fn_name = format_ident!("delete");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &StorageWriteTx, pk: &#pk_type) -> Result<bool, AppError> {
            let mut removed: Vec<bool> = Vec::new();
            #(#delete_statements)*
            Ok(!removed.contains(&false))
        }
    };
    FunctionDef { fn_stream, endpoint: None, test_stream: None, bench_stream: None }
}

pub fn delete_many_def(pk_type: &Type, delete_many_statements: &Vec<TokenStream>) -> FunctionDef {
    let fn_name = format_ident!("delete_many");
    let fn_stream = quote! {
        pub fn #fn_name(tx: &StorageWriteTx, pks: &Vec<#pk_type>) -> Result<bool, AppError> {
            let mut removed: Vec<bool> = Vec::new();
            #(#delete_many_statements)*
            Ok(!removed.contains(&false))
        }
    };
    FunctionDef { fn_stream, endpoint: None, test_stream: None, bench_stream: None }
}

pub fn delete_and_commit_def(
    entity_name: &Ident,
    entity_type: &Type,
    pk_name: &Ident,
    pk_type: &Type,
    delete_statements: &Vec<TokenStream>,
) -> FunctionDef {
    let fn_name = format_ident!("delete_and_commit");
    let fn_stream = quote! {
        pub fn #fn_name(storage: Arc<Storage>, pk: &#pk_type) -> Result<bool, AppError> {
            let tx = storage.begin_write()?;
            let mut removed: Vec<bool> = Vec::new();
            {
               #(#delete_statements)*
            }
            tx.commit()?;
            Ok(!removed.contains(&false))
       }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let storage = random_storage();
            let entity_count: usize = 3;
            for test_entity in #entity_type::sample_many(entity_count) {
                #entity_name::store_and_commit(Arc::clone(&storage), &test_entity).expect("Failed to store and commit instance");
                let pk = test_entity.#pk_name;
                let removed = #entity_name::#fn_name(Arc::clone(&storage), &pk).expect("Failed to delete and commit instance");
                let read_tx = storage.begin_read().expect("Failed to begin read transaction");
                let is_empty = #entity_name::get(&read_tx, &pk).expect("Failed to get instance").is_none();
                assert!(removed, "Instance should be deleted");
                assert!(is_empty, "Instance should be deleted");
            }
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let storage = random_storage();
            let test_entity = #entity_type::sample();
            #entity_name::store_and_commit(Arc::clone(&storage), &test_entity).expect("Failed to store and commit instance");
            let pk = test_entity.#pk_name;
            b.iter(|| {
                #entity_name::#fn_name(Arc::clone(&storage), &pk).expect("Failed to delete and commit instance");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            _entity_name: entity_name.clone(),
            tag: EndpointTag::DataDelete,
            fn_name: fn_name.clone(),
            params: vec![FromPath(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().url_encode() },
            }])],
            method: HttpMethod::DELETE,
            handler_name: format_ident!("{}", handler_fn_name),
            utoipa_responses: quote! {
                responses(
                    (status = OK),
                    (status = NOT_FOUND, content_type = "application/json", body = ErrorResponse),
                )
            },
            handler_impl_stream: quote! {
                impl IntoResponse {
                    match #entity_name::#fn_name(Arc::clone(&state.storage), &#pk_name) {
                        Ok(true) => {
                            Response::builder().status(StatusCode::OK).body(Body::empty()).unwrap().into_response()
                        },
                        Ok(false) => {
                            Response::builder().status(StatusCode::NOT_FOUND).body(Body::empty()).unwrap().into_response()
                        },
                        Err(err) => err.into_response(),
                    }
                }
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
        }.to_endpoint()),
        test_stream,
        bench_stream
    }
}
