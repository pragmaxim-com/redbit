use proc_macro2::{Ident, TokenStream};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use syn::Type;

pub fn write_to_local_file(lines: Vec<String>, dir_name: &str, entity: &Ident) {
    let dir_path = env::current_dir().expect("current dir inaccessible").join("target").join(dir_name);
    if let Err(e) = std::fs::create_dir_all(&dir_path) {
        eprintln!("Failed to create directory {:?}: {}", dir_path, e);
        return;
    }
    let full_path = dir_path.join(entity.to_string());

    #[cfg(not(test))]
    {
        if let Err(e) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&full_path)
            .and_then(|mut file| file.write_all(lines.join("\n").as_bytes()))
        {
            eprintln!("Failed to write to {:?}: {}", full_path, e);
        }
    }
}

pub fn write_stream_and_return(stream: TokenStream, entity: &Ident) -> TokenStream {
    let formatted_str = match syn::parse2(stream.clone()) {
        Ok(ast) => prettyplease::unparse(&ast),
        Err(_) => stream.to_string(),
    };
    write_to_local_file(vec![formatted_str], "macros", entity);
    stream
}

pub fn is_string(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        tp.path.is_ident("String")
    } else {
        false
    }
}

fn is_byte_array(ty: &Type) -> bool {
    matches!(ty, Type::Array(_)) && {
        if let Type::Array(arr) = ty {
            if let Type::Path(tp) = &*arr.elem {
                tp.path.is_ident("u8")
            } else {
                false
            }
        } else {
            false
        }
    }
}
pub fn is_vec_u8(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        tp.path.segments.last().map_or(false, |seg| {
            seg.ident == "Vec" && match &seg.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    args.args.iter().any(|arg| matches!(arg,
                        syn::GenericArgument::Type(Type::Path(p)) if p.path.is_ident("u8")))
                }
                _ => false,
            }
        })
    } else {
        false
    }
}

pub fn get_array_len(ty: &Type) -> Option<usize> {
    if let Type::Array(arr) = ty {
        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int), .. }) = &arr.len {
            return int.base10_parse::<usize>().ok();
        }
    }
    None
}

pub fn is_integer(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        let ident = &tp.path.segments.last().unwrap().ident;
        matches!(ident.to_string().as_str(), "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize")
    } else {
        false
    }
}

pub fn classify_inner_type(ty: &Type) -> InnerKind {
    if is_string(ty) {
        InnerKind::String
    } else if is_vec_u8(ty) {
        InnerKind::VecU8
    } else if is_byte_array(ty) {
        InnerKind::ByteArray(get_array_len(ty).unwrap())
    } else if is_integer(ty) {
        InnerKind::Integer
    } else {
        InnerKind::Other
    }
}

pub enum InnerKind {
    String,
    VecU8,
    ByteArray(usize),
    Integer,
    Other,
}
