use proc_macro2::Ident;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Fields, GenericArgument, ItemStruct, PathArguments, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Multiplicity {
    OneToOption,
    OneToOne,
    OneToMany,
}

#[derive(Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub tpe: Type,
}

#[derive(Clone)]
pub enum IndexingType {
    Off,
    On { dictionary: bool, range: bool },
}

#[derive(Clone)]
pub enum ColumnDef {
    Key { field_def: FieldDef, fk: Option<Multiplicity> },
    Plain(FieldDef, IndexingType),
    Relationship(FieldDef, Multiplicity),
    Transient(FieldDef),
}

pub fn get_named_fields(ast: &ItemStruct) -> Result<Punctuated<syn::Field, Comma>, syn::Error> {
    match &ast.fields {
        Fields::Named(columns_named) => Ok(columns_named.named.clone()),
        _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` only supports structs with named columns.")),
    }
}

fn parse_entity_field(field: &syn::Field) -> Result<ColumnDef, syn::Error> {
    match &field.ident {
        None => Err(syn::Error::new(field.span(), "Unnamed fields not supported")),
        Some(column_name) => {
            let column_type = field.ty.clone();
            for attr in &field.attrs {
                if attr.path().is_ident("pk") {
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ColumnDef::Key { field_def: field.clone(), fk: None });
                } else if attr.path().is_ident("fk") {
                    let mut fk = None;
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("one2many") {
                            fk = Some(Multiplicity::OneToMany);
                        } else if nested.path.is_ident("one2one") {
                            fk = Some(Multiplicity::OneToOne);
                        } else if nested.path.is_ident("one2opt") {
                            fk = Some(Multiplicity::OneToOption);
                        }
                        Ok(())
                    });
                    if fk.is_none() {
                        return Err(syn::Error::new(attr.span(), "Foreign key must specify either `one2many` or `one2one`"));
                    }
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ColumnDef::Key { field_def: field.clone(), fk });
                } else if attr.path().is_ident("column") {
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    let mut indexing = ColumnDef::Plain(field.clone(), IndexingType::Off);
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("transient") {
                            indexing = ColumnDef::Transient(field.clone());
                        } else if nested.path.is_ident("index") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: false, range: false });
                        } else if nested.path.is_ident("dictionary") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: true, range: false });
                        } else if nested.path.is_ident("range") {
                            indexing = ColumnDef::Plain(field.clone(), IndexingType::On { dictionary: false, range: true });
                        }
                        Ok(())
                    });
                    return Ok(indexing);
                }
            }
            if let Type::Path(type_path) = &column_type {
                if let Some(segment) = type_path.path.segments.last() {
                    match segment.ident.to_string().as_str() {
                        "Vec" => {
                            // one-to-many
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                    let inner_type = inner_type_path
                                        .path
                                        .segments
                                        .last()
                                        .ok_or_else(|| syn::Error::new(field.span(), "Parent field missing"))?
                                        .ident
                                        .clone();
                                    let type_path = Type::Path(syn::TypePath {
                                        qself: None,
                                        path: syn::Path::from(inner_type),
                                    });
                                    let field = FieldDef {
                                        name: column_name.clone(),
                                        tpe: type_path,
                                    };
                                    return Ok(ColumnDef::Relationship(field, Multiplicity::OneToMany));
                                }
                            }
                        }
                        "Option" => {
                            // one-to-option
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                    let inner_type = inner_type_path
                                        .path
                                        .segments
                                        .last()
                                        .ok_or_else(|| syn::Error::new(field.span(), "Parent field missing"))?
                                        .ident
                                        .clone();
                                    let type_path = Type::Path(syn::TypePath {
                                        qself: None,
                                        path: syn::Path::from(inner_type),
                                    });
                                    let field = FieldDef {
                                        name: column_name.clone(),
                                        tpe: type_path,
                                    };
                                    return Ok(ColumnDef::Relationship(field, Multiplicity::OneToOption));
                                }
                            }
                        }
                        _ => {
                            // one-to-one (plain type)
                            let struct_type = &segment.ident;
                            if segment.arguments.is_empty() {
                                let type_path = Type::Path(syn::TypePath {
                                    qself: None,
                                    path: syn::Path::from(struct_type.clone()),
                                });
                                let field = FieldDef {
                                    name: column_name.clone(),
                                    tpe: type_path,
                                };
                                return Ok(ColumnDef::Relationship(field, Multiplicity::OneToOne));
                            }
                        }
                    }
                }
            }
            Err(syn::Error::new(
                field.span(),
                "Field must have one of #[pk(...)] / #[fk(...)] / #[column(...)] / #[transient] annotations or it is a one2one, one2opt, or one2many relationship (e.g., `Vec<Transaction>` or `Option<Transaction>`).",
            ))
        }
    }
}

pub fn get_field_macros(fields: &Punctuated<syn::Field, Comma>, ast: &ItemStruct) -> Result<(FieldDef, Vec<ColumnDef>), syn::Error> {
    let mut pk_column: Option<FieldDef> = None;
    let mut columns: Vec<ColumnDef> = Vec::new();

    for field in fields.iter() {
        match parse_entity_field(field)? {
            ColumnDef::Key {field_def, fk}  => {
                if pk_column.is_some() {
                    return Err(syn::Error::new(field.span(), "Multiple `#[pk]` columns found; only one is allowed"));
                }
                pk_column = Some(field_def.clone());
                columns.push(ColumnDef::Key {field_def, fk});
            }
            column => columns.push(column),
        }
    }

    let pk = pk_column.ok_or_else(|| syn::Error::new(ast.span(), "`#[pk]` attribute not found on any column. Exactly one column must have `#[pk]`."))?;

    Ok((pk, columns))
}
