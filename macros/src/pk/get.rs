use crate::endpoint::EndpointDef;
use crate::field_parser::EntityDef;
use crate::rest::HttpParams::Path;
use crate::rest::{EndpointTag, FunctionDef, HttpMethod, PathExpr};
use proc_macro2::Ident;
use quote::{format_ident, quote};

pub fn fn_def(entity_def: &EntityDef, table: &Ident) -> FunctionDef {
    let fn_name = format_ident!("get");
    let EntityDef { key_def, entity_name, entity_type, query_type:_, info_type:_, read_ctx_type, write_ctx_type:_ } = &entity_def;
    let key_def = key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;

    let fn_stream = quote! {
        pub fn #fn_name(tx_context: &#read_ctx_type, pk: #pk_type) -> Result<Option<#entity_type>, AppError> {
            if tx_context.#table.get(pk)?.is_some() {
                Ok(Some(Self::compose(&tx_context, pk)?))
            } else {
                Ok(None)
            }
        }
    };

    let test_stream = Some(quote! {
        #[test]
        fn #fn_name() {
            let (storage_owner, storage) = &*STORAGE;
            let pk_value = #pk_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            let entity = #entity_name::#fn_name(&tx_context, pk_value).expect("Failed to get entity by PK").expect("Expected entity to exist");
            let expected_enity = #entity_type::sample();
            assert_eq!(entity, expected_enity, "Entity PK does not match the requested PK");
        }
    });

    let bench_fn_name = format_ident!("_{}", fn_name);
    let bench_stream = Some(quote! {
        #[bench]
        fn #bench_fn_name(b: &mut Bencher) {
            let (storage_owner, storage) = &*STORAGE;
            let pk_value = #pk_type::default();
            let tx_context = #entity_name::begin_read_ctx(&storage).expect("Failed to begin read transaction context");
            b.iter(|| {
                #entity_name::#fn_name(&tx_context, pk_value).expect("Failed to get entity by PK").expect("Expected entity to exist");
            });
        }
    });

    let handler_fn_name = format!("{}_{}", entity_name.to_string().to_lowercase(), fn_name);

    FunctionDef {
        fn_stream,
        endpoint: Some(EndpointDef {
            return_type: Some(entity_type.clone()),
            tag: EndpointTag::DataRead,
            fn_name: fn_name.clone(),
            params: vec![Path(vec![PathExpr {
                name: pk_name.clone(),
                ty: pk_type.clone(),
                description: "Primary key".to_string(),
                sample: quote! { #pk_type::default().url_encode() },
            }])],
            method: HttpMethod::GET,
            handler_name: format_ident!("{}", handler_fn_name),
            handler_impl_stream: quote! {
              impl IntoResponse {
                 match #entity_name::begin_read_ctx(&state.storage)
                   .and_then(|tx_context| #entity_name::#fn_name(&tx_context, #pk_name) ) {
                       Ok(Some(entity)) => {
                           (StatusCode::OK, AppJson(entity)).into_response()
                       },
                       Ok(None) => {
                            let message = format!("{} not found", stringify!(#entity_name));
                            let response = ErrorResponse { message, code: StatusCode::NOT_FOUND.as_u16() };
                            (StatusCode::NOT_FOUND, AppJson(response)).into_response()
                       },
                       Err(err) => err.into_response(),
                   }
               }
            },
            utoipa_responses: quote! {
                responses(
                    (status = OK, content_type = "application/json", body = #entity_type),
                    (status = NOT_FOUND, content_type = "application/json", body = ErrorResponse)
                )
            },
            endpoint: format!("/{}/{}/{{{}}}", entity_name.to_string().to_lowercase(), pk_name, pk_name),
        }.to_endpoint()),
        test_stream,
        bench_stream,
    }
}
