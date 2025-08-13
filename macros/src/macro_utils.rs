use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Attribute, ItemStruct, Path, Type};

pub fn extract_derives(attr: &Attribute) -> syn::Result<Vec<syn::Path>> {
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

pub fn is_byte_array(ty: &Type) -> bool {
    matches!(ty, Type::Array(arr) if matches!(&*arr.elem, Type::Path(tp) if tp.path.is_ident("u8")))
}

pub fn get_array_len(ty: &Type) -> Option<usize> {
    if let Type::Array(arr) = ty {
        if let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(int), .. }) = &arr.len {
            return int.base10_parse::<usize>().ok();
        }
    }
    None
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

pub fn is_integer(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        let ident = &tp.path.segments.last().unwrap().ident;
        matches!(ident.to_string().as_str(), "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "isize")
    } else {
        false
    }
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
        if segments.last().map(|s| s.ident == "DateTime") == Some(true) {
            if let Some(syn::PathArguments::AngleBracketed(gen_args)) = &segments.last().map(|s|s.arguments.clone()) {
                for arg in gen_args.args.iter() {
                    if let syn::GenericArgument::Type(Type::Path(p)) = arg {
                        if p.path.segments.last().is_some_and(|seg| seg.ident == "Utc") {
                            return true;
                        }
                    }
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

pub fn classify_inner_type(ty: &Type) -> InnerKind {
    if is_string(ty) {
        InnerKind::String
    } else if is_vec_u8(ty) {
        InnerKind::VecU8
    } else if is_byte_array(ty) {
        InnerKind::ByteArray(get_array_len(ty).unwrap())
    } else if is_integer(ty) {
        InnerKind::Integer
    } else if is_bool(ty) {
        InnerKind::Bool
    } else if is_uuid(ty) {
        InnerKind::Uuid
    } else if is_datetime_utc(ty) {
        InnerKind::Other // not existing Bincode impls
    } else if is_time(ty) {
        InnerKind::Time
    } else {
        InnerKind::Other
    }
}

#[derive(Debug)]
pub enum InnerKind {
    String,
    VecU8,
    ByteArray(usize),
    Integer,
    Bool,
    Uuid,
   // UtcDateTime,
    Time,
    Other,
}

pub fn write_to_local_file(lines: Vec<String>, dir_name: &str, file_name: &str) {
    let dir_path = env::current_dir().expect("current dir inaccessible").join("target").join("macros").join(dir_name);
    if let Err(e) = std::fs::create_dir_all(&dir_path) {
        eprintln!("Failed to create directory {:?}: {}", dir_path, e);
        return;
    }
    let full_path = dir_path.join(file_name);

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

pub fn submit_struct_to_stream(stream: proc_macro2::TokenStream, dir: &str, struct_ident: &Ident, suffix: &str) -> TokenStream {
    let formatted_token_stream =
        match syn::parse2::<syn::File>(stream.clone()) {
            Ok(ast) => prettyplease::unparse(&ast),
            Err(_) => stream.to_string(),
        };

    write_to_local_file(vec![formatted_token_stream], dir, &format!("{}{}", struct_ident, suffix));

    quote! {
        #stream
    }.into()
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
