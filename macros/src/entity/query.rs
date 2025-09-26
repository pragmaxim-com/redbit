use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Type;
use crate::field_parser::EntityDef;
use crate::macro_utils;

static FILTER_QUERY: &str = "FilterQuery";

pub fn filter_query_type(entity_type: &Type) -> Type {
    let suffix = FILTER_QUERY.to_string();
    let entity_ident = match entity_type {
        Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
        _ => panic!("Unsupported entity type for filter query"),
    };
    let filter_query_type = format_ident!("{}{}", entity_ident, suffix);
    syn::parse_quote!(#filter_query_type)
}

#[derive(Clone)]
pub struct FilterQueryItem {
    pub definition: TokenStream,
    pub init: TokenStream,
}

pub fn filter_query(filter_query_ty: &Type, filter_queries: &[FilterQueryItem]) -> TokenStream {
    let definitions: Vec<TokenStream> = filter_queries.iter().map(|item| item.definition.clone()).collect();
    let inits: Vec<TokenStream> = filter_queries.iter().map(|item| item.init.clone()).collect();
    quote! {
        #[derive(Clone, Debug, IntoParams, Serialize, Deserialize, Default, ToSchema)]
        #[schema(example = json!(#filter_query_ty::sample()))]
        pub struct #filter_query_ty {
            #(#definitions),*
        }
        impl #filter_query_ty {
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

pub fn pk_range_query(entity_def: &EntityDef) -> RangeQuery {
    let entity_name = &entity_def.entity_name;
    let key_def = &entity_def.key_def.field_def();
    let pk_name = &key_def.name;
    let pk_type = &key_def.tpe;
    let prefix = macro_utils::to_camel_case(&pk_name.to_string(), true);
    let entity_range_query = format_ident!("{}{}{}", entity_name.to_string(), prefix, "RangeQuery");
    let ty = syn::parse_quote!(#entity_range_query);

    let stream =
        quote! {
            #[derive(Clone, IntoParams, Serialize, Deserialize, Default)]
            pub struct #entity_range_query {
                pub from: #pk_type,
                pub until: #pk_type,
            }
            impl #entity_range_query {
                pub fn sample() -> Self {
                    Self {
                        from: #pk_type::default(),
                        until: #pk_type::default().next_index().next_index().next_index()
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
                        until: #tpe::default().nth_value(3)
                    }
                }
            }
        };
    RangeQuery {
        stream,
        ty,
    }
}