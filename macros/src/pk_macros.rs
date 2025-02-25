use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Data, Fields, Attribute};
use crate::entity_macros::{EntityMacros, Pk};

pub enum PointerType {
    Root,
    Child,
}

pub struct PkMacros {
    pub table_definition: TokenStream,
    pub store_statement: TokenStream,
    pub store_many_statement: TokenStream,
    pub delete_statement: TokenStream,
    pub delete_many_statement: TokenStream,
    pub functions: Vec<(String, TokenStream)>,
}

impl PkMacros {
    pub fn new(struct_name: &Ident, pk_column: &Pk) -> Self {
        let table_ident = format_ident!("{}_{}", struct_name.to_string().to_uppercase(), pk_column.field.name.to_string().to_uppercase());
        let table_name_str = table_ident.to_string();
        let pk_name: Ident = pk_column.field.name.clone();
        let pk_type = pk_column.field.tpe.clone();

        let table_definition = quote! {
            pub const #table_ident: ::redb::TableDefinition<'static, Bincode<#pk_type>, ()> = ::redb::TableDefinition::new(#table_name_str);
        };

        let store_statement = quote! {
            let mut table = write_tx.open_table(#table_ident)?;
            table.insert(&instance.#pk_name, ())?;
        };

        let store_many_statement = quote! {
            let mut table = write_tx.open_table(#table_ident)?;
            for instance in instances.iter() {
                table.insert(&instance.#pk_name, ())?;
            };
        };

        let delete_statement = quote! {
            let mut table = write_tx.open_table(#table_ident)?;
            let value = table.remove(pk)?;
            value.map(|g| g.value());
        };

        let delete_many_statement = quote! {
            let mut table = write_tx.open_table(#table_ident)?;
            for pk in pks.iter() {
                table.remove(pk)?;
            }
        };

        let mut functions: Vec<(String, TokenStream)> = Vec::new();
        let get_fn_name = format_ident!("get");
        functions.push((
            get_fn_name.to_string(),
            quote! {
                pub fn #get_fn_name(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<Option<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    if table.get(pk)?.is_some() {
                        Ok(Some(Self::compose(&read_tx, pk)?))
                    } else {
                        Ok(None)
                    }
                }
            },
        ));

        let all_fn_name = format_ident!("all");
        functions.push((
            all_fn_name.to_string(),
            quote! {
                pub fn #all_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Vec<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    let mut iter = table.iter()?;
                    let mut results = Vec::new();
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(Self::compose(&read_tx, &pk)?);
                    }
                    Ok(results)
                }
            },
        ));

        let first_fn_name = format_ident!("first");
        functions.push((
            first_fn_name.to_string(),
            quote! {
                pub fn #first_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Option<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    if let Some((k, _)) = table.last()? {
                        return Self::compose(&read_tx, &k.value()).map(Some);
                    }
                    Ok(None)
                }
            },
        ));

        let last_fn_name = format_ident!("last");
        functions.push((
            last_fn_name.to_string(),
            quote! {
                pub fn #last_fn_name(read_tx: &::redb::ReadTransaction) -> Result<Option<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    if let Some((k, _)) = table.last()? {
                        return Self::compose(&read_tx, &k.value()).map(Some);
                    }
                    Ok(None)
                }
            },
        ));

        if pk_column.range {
            let range_fn_name = format_ident!("range");
            functions.push((range_fn_name.to_string(), quote! {
                pub fn #range_fn_name(read_tx: &::redb::ReadTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#struct_name>, DbEngineError> {
                    let table = read_tx.open_table(#table_ident)?;
                    let range = from.clone()..until.clone();
                    let mut iter = table.range(range)?;
                    let mut results = Vec::new();
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(Self::compose(&read_tx, &pk)?);
                    }
                    Ok(results)
                }
            }));
            let pk_range_fn_name = format_ident!("pk_range");
            functions.push((pk_range_fn_name.to_string(), quote! {
                fn #pk_range_fn_name(write_tx: &::redb::WriteTransaction, from: &#pk_type, until: &#pk_type) -> Result<Vec<#pk_type>, DbEngineError> {
                    let table = write_tx.open_table(#table_ident)?;
                    let range = from.clone()..until.clone();
                    let mut iter = table.range(range)?;
                    let mut results = Vec::new();
                    while let Some(entry_res) = iter.next() {
                        let pk = entry_res?.0.value();
                        results.push(pk);
                    }
                    Ok(results)
                }
            }))
        };

        PkMacros { table_definition, store_statement, store_many_statement, delete_statement, delete_many_statement, functions }
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

                let parent_field = fields.iter().find(|f| Self::has_parent_attribute(&f.attrs)).unwrap().clone();
                let index_field = fields.iter().find(|f| !Self::has_parent_attribute(&f.attrs)).unwrap().clone();

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
            impl RootPointer for #struct_name {}

        };
        EntityMacros::write_to_file(&expanded, "redbit_pk_macros.rs").unwrap();
        expanded
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
        EntityMacros::write_to_file(&expanded, "redbit_pk_macros.rs").unwrap();
        expanded
    }

    /// Checks if a field has the `#[parent]` attribute.
    fn has_parent_attribute(attrs: &[Attribute]) -> bool {
        attrs.iter().any(|attr| attr.path().is_ident("parent"))
    }
}
