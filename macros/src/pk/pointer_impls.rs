use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_str, Field, Type};

pub fn new(struct_name: &Ident, parent_field: Field, index_field: Field) -> TokenStream {
    let parent_name = &parent_field.ident;
    let parent_type = &parent_field.ty;
    let index_name = &index_field.ident;
    let index_type = &index_field.ty;
    let struct_type: Type = parse_str(&format!("{}", struct_name)).expect("Invalid Struct type");
    quote! {
        impl Into<String> for #struct_name {
            fn into(self) -> String {
                self.url_encode()
            }
        }
        impl UrlEncoded for #struct_name {
            fn url_encode(&self) -> String {
                format!("{}-{}", self.#parent_name.url_encode(), self.#index_name)
            }
        }

        impl Sampleable for #struct_name {
            fn next_value(&self) -> Self {
                self.next_index()
            }
        }
        impl_redb_newtype_binary!(#struct_type);
        impl_cachekey_binary!(#struct_type);
        impl_indexed_pointer!(#struct_name, #index_type, #parent_name, #index_name);
        impl_child_pointer!(#struct_name, #parent_type, #parent_name, #index_type, #index_name);
        impl_binary_codec!(#struct_name, #parent_type, #index_type, #parent_name, #index_name);
        impl_copy_owned_value_identity!(#struct_name);
        impl_tryfrom_pointer!(#struct_name, #parent_type, #parent_name, #index_type, #index_name);
        impl_utoipa_partial_schema!(
            #struct_name,
            SchemaType::Type(Type::String),
            vec![Self::default().url_encode()],
            Some(ExtensionsBuilder::new().add("key", "fk").build())
        );
        impl_utoipa_to_schema!(#struct_name, #parent_type);
    }
}
