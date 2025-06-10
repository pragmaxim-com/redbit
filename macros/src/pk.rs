mod exists;
mod get;
mod take;
mod first;
mod last;
mod range;
mod pk_range;
mod store;
mod delete;
mod parent_pk;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Type};
use crate::field_parser::{Multiplicity, PkDef};
use crate::http::FunctionDef;
use crate::macro_utils;
use crate::table::TableDef;

pub enum PointerType {
    Root,
    Child,
}

pub struct DbPkMacros {
    pub definition: PkDef,
    pub table_def: TableDef,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub function_defs: Vec<FunctionDef>,
}

impl DbPkMacros {
    pub fn new(entity_name: &Ident, entity_type: &Type, pk_def: &PkDef) -> Self {
        let pk_name: Ident = pk_def.field.name.clone();
        let pk_type = pk_def.field.tpe.clone();
        let table_def = TableDef::pk(entity_name, &pk_name, &pk_type);

        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.push(get::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.name));
        function_defs.push(take::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(first::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(last::fn_def(entity_name, entity_type, &table_def.name));
        function_defs.push(exists::fn_def(entity_name, &pk_name, &pk_type, &table_def.name));

        match pk_def.fk {
            Some(Multiplicity::OneToMany) => {
                function_defs.push(parent_pk::fn_def(entity_name, &pk_name, &pk_type));
            },
            _ => {
            }
        };

        if pk_def.range {
            function_defs.push(range::fn_def(entity_name, entity_type, &pk_name, &pk_type, &table_def.name));
            function_defs.push(pk_range::fn_def(entity_name, &pk_name, &pk_type, &table_def.name));
        };

        DbPkMacros {
            definition: pk_def.clone(),
            table_def: table_def.clone(),
            store_statement: store::store_statement(&pk_name, &table_def.name),
            store_many_statement: store::store_many_statement(&pk_name, &table_def.name),
            delete_statement: delete::delete_statement(&table_def.name),
            delete_many_statement: delete::delete_many_statement(&table_def.name),
            function_defs
        }
    }

    /// Determines whether a struct is a `Root` or `Child` based on `#[parent]` attributes.
    pub fn extract_pointer_type(input: &DeriveInput) -> Result<PointerType, syn::Error> {
        let data_struct = match &input.data {
            Data::Struct(data_struct) => data_struct,
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be derived for structs")),
        };

        let fields: Vec<_> = match &data_struct.fields {
            Fields::Named(fields) => fields.named.iter().collect(),
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be used with named fields")),
        };

        if fields.iter().any(|f| Self::has_parent_attribute(&f.attrs)) {
            Ok(PointerType::Child)
        } else {
            Ok(PointerType::Root)
        }
    }

    /// Extracts and validates the required fields (parent & index) for root or child structs.
    pub fn extract_fields(input: &DeriveInput, pointer_type: &PointerType) -> Result<(Option<syn::Field>, syn::Field), syn::Error> {
        let data_struct = match input.data.clone() {
            Data::Struct(data_struct) => data_struct,
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be derived for structs")),
        };

        let fields: Vec<_> = match data_struct.fields {
            Fields::Named(fields) => fields.named.into_iter().collect(),
            _ => return Err(syn::Error::new_spanned(input, "Pk can only be used with named fields")),
        };

        match pointer_type {
            PointerType::Root => {
                if fields.len() != 1 {
                    return Err(syn::Error::new_spanned(input, "Root struct must have exactly one field"));
                }
                Ok((None, fields[0].clone())) // Root has only an index field
            }
            PointerType::Child => {
                if fields.len() != 2 {
                    return Err(syn::Error::new_spanned(input, "Child struct must have exactly two fields (parent and index)"));
                }

                let parent_field = match fields.iter().find(|f| Self::has_parent_attribute(&f.attrs)) {
                    Some(f) => f.clone(),
                    None => return Err(syn::Error::new_spanned(input, "Unable to find parent field")),
                };
                let index_field = match fields.iter().find(|f| !Self::has_parent_attribute(&f.attrs)) {
                    Some(f) => f.clone(),
                    None => return Err(syn::Error::new_spanned(input, "Unable to find index field")),
                };

                Ok((Some(parent_field), index_field))
            }
        }
    }

    /// Generates trait implementations for **Root Pointers**.
    pub fn generate_root_impls(struct_name: &Ident, index_field: syn::Field) -> TokenStream {
        let index_type = &index_field.ty;
        let index_name = &index_field.ident;

        let expanded =
            quote! {
            impl IndexedPointer for #struct_name {
                type Index = #index_type;

                fn index(&self) -> Self::Index {
                    self.#index_name
                }

                fn next(&self) -> Self {
                    #struct_name { #index_name: self.#index_name + 1 }
                }
            }
            impl RootPointer for #struct_name {
                fn is_child(&self) -> bool {
                    false
                }
            }

        };
        macro_utils::write_stream_and_return(expanded, &struct_name)
    }

    /// Generates trait implementations for **Child Pointers**.
    pub fn generate_child_impls(struct_name: &Ident, parent_field: syn::Field, index_field: syn::Field) -> TokenStream {
        let parent_name = &parent_field.ident;
        let parent_type = &parent_field.ty;
        let index_name = &index_field.ident;
        let index_type = &index_field.ty;
        let expanded =
            quote! {
                impl IndexedPointer for #struct_name {
                    type Index = #index_type;

                    fn index(&self) -> Self::Index {
                        self.#index_name
                    }

                    fn next(&self) -> Self {
                        #struct_name {
                            #parent_name: self.#parent_name.clone(),
                            #index_name: self.#index_name + 1,
                        }
                    }
                }
                impl ChildPointer for #struct_name {
                    type Parent = #parent_type;

                    fn is_child(&self) -> bool {
                        true
                    }
                    fn parent(&self) -> &Self::Parent {
                        &self.#parent_name
                    }

                    fn from_parent(parent: Self::Parent) -> Self {
                        #struct_name {
                            #parent_name: parent,
                            #index_name: <#index_type as Default>::default(),
                        }
                    }
                }
            };
        macro_utils::write_stream_and_return(expanded, &struct_name)
    }

    /// Checks if a field has the `#[parent]` attribute.
    fn has_parent_attribute(attrs: &[Attribute]) -> bool {
        attrs.iter().any(|attr| attr.path().is_ident("parent"))
    }
}
