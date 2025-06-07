use proc_macro2::Ident;
use syn::{Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
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

pub enum ParsingResult {
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

pub struct FieldDefs {
    pub pk: Pk,
    pub columns: Vec<Column>,
    pub relationships: Vec<Relationship>,
    pub transients: Vec<Transient>
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

pub fn get_field_macros(fields: &Punctuated<syn::Field, Comma>, ast: &DeriveInput) -> Result<FieldDefs, syn::Error> {
    let mut pk_column: Option<Pk> = None;
    let mut columns: Vec<Column> = Vec::new();
    let mut relationships: Vec<Relationship> = Vec::new();
    let mut transients: Vec<Transient> = Vec::new();

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
