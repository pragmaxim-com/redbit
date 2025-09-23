use crate::endpoint::EndpointDef;
use crate::rest::HttpParams::Path;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;

pub fn delete_def(pk_type: &Type, tx_context_ty: &Type, delete_statements: &[TokenStream]) -> FunctionDef {
    let fn_name = format_ident!("delete");
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, pk: #pk_type) -> Result<bool, AppError> {
            let mut removed: Vec<bool> = Vec::new();
            #(#delete_statements)*
            Ok(!removed.contains(&false))
        }
    };
    FunctionDef { fn_stream, endpoint: None, test_stream: None, bench_stream: None }
}

pub fn delete_many_def(pk_type: &Type, tx_context_ty: &Type, delete_many_statements: &[TokenStream]) -> FunctionDef {
    let fn_name = format_ident!("delete_many");
    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#tx_context_ty, pks: &[#pk_type]) -> Result<bool, AppError> {
            let mut removed: Vec<bool> = Vec::new();
            #(#delete_many_statements)*
            Ok(!removed.contains(&false))
        }
    };
    FunctionDef { fn_stream, endpoint: None, test_stream: None, bench_stream: None }
}

pub fn remove_def(
    entity_name: &Ident,
    entity_type: &Type,
    pk_name: &Ident,
    pk_type: &Type,
    delete_statements: &[TokenStream],
) -> FunctionDef {
    let fn_name = format_ident!("remove");
    let fn_stream = quote! {
        pub fn #fn_name(storage: Arc<Storage>, pk: #pk_type) -> Result<bool, AppError> {
           let mut removed: Vec<bool> = Vec::new();
           let tx_context = #entity_name::begin_write_ctx(&storage)?;
           #(#delete_statements)*
           tx_context.commit_and_close_ctx()?;
           Ok(!removed.contains(&false))
       }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = random_storage();
            let entity_count: usize = 3;
            let entities = #entity_type::sample_many(entity_count);
            let tx_context = #entity_name::begin_write_ctx(&storage).expect("Failed to begin write transaction context");
            #entity_name::store_many(&tx_context, entities.clone()).expect("Failed to store many instances");
            tx_context.commit_and_close_ctx().expect("Failed to commit transaction context");

            for test_entity in entities {
                let pk = test_entity.#pk_name;
                let removed = #entity_name::#fn_name(Arc::clone(&storage), pk).expect("Failed to delete and commit instance");
                let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
                let is_empty = #entity_name::get(&tx_context, pk).expect("Failed to get instance").is_none();
                assert!(removed, "Instance should be deleted");
                assert!(is_empty, "Instance should be deleted");
            }
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = random_storage();
            let test_entity = #entity_type::sample();
            let pk = test_entity.#pk_name;
            #entity_name::persist(Arc::clone(&storage), test_entity).expect("Failed to store and commit instance");
            b.iter(|| {
                #entity_name::#fn_name(Arc::clone(&storage), pk).expect("Failed to delete and commit instance");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: None,
            tag: EndpointTag::DataDelete,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
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
                    match #entity_name::#fn_name(Arc::clone(&state.storage), #pk_name) {
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
