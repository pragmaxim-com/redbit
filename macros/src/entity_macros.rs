use crate::column_macros::ColumnMacros;
use crate::pk_macros::PkMacros;
use crate::relationship_macros::RelationshipMacros;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use syn::{spanned::Spanned, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};
use syn::punctuated::Punctuated;
use syn::token::Comma;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indexing {
    Off,
    On { dictionary: bool, range: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Multiplicity {
    OneToOne,
    OneToMany,
}

enum ParsingResult {
    Pk(Pk),
    Column(Column),
    RelationShip(Relationship),
}

#[derive(Clone)]
pub struct Field {
    pub name: Ident,
    pub tpe: Type,
}

pub struct Pk {
    pub field: Field,
    pub range: bool,
}

pub struct Column {
    pub field: Field,
    pub indexing: Indexing,
}

#[derive(Clone)]
pub struct Relationship {
    pub field: Field,
    pub multiplicity: Multiplicity,
}

pub struct EntityMacros {
    pub struct_name: Ident,
    pub pk_column: (Pk, PkMacros),
    pub columns: Vec<(Column, ColumnMacros)>,
    pub relationships: Vec<(Relationship, RelationshipMacros)>,
}

impl EntityMacros {
    pub fn new(struct_name: Ident, pk_column: Pk, struct_columns: Vec<Column>, relationships: Vec<Relationship>) -> Result<EntityMacros, syn::Error> {
        let pk_name = &pk_column.field.name;
        let pk_type = &pk_column.field.tpe;
        let mut column_macros: Vec<(Column, ColumnMacros)> = Vec::new();
        for struct_column in struct_columns.into_iter() {
            let column_name = &struct_column.field.name.clone();
            let column_type = &struct_column.field.tpe.clone();
            match struct_column.indexing {
                Indexing::Off => {
                    column_macros.push((struct_column, ColumnMacros::simple(&struct_name, pk_name, pk_type, column_name, column_type)));
                }
                Indexing::On { dictionary: false, range } => {
                    column_macros.push((struct_column, ColumnMacros::indexed(&struct_name, pk_name, pk_type, column_name, column_type, range)));
                }
                Indexing::On { dictionary: true, range: false } => {
                    column_macros.push((struct_column, ColumnMacros::indexed_with_dict(&struct_name, pk_name, pk_type, column_name, column_type)));
                }
                Indexing::On { dictionary: true, range: true } => {
                    return Err(syn::Error::new(column_name.span(), "Range indexing on dictionary columns is not supported"))
                }
            }
        }
        // println!("Tables for {}:\n{}\n{}\n", struct_name, table_name_str, table_names.join("\n"));
        let pk_macros = PkMacros::new(&struct_name, &pk_column);
        let relationship_macros = RelationshipMacros::new(&pk_column, relationships);
        Ok(EntityMacros { struct_name, pk_column: (pk_column, pk_macros), columns: column_macros, relationships: relationship_macros })
    }

    pub fn expand(&self) -> TokenStream {
        let struct_ident = &self.struct_name;
        let (pk_column, pk_column_macros) = &self.pk_column;
        let pk_ident = pk_column.field.name.clone();
        let pk_type = pk_column.field.tpe.clone();
        let pk_table_definition = pk_column_macros.table_definition.clone();
        let pk_store_statement = pk_column_macros.store_statement.clone();
        let pk_store_many_statement = pk_column_macros.store_many_statement.clone();
        let pk_delete_statement = pk_column_macros.delete_statement.clone();

        let mut table_definitions = Vec::new();
        let mut store_statements = Vec::new();
        let mut store_many_statements = Vec::new();
        let mut struct_initializers = Vec::new();
        let mut delete_statements = Vec::new();
        let mut functions = Vec::new();
        functions.extend(pk_column_macros.functions.clone());

        for (_, macros) in &self.columns {
            table_definitions.extend(macros.table_definitions.clone());
            store_statements.push(macros.store_statement.clone());
            store_many_statements.push(macros.store_many_statement.clone());
            struct_initializers.push(macros.struct_initializer.clone());
            functions.extend(macros.functions.clone());
            delete_statements.push(macros.delete_statement.clone());
        }

        for (_, macros) in &self.relationships {
            store_statements.push(macros.store_statement.clone());
            store_many_statements.push(macros.store_many_statement.clone());
            struct_initializers.push(macros.struct_initializer.clone());
            functions.push(macros.query_function.clone());
            delete_statements.push(macros.delete_statement.clone());
        }
        let function_macros: Vec<TokenStream> = functions.into_iter().map(|f| f.1).collect::<Vec<_>>();
        let expanded = quote! {
            #pk_table_definition
            #(#table_definitions)*

            impl #struct_ident {
                #(#function_macros)*

                fn compose(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#struct_ident, DbEngineError> {
                    Ok(#struct_ident {
                        #pk_ident: pk.clone(),
                        #(#struct_initializers),*
                    })
                }

                pub fn delete(write_tx: &::redb::WriteTransaction, pk: &#pk_type) -> Result<(), DbEngineError> {
                    #pk_delete_statement
                    #(#delete_statements)*
                    Ok(())
                }

                pub fn delete_and_commit(db: &::redb::Database, pk: &#pk_type) -> Result<(), DbEngineError> {
                    let write_tx = db.begin_write()?;
                    {
                        #pk_delete_statement
                        #(#delete_statements)*
                    }
                    write_tx.commit()?;
                    Ok(())
                }

                pub fn store_many(write_tx: &::redb::WriteTransaction, instances: &Vec<#struct_ident>) -> Result<(), DbEngineError> {
                    #pk_store_many_statement
                    #(#store_many_statements)*
                    Ok(())
                }

                pub fn store(write_tx: &::redb::WriteTransaction, instance: &#struct_ident) -> Result<(), DbEngineError> {
                    #pk_store_statement
                    #(#store_statements)*
                    Ok(())
                }
                pub fn store_and_commit(db: &::redb::Database, instance: &#struct_ident) -> Result<(), DbEngineError> {
                    let write_tx = db.begin_write()?;
                    {
                        #pk_store_statement
                        #(#store_statements)*
                    }
                    write_tx.commit()?;
                    Ok(())
                }
            }
        };

        Self::write_to_file(&expanded, "redbit_entity_macros.rs").unwrap();
        expanded
    }

    pub fn write_to_file(stream: &TokenStream, file_path: &str) -> Result<(), std::io::Error> {
        let formatted_str = match syn::parse2(stream.clone()) {
            Ok(ast) => prettyplease::unparse(&ast),
            Err(_) => stream.to_string(),
        };
        let path = env::temp_dir().join(file_path);
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(formatted_str.as_bytes())
    }


    pub fn get_named_fields(ast: &DeriveInput) -> Result<Punctuated<syn::Field, Comma>, syn::Error> {
        match &ast.data {
            Data::Struct(data_struct) => match &data_struct.fields {
                Fields::Named(columns_named) => Ok(columns_named.named.clone()),
                _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` only supports structs with named columns.")),
            },
            _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` can only be applied to structs.")),
        }
    }

    fn parse_entity_field(field: &syn::Field) -> Result<ParsingResult, syn::Error> {
        match &field.ident {
            None => Err(syn::Error::new(field.span(), "Unnamed fields not supported")),
            Some(column_name) => {
                let column_type = field.ty.clone();
                for attr in &field.attrs {
                    if attr.path().is_ident("pk") {
                        let mut range = false;
                        let _ = attr.parse_nested_meta(|nested| {
                            if nested.path.is_ident("range") {
                                range = true;
                            }
                            Ok(())
                        });
                        let field = Field { name: column_name.clone(), tpe: column_type.clone() };
                        return Ok(ParsingResult::Pk(Pk { field, range }));
                    } else if attr.path().is_ident("column") {
                        let mut indexing = Indexing::Off;
                        let _ = attr.parse_nested_meta(|nested| {
                            if nested.path.is_ident("index") {
                                indexing = Indexing::On { dictionary: false, range: false };
                            }
                            if nested.path.is_ident("dictionary") {
                                indexing = Indexing::On { dictionary: true, range: false };
                            } else if nested.path.is_ident("range") {
                                indexing = Indexing::On { dictionary: false, range: true };
                            }
                            Ok(())
                        });
                        let field = Field { name: column_name.clone(), tpe: column_type.clone() };
                        return Ok(ParsingResult::Column(Column { field, indexing }));
                    } else if let Type::Path(type_path) = &column_type {
                        if let Some(segment) = type_path.path.segments.last() {
                            if attr.path().is_ident("one2many") && segment.ident == "Vec" {
                                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                    if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                        let inner_type = &inner_type_path.path.segments.last().unwrap().ident;
                                        let type_path = Type::Path(syn::TypePath { qself: None, path: syn::Path::from(inner_type.clone()) });
                                        let field = Field { name: column_name.clone(), tpe: type_path };
                                        return Ok(ParsingResult::RelationShip(Relationship { field, multiplicity: Multiplicity::OneToMany }));
                                    }
                                }
                            } else if attr.path().is_ident("one2one") && segment.arguments.is_empty() {
                                let struct_type = &segment.ident;
                                let type_path = Type::Path(syn::TypePath { qself: None, path: syn::Path::from(struct_type.clone()) });
                                let field = Field { name: column_name.clone(), tpe: type_path };
                                return Ok(ParsingResult::RelationShip(Relationship { field, multiplicity: Multiplicity::OneToOne }));
                            }
                        }
                    }
                }

                Err(syn::Error::new(
                    field.span(),
                    "Field must have one of #[pk(...)] / #[column(...)] / #[one2one] / #[one2many] annotations of expected underlying types",
                ))
            }
        }
    }

    pub fn get_pk_and_column_macros(fields: &Punctuated<syn::Field, Comma>, ast: &DeriveInput) -> Result<(Pk, Vec<Column>, Vec<Relationship>), syn::Error> {
        let mut pk_column: Option<Pk> = None;
        let mut columns: Vec<Column> = Vec::new();
        let mut relationships: Vec<Relationship> = Vec::new();

        for field in fields.iter() {
            match Self::parse_entity_field(field)? {
                ParsingResult::Column(column) => columns.push(column),
                ParsingResult::Pk(pk) => {
                    if pk_column.is_some() {
                        return Err(syn::Error::new(field.span(), "Multiple `#[pk]` columns found; only one is allowed"));
                    }
                    pk_column = Some(pk);
                }
                ParsingResult::RelationShip(relationship) => relationships.push(relationship),
            }
        }

        let pk_col =
            pk_column.ok_or_else(|| syn::Error::new(ast.span(), "`#[pk]` attribute not found on any column. Exactly one column must have `#[pk]`."))?;

        let one2many_rels_count = relationships.iter().filter(|rel| rel.multiplicity == Multiplicity::OneToMany).count();

        if one2many_rels_count > 1 {
            return Err(syn::Error::new(ast.span(), "Multiple `#[one2many]` relationships found. Only one relationship is allowed per entity."));
        }

        if columns.is_empty() && relationships.is_empty() {
            return Err(syn::Error::new(ast.span(), "No relationships or #[column(...)] fields found. You must have at least one of those."));
        }

        Ok((pk_col, columns, relationships))
    }

}
