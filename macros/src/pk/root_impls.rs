use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse_str, Field, Type};

/// Generates trait implementations for **Root Pointers** (IndexedPointer + RootPointer)
/// and also derives Display, FromStr, Serialize, and Deserialize based on a dash-separated format.
pub fn new(struct_name: &Ident, index_field: Field) -> TokenStream {
    let index_type = &index_field.ty;
    let struct_type: Type = parse_str(&format!("{}", struct_name)).expect("Invalid Struct type");
    quote! {
        impl ColInnerType for #struct_name {
            type Repr = #index_type;
        }

        impl UrlEncoded for #struct_name {
            fn url_encode(&self) -> String {
                format!("{}", self.0)
            }
        }

        impl Into<String> for #struct_name {
            fn into(self) -> String {
                self.url_encode()
            }
        }
        impl_redb_newtype_binary!(#struct_type);
        impl_cachekey_binary!(#struct_type);
        impl_indexed_pointer!(#struct_name, #index_type);
        impl_root_pointer!(#struct_name, #index_type);
        impl_binary_codec!(#struct_name, #index_type);
        impl_copy_owned_value_identity!(#struct_name);
        impl_tryfrom_pointer!(#struct_name, #index_type);
        impl_utoipa_partial_schema!(
            #struct_name,
            SchemaType::Type(Type::Integer),
            vec![0],
            Some(ExtensionsBuilder::new().add("key", "pk").build())
        );
        impl_utoipa_to_schema!(#struct_name);
    }

}
