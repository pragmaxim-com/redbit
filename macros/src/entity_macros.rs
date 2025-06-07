use crate::db_column_macros::{DbColumnMacros};
use crate::db_pk_macros::DbPkMacros;
use crate::db_relationship_macros::{DbRelationshipMacros, TransientMacros};
use crate::http_macros::{to_http_endpoint, FunctionDef, HttpEndpointMacro};
use crate::macro_utils;

use proc_macro2::{Ident, TokenStream};
use quote::quote;
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
    Transient(Transient),
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

#[derive(Clone)]
pub struct Column {
    pub field: Field,
    pub indexing: Indexing,
}

pub struct Transient {
    pub field: Field,
}

#[derive(Clone)]
pub struct Relationship {
    pub field: Field,
    pub multiplicity: Multiplicity,
}

pub struct EntityMacros {
    pub struct_name: Ident,
    pub pk: (Pk, DbPkMacros),
    pub columns: Vec<(Column, DbColumnMacros)>,
    pub relationships: Vec<(Relationship, DbRelationshipMacros)>,
    pub transients: Vec<(Transient, TransientMacros)>,
}

pub struct FieldMacros {
    pub pk: Pk,
    pub columns: Vec<Column>,
    pub relationships: Vec<Relationship>,
    pub transients: Vec<Transient>
}

impl EntityMacros {
    pub fn new(entity_ident: Ident, field_macros: FieldMacros) -> Result<EntityMacros, syn::Error> {
        let FieldMacros { pk, columns, relationships, transients } = field_macros;
        let pk_name = &pk.field.name;
        let pk_type = &pk.field.tpe;
        let mut column_macros: Vec<(Column, DbColumnMacros)> = Vec::new();
        for entity_column in columns.into_iter() {
            let column_name = &entity_column.field.name.clone();
            let column_type = &entity_column.field.tpe.clone();
            match entity_column.indexing {
                Indexing::Off => {
                    column_macros.push((
                        entity_column.clone(),
                        DbColumnMacros::plain(&entity_ident, pk_name, pk_type, column_name, column_type),
                    ));
                }
                Indexing::On { dictionary: false, range } => {
                    column_macros.push((
                        entity_column.clone(),
                        DbColumnMacros::indexed(&entity_ident, pk_name, pk_type, column_name, column_type, range),
                    ));
                }
                Indexing::On { dictionary: true, range: false } => {
                    column_macros.push((
                        entity_column.clone(),
                        DbColumnMacros::indexed_with_dict(&entity_ident, pk_name, pk_type, column_name, column_type),
                    ));
                }
                Indexing::On { dictionary: true, range: true } => {
                    return Err(syn::Error::new(column_name.span(), "Range indexing on dictionary columns is not supported"))
                }
            }
        }

        let db_pk_macro = DbPkMacros::new(&entity_ident, &pk);
        let mut relationship_macros: Vec<(Relationship, DbRelationshipMacros)> = Vec::new();
        for rel in relationships.iter() {
            let db_relationship_macros = DbRelationshipMacros::new(&entity_ident, &pk, rel.clone());
            relationship_macros.push((rel.clone(), db_relationship_macros));
        }
        let transient_macros = TransientMacros::new(transients);
        Ok(EntityMacros {
            struct_name: entity_ident,
            pk: (pk, db_pk_macro),
            columns: column_macros,
            relationships: relationship_macros,
            transients: transient_macros
        })
    }

    pub fn expand(&self) -> TokenStream {
        let struct_ident = &self.struct_name;
        let (pk_column, db_pk_macros) = &self.pk;
        let pk_ident = pk_column.field.name.clone();
        let pk_type = pk_column.field.tpe.clone();
        let pk_table_name = db_pk_macros.table_name.clone();
        let pk_table_definition = db_pk_macros.table_definition.clone();
        let pk_store_statement = db_pk_macros.store_statement.clone();
        let pk_store_many_statement = db_pk_macros.store_many_statement.clone();
        let pk_delete_statement = db_pk_macros.delete_statement.clone();
        let pk_delete_many_statement = db_pk_macros.delete_many_statement.clone();

        let mut table_definitions = Vec::new();
        let mut store_statements = Vec::new();
        let mut store_many_statements = Vec::new();
        let mut struct_initializers = Vec::new();
        let mut delete_statements = Vec::new();
        let mut delete_many_statements = Vec::new();
        let mut function_defs: Vec<FunctionDef> = Vec::new();
        function_defs.extend(db_pk_macros.function_defs.clone());

        for (_, db_column_macros) in &self.columns {
            table_definitions.extend(db_column_macros.table_definitions.clone());
            struct_initializers.push(db_column_macros.struct_initializer.clone());
            store_statements.push(db_column_macros.store_statement.clone());
            store_many_statements.push(db_column_macros.store_many_statement.clone());
            function_defs.extend(db_column_macros.function_defs.clone());
            delete_statements.push(db_column_macros.delete_statement.clone());
            delete_many_statements.push(db_column_macros.delete_many_statement.clone());
        }

        for (_, db_relationship_macros) in &self.relationships {
            struct_initializers.push(db_relationship_macros.struct_initializer.clone());
            store_statements.push(db_relationship_macros.store_statement.clone());
            store_many_statements.push(db_relationship_macros.store_many_statement.clone());
            function_defs.push(db_relationship_macros.function_def.clone());
            delete_statements.push(db_relationship_macros.delete_statement.clone());
            delete_many_statements.push(db_relationship_macros.delete_many_statement.clone());
        }

        for (_, macros) in &self.transients {
            struct_initializers.push(macros.struct_initializer.clone());
        }
        let function_streams: Vec<TokenStream> = function_defs.iter().map(|f| f.stream.clone()).collect::<Vec<_>>();
        let table_definition_names: Vec<String> = table_definitions.iter().map(|(name, _)| name.to_string()).collect();
        let table_definition_streams: Vec<TokenStream> = table_definitions.into_iter().map(|(_, stream)| stream).collect();
        let endpoints: Vec<HttpEndpointMacro> = function_defs.iter().filter_map(|fn_def| to_http_endpoint(fn_def)).collect();

        let mut table_lines = Vec::new();
        table_lines.push(format!("| PK_Table      |  {}", &pk_table_name));
        for column_table_name in &table_definition_names {
            table_lines.push(format!("| Index_Table   |  {}", column_table_name));
        }
        macro_utils::write_to_local_file(table_lines, "tables", &struct_ident);

        let mut entity_lines = Vec::new();
        for endpoint in &endpoints {
            entity_lines.push(format!("| Endpoint      |  {}", endpoint.endpoint));
        }
        macro_utils::write_to_local_file(entity_lines, "endpoints", &struct_ident);

        let endpoint_macros: Vec<TokenStream> = endpoints.iter().map(|e| e.handler.clone()).collect();
        let route_chains: Vec<TokenStream> =
            endpoints
                .into_iter()
                .map(|e| (e.endpoint, e.fn_name))
                .map(|(endpoint, function_name)| {
                    quote! {
                        .route(#endpoint, ::axum::routing::get(#function_name))
                    }
                })
                .collect();

        let expanded = quote! {
            // table definitions are not in the impl object because they are accessed globally with semantic meaning
            #pk_table_definition
            #(#table_definition_streams)*
            // axum endpoints cannot be in the impl object https://docs.rs/axum/latest/axum/attr.debug_handler.html#limitations
            #(#endpoint_macros)*

            impl #struct_ident {

                #(#function_streams)*

                fn compose(read_tx: &::redb::ReadTransaction, pk: &#pk_type) -> Result<#struct_ident, AppError> {
                    Ok(#struct_ident {
                        #pk_ident: pk.clone(),
                        #(#struct_initializers),*
                    })
                }

                pub fn delete(write_tx: &::redb::WriteTransaction, pk: &#pk_type) -> Result<(), AppError> {
                    #pk_delete_statement
                    #(#delete_statements)*
                    Ok(())
                }

                pub fn delete_many(write_tx: &::redb::WriteTransaction, pks: &Vec<#pk_type>) -> Result<(), AppError> {
                    #pk_delete_many_statement
                    #(#delete_many_statements)*
                    Ok(())
                }

                pub fn delete_and_commit(db: &::redb::Database, pk: &#pk_type) -> Result<(), AppError> {
                    let write_tx = db.begin_write()?;
                    {
                        #pk_delete_statement
                        #(#delete_statements)*
                    }
                    write_tx.commit()?;
                    Ok(())
                }

                pub fn store_many(write_tx: &::redb::WriteTransaction, instances: &Vec<#struct_ident>) -> Result<(), AppError> {
                    #pk_store_many_statement
                    #(#store_many_statements)*
                    Ok(())
                }

                pub fn store(write_tx: &::redb::WriteTransaction, instance: &#struct_ident) -> Result<(), AppError> {
                    #pk_store_statement
                    #(#store_statements)*
                    Ok(())
                }
                pub fn store_and_commit(db: &::redb::Database, instance: &#struct_ident) -> Result<(), AppError> {
                    let write_tx = db.begin_write()?;
                    {
                        #pk_store_statement
                        #(#store_statements)*
                    }
                    write_tx.commit()?;
                    Ok(())
                }

                    pub fn routes() -> axum::Router<RequestState> {
                        axum::Router::new()
                            #(#route_chains)*
                }
            }
        };
        eprintln!("----------------------------------------------------------");
        macro_utils::write_stream_and_return(expanded, &struct_ident)
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
                    } else if attr.path().is_ident("transient") {
                        let field = Field { name: column_name.clone(), tpe: column_type.clone() };
                        return Ok(ParsingResult::Transient(Transient{field}))
                    } else if let Type::Path(type_path) = &column_type {
                        if let Some(segment) = type_path.path.segments.last() {
                            if attr.path().is_ident("one2many") && segment.ident == "Vec" {
                                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                    if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                        let inner_type =
                                            &inner_type_path.path.segments.last()
                                                .ok_or_else(|| syn::Error::new(field.span(), "Parent field missing"))?.ident;
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
                    "Field must have one of #[pk(...)] / #[column(...)] / #[one2one] / #[one2many] / #[transient] annotations of expected underlying types",
                ))
            }
        }
    }

    pub fn get_field_macros(fields: &Punctuated<syn::Field, Comma>, ast: &DeriveInput) -> Result<FieldMacros, syn::Error> {
        let mut pk_column: Option<Pk> = None;
        let mut columns: Vec<Column> = Vec::new();
        let mut relationships: Vec<Relationship> = Vec::new();
        let mut transients: Vec<Transient> = Vec::new();

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
                ParsingResult::Transient(transient) => transients.push(transient),
            }
        }

        let pk = pk_column.ok_or_else(|| syn::Error::new(ast.span(), "`#[pk]` attribute not found on any column. Exactly one column must have `#[pk]`."))?;

        Ok(FieldMacros {
            pk,
            columns,
            relationships,
            transients,
        })
    }
}
