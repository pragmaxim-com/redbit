extern crate proc_macro;

mod column_macros;
mod entity_macros;
mod pk_macros;
mod relationship_macros;

use crate::entity_macros::EntityMacros;
use crate::pk_macros::{PkMacros, PointerType};
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(PK, attributes(parent))]
pub fn derive_pk(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    let pointer_type = match PkMacros::extract_pointer_type(&ast) {
        Ok(t) => t,
        Err(e) => return e.to_compile_error().into(),
    };

    let (parent_field, index_field) = match PkMacros::extract_fields(&ast, &pointer_type) {
        Ok(fields) => fields,
        Err(e) => return e.to_compile_error().into(),
    };

    match pointer_type {
        PointerType::Root => PkMacros::generate_root_impls(struct_name, index_field).into(),
        PointerType::Child => PkMacros::generate_child_impls(struct_name, parent_field.unwrap(), index_field).into(),
    }
}

#[proc_macro_derive(Entity, attributes(pk, column, one2many, one2one, transient))]
pub fn derive_entity(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: DeriveInput = parse_macro_input!(input as DeriveInput);
    let struct_name = &ast.ident;

    let entity_macros =
        EntityMacros::get_named_fields(&ast)
            .and_then(|named_fields| {
                EntityMacros::get_field_macros(&named_fields, &ast)
            })
            .and_then(|field_macros| {
                EntityMacros::new(struct_name.clone(), field_macros)
            })
            .map_err(|err| err.to_compile_error()).expect("Failed to derive Entity");
    entity_macros.expand().into()
}
