use proc_macro2::Ident;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{Fields, GenericArgument, ItemStruct, PathArguments, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indexing {
    Off,
    On { dictionary: bool, range: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Multiplicity {
    OneToOption,
    OneToOne,
    OneToMany,
}

pub enum ParsingResult {
    Pk(PkDef),
    Column(ColumnDef),
    RelationShip(RelationshipDef),
    Transient(TransientDef),
}

#[derive(Clone)]
pub struct FieldDef {
    pub name: Ident,
    pub tpe: Type,
}

#[derive(Clone)]
pub struct PkDef {
    pub field: FieldDef,
    pub fk: Option<Multiplicity>,
    pub range: bool,
}

#[derive(Clone)]
pub struct ColumnDef {
    pub field: FieldDef,
    pub indexing: Indexing,
}

pub struct TransientDef {
    pub field: FieldDef,
}

#[derive(Clone)]
pub struct RelationshipDef {
    pub field: FieldDef,
    pub multiplicity: Multiplicity,
}

pub struct FieldDefs {
    pub pk: PkDef,
    pub columns: Vec<ColumnDef>,
    pub relationships: Vec<RelationshipDef>,
    pub transients: Vec<TransientDef>
}

pub fn get_named_fields(ast: &ItemStruct) -> Result<Punctuated<syn::Field, Comma>, syn::Error> {
    match &ast.fields {
        Fields::Named(columns_named) => Ok(columns_named.named.clone()),
        _ => Err(syn::Error::new(ast.span(), "`#[derive(Entity)]` only supports structs with named columns.")),
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
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ParsingResult::Pk(PkDef { field, fk: None, range }));
                } else if attr.path().is_ident("fk") {
                    let mut range = false;
                    let mut fk = None;
                    let _ = attr.parse_nested_meta(|nested| {
                        if nested.path.is_ident("range") {
                            range = true;
                        }
                        if nested.path.is_ident("one2many") {
                            fk = Some(Multiplicity::OneToMany);
                        } else if nested.path.is_ident("one2one") {
                            fk = Some(Multiplicity::OneToOne);
                        }
                        Ok(())
                    });
                    if fk.is_none() {
                        return Err(syn::Error::new(attr.span(), "Foreign key must specify either `one2many` or `one2one`"));
                    }
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ParsingResult::Pk(PkDef { field, fk, range }));
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
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ParsingResult::Column(ColumnDef { field, indexing }));
                } else if attr.path().is_ident("transient") {
                    let field = FieldDef { name: column_name.clone(), tpe: column_type.clone() };
                    return Ok(ParsingResult::Transient(TransientDef {field}))
                } else if let Type::Path(type_path) = &column_type {
                    if let Some(segment) = type_path.path.segments.last() {
                        if attr.path().is_ident("one2many") && segment.ident == "Vec" {
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first() {
                                    let inner_type =
                                        &inner_type_path.path.segments.last()
                                            .ok_or_else(|| syn::Error::new(field.span(), "Parent field missing"))?.ident;
                                    let type_path = Type::Path(syn::TypePath { qself: None, path: syn::Path::from(inner_type.clone()) });
                                    let field = FieldDef { name: column_name.clone(), tpe: type_path };
                                    return Ok(ParsingResult::RelationShip(RelationshipDef { field, multiplicity: Multiplicity::OneToMany }));
                                }
                            }
                        } else if attr.path().is_ident("one2one") {
                            // Default to OneToOne
                            let mut multiplicity = Multiplicity::OneToOne;
                            let mut actual_type = &field.ty;

                            // Check if the type is Option<T>
                            if let Type::Path(type_path) = &field.ty {
                                if let Some(segment) = type_path.path.segments.first() {
                                    if segment.ident == "Option" {
                                        // It is Option<T>, now get T
                                        if let PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                                            if let Some(GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                                                actual_type = inner_type;
                                                multiplicity = Multiplicity::OneToOption;
                                            }
                                        }
                                    }
                                }
                            }

                            // Now get the segment from the actual type
                            if let Type::Path(type_path) = actual_type {
                                if let Some(segment) = type_path.path.segments.last() {
                                    if segment.arguments.is_empty() {
                                        let struct_type = &segment.ident;
                                        let type_path = Type::Path(syn::TypePath { qself: None, path: syn::Path::from(struct_type.clone()) });
                                        let field = FieldDef {
                                            name: column_name.clone(),
                                            tpe: type_path,
                                        };
                                        return Ok(ParsingResult::RelationShip(RelationshipDef {
                                            field,
                                            multiplicity,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(syn::Error::new(
                field.span(),
                "Field must have one of #[pk(...)] / #[fk(...)] / #[column(...)] / #[one2one] / #[one2many] / #[transient] annotations of expected underlying types",
            ))
        }
    }
}

pub fn get_field_macros(fields: &Punctuated<syn::Field, Comma>, ast: &ItemStruct) -> Result<FieldDefs, syn::Error> {
    let mut pk_column: Option<PkDef> = None;
    let mut columns: Vec<ColumnDef> = Vec::new();
    let mut relationships: Vec<RelationshipDef> = Vec::new();
    let mut transients: Vec<TransientDef> = Vec::new();

    for field in fields.iter() {
        match parse_entity_field(field)? {
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

    Ok(FieldDefs {
        pk,
        columns,
        relationships,
        transients,
    })
}
