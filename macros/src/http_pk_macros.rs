use crate::entity_macros::{HttpEndpointMacro, Pk};
use proc_macro2::Ident;

pub fn new(struct_name: &Ident, pk_column: &Pk) -> Vec<HttpEndpointMacro> {
    vec![]
}