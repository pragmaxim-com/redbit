use heck::ToSnakeCase;
use proc_macro2::Ident;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Attribute, ItemStruct, Path, Type};

fn extract_derives(attr: &Attribute) -> syn::Result<Vec<syn::Path>> {
    let mut derives = Vec::new();
    attr.parse_nested_meta(|meta| {
        if let Some(ident) = meta.path.get_ident() {
            derives.push(syn::Path::from(ident.clone()));
            Ok(())
        } else {
            Err(meta.error("Expected identifier in derive"))
        }
    })?;
    Ok(derives)
}

pub(crate) fn one_to_many_field_name_from_type(inner_type: &Type) -> Ident {
    let inner_type_str = quote!(#inner_type).to_string(); // e.g. "Utxo"
    format_ident!("{}s", inner_type_str.to_snake_case())
}

pub(crate) fn one_to_many_field_name_from_ident(inner_type: &Ident) -> Ident {
    format_ident!("{}s", inner_type.to_string().to_snake_case())
}

pub fn merge_struct_derives(input: &mut ItemStruct, extra_derives: Punctuated<Path, Comma>) {
    let mut derives_vec: Vec<Path> = extra_derives.into_iter().collect();
    input.attrs.retain(|attr| {
        if attr.path().is_ident("derive") {
            match extract_derives(attr) {
                Ok(paths) => derives_vec.extend(paths),
                Err(e) => {
                    eprintln!("Error parsing derive attribute: {}", e);
                    return true
                }
            }
            false
        } else {
            true
        }
    });

    derives_vec.sort_by(|a, b| quote!(#a).to_string().cmp(&quote!(#b).to_string()));
    derives_vec.dedup_by(|a, b| quote!(#a).to_string() == quote!(#b).to_string());

    // Reinsert merged derive attribute
    input.attrs.push(syn::parse_quote! {
        #[derive(#(#derives_vec),*)]
    });

}

pub fn is_string(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.path.is_ident("String"))
}

pub fn is_bool(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if tp.path.is_ident("bool"))
}

pub fn is_vec_u8(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if {
        tp.path.segments.last().is_some_and(|seg| {
            seg.ident == "Vec" && matches!(&seg.arguments, syn::PathArguments::AngleBracketed(args) if {
                args.args.iter().any(|arg| matches!(arg,
                    syn::GenericArgument::Type(Type::Path(p)) if p.path.is_ident("u8")))
            })
        })
    })
}

pub fn is_uuid(ty: &Type) -> bool {
    matches!(ty, Type::Path(tp) if {
        tp.path.segments.last().is_some_and(|seg| seg.ident == "Uuid")
            && tp.path.segments.iter().any(|s| s.ident == "uuid")
    })
}

pub fn is_datetime_utc(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        let segments = &tp.path.segments;
        if segments.last().map(|s| s.ident == "DateTime") == Some(true)
            && let Some(syn::PathArguments::AngleBracketed(gen_args)) = &segments.last().map(|s|s.arguments.clone()) {
                for arg in gen_args.args.iter() {
                    if let syn::GenericArgument::Type(Type::Path(p)) = arg && p.path.segments.last().is_some_and(|seg| seg.ident == "Utc") {
                        return true;
                    }
                }
            }
    }
    false
}

fn is_time(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        tp.path.segments.last().is_some_and(|seg| seg.ident == "Duration")
    } else {
        false
    }
}

pub fn classify_integer_type(ty: &Type) -> Option<IntegerType> {
    use syn::{TypePath};
    let Type::Path(TypePath { path, .. }) = ty else { return None };
    let ident = path.get_ident()?;
    match ident.to_string().as_str() {
        "u8"    => Some(IntegerType::U8),
        "u16"   => Some(IntegerType::U16),
        "u32"   => Some(IntegerType::U32),
        "u64"   => Some(IntegerType::U64),
        "u128"  => Some(IntegerType::U128),
        "usize" => Some(IntegerType::Usize),
        "i8"    => Some(IntegerType::I8),
        "i16"   => Some(IntegerType::I16),
        "i32"   => Some(IntegerType::I32),
        "i64"   => Some(IntegerType::I64),
        "i128"  => Some(IntegerType::I128),
        "isize" => Some(IntegerType::Isize),
        _ => None,
    }
}

pub fn extract_int_type(ty: &Type) -> InnerKind {
    match classify_integer_type(ty) {
        Some(k) => InnerKind::Integer(k),
        None => InnerKind::Other,
    }
}

pub fn classify_inner_type(ty: &Type) -> InnerKind {
    if is_string(ty) {
        InnerKind::String
    } else if is_vec_u8(ty) {
        InnerKind::VecU8
    } else if is_bool(ty) {
        InnerKind::Bool
    } else if is_uuid(ty) {
        InnerKind::Uuid
    } else if is_datetime_utc(ty) {
        InnerKind::Other // not existing Bincode impls
    } else if is_time(ty) {
        InnerKind::Time
    } else {
        if let Type::Array(arr) = ty
            && let Type::Path(tp) = &*arr.elem
            && let Some(seg) = tp.path.segments.last()
            && seg.ident == "u8"
            && let syn::Expr::Lit(expr_lit) = &arr.len
            && let syn::Lit::Int(int_lit) = &expr_lit.lit
            && let Ok(n) = int_lit.base10_parse::<usize>() {
                return InnerKind::ByteArray(n);
            }
        extract_int_type(ty)
    }
}

#[derive(Debug)]
pub enum IntegerType {
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
}

impl IntegerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            IntegerType::U8 => "u8",
            IntegerType::U16 => "u16",
            IntegerType::U32 => "u32",
            IntegerType::U64 => "u64",
            IntegerType::U128 => "u128",
            IntegerType::Usize => "usize",
            IntegerType::I8 => "i8",
            IntegerType::I16 => "i16",
            IntegerType::I32 => "i32",
            IntegerType::I64 => "i64",
            IntegerType::I128 => "i128",
            IntegerType::Isize => "isize",
        }
    }
}

#[derive(Debug)]
pub enum InnerKind {
    String,
    VecU8,
    ByteArray(usize),
    Integer(IntegerType),
    Bool,
    Uuid,
   // UtcDateTime,
    Time,
    Other,
}

pub fn to_camel_case(input: &str, upper_first_char: bool) -> String {
    let mut result = String::with_capacity(input.len());
    for (i, word) in input.split('_').enumerate() {
        if word.is_empty() {
            continue; // Skip consecutive underscores
        }
        if i == 0 {
            if upper_first_char {
                let mut chars: Vec<char> = word.to_lowercase().chars().collect();
                chars[0] = chars[0].to_uppercase().next().unwrap();
                let upper_first_char_word: String = chars.into_iter().collect();
                result.push_str(&upper_first_char_word);
            } else {
                result.push_str(&word.to_lowercase());
            }
        } else {
            // Capitalize the first character of each subsequent word
            let mut chars = word.chars();
            if let Some(first) = chars.next() {
                result.push_str(&first.to_uppercase().to_string());
                result.push_str(&chars.as_str().to_lowercase());
            }
        }
    }
    result
}
