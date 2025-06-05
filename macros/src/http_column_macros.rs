use crate::entity_macros::HttpEndpointMacro;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::Type;

pub fn plain(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> Vec<HttpEndpointMacro> {
    vec![]
}

pub fn indexed(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type, range: bool) -> Vec<HttpEndpointMacro> {
    let mut endpoints: Vec<HttpEndpointMacro> = Vec::new();

    let get_by_name = format_ident!("get_by_{}", column_name);
    let handle_get_by_name = format_ident!("{}_handle_{}", struct_name.to_string().to_lowercase(), get_by_name);
    endpoints.push(HttpEndpointMacro {
        endpoint: format!("/{}/{}", struct_name.to_string().to_lowercase(), get_by_name),
        function_name: handle_get_by_name.clone(),
        handler: quote! {
             #[axum::debug_handler]
             pub async fn #handle_get_by_name(
                 ::axum::extract::State(state): ::axum::extract::State<RequestState>,
                 AppJson(params): AppJson<ByParams<#column_type>>,
             ) -> Result<AppJson<Vec<#struct_name>>, AppError> {
                 state.db.begin_read()
                     .map_err(|err| err.into())
                     .and_then(|read_tx| #struct_name::#get_by_name(&read_tx, &params.value))
                     .map(|result| AppJson(result))
             }
        },
    });

    if range {
        let range_by_name = format_ident!("range_by_{}", column_name);
        let handle_range_by_name = format_ident!("{}_handle_{}", struct_name.to_string().to_lowercase(), range_by_name);
        endpoints.push(HttpEndpointMacro {
            endpoint: format!("/{}/{}", struct_name.to_string().to_lowercase(), range_by_name),
            function_name: handle_range_by_name.clone(),
            handler: quote! {
                #[axum::debug_handler]
                pub async fn #handle_range_by_name(
                    ::axum::extract::State(state): ::axum::extract::State<RequestState>,
                    AppJson(params): AppJson<RangeParams<#column_type, #column_type>>,
                ) -> Result<AppJson<Vec<#struct_name>>, AppError> {
                    state.db.begin_read()
                        .map_err(|err| err.into())
                        .and_then(|read_tx| #struct_name::#range_by_name(&read_tx, &params.from, &params.to))
                        .map(|result| AppJson(result))
                }
            },
        });
    };
    endpoints
}

pub fn indexed_with_dict(struct_name: &Ident, pk_name: &Ident, pk_type: &Type, column_name: &Ident, column_type: &Type) -> Vec<HttpEndpointMacro> {
    vec![]
}
