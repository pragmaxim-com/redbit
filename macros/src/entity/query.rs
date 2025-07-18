use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::macro_utils;

static STREAM_QUERY: &str = "StreamQuery";

pub fn stream_query_type(entity_type: &Type) -> Type {
    let suffix = STREAM_QUERY.to_string();
    let entity_ident = match entity_type {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => panic!("Unsupported entity type for stream query"),
    };
    let stream_query_type = format_ident!("{}{}", entity_ident, suffix);
    syn::parse_quote!(#stream_query_type)
}

#[derive(Clone)]
pub struct StreamQueryItem {
    pub definition: TokenStream,
    pub init: TokenStream,
}

pub fn stream_query(stream_query_ty: &Type, stream_queries: &Vec<StreamQueryItem>) -> TokenStream {
    let definitions: Vec<TokenStream> = stream_queries.iter().map(|item| item.definition.clone()).collect();
    let inits: Vec<TokenStream> = stream_queries.iter().map(|item| item.init.clone()).collect();
    quote! {
        #[derive(Clone, Debug, IntoParams, Serialize, Deserialize, Default, ToSchema)]
        pub struct #stream_query_ty {
            #(#definitions),*
        }
        impl #stream_query_ty {
            pub fn sample() -> Self {
                Self {
                    #(#inits),*
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct RangeQuery {
    pub stream: TokenStream,
    pub ty: Type,
}

pub fn pk_range_query(entity_name: &Ident, field_ident: &Ident, tpe: &Type) -> RangeQuery {
    let prefix = macro_utils::to_camel_case(&field_ident.to_string(), true);
    let entity_range_query = format_ident!("{}{}{}", entity_name.to_string(), prefix, "RangeQuery");
    let ty = syn::parse_quote!(#entity_range_query);

    let stream =
        quote! {
            #[derive(Clone, IntoParams, Serialize, Deserialize, Default)]
            pub struct #entity_range_query {
                pub from: #tpe,
                pub until: #tpe,
            }
            impl #entity_range_query {
                pub fn sample() -> Self {
                    Self {
                        from: #tpe::default(),
                        until: #tpe::default().next_index().next_index().next_index()
                    }
                }
            }
        };
    RangeQuery {
        stream,
        ty,
    }
}

pub fn col_range_query(entity_name: &Ident, field_ident: &Ident, tpe: &Type) -> RangeQuery {
    let prefix = macro_utils::to_camel_case(&field_ident.to_string(), true);
    let entity_range_query = format_ident!("{}{}{}", entity_name.to_string(), prefix, "RangeQuery");
    let ty = syn::parse_quote!(#entity_range_query);

    let stream =
        quote! {
            #[derive(Clone, IntoParams, Serialize, Deserialize, Default)]
            pub struct #entity_range_query {
                pub from: #tpe,
                pub until: #tpe,
            }
            impl #entity_range_query {
                pub fn sample() -> Self {
                    Self {
                        from: #tpe::default(),
                        until: #tpe::default().next_value().next_value().next_value()
                    }
                }
            }
        };
    RangeQuery {
        stream,
        ty,
    }
}