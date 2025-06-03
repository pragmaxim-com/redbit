use std::env;
use std::fs::OpenOptions;
use proc_macro2::TokenStream;
use quote::quote;
use std::io::Write;

pub fn write_stream_and_return(stream: TokenStream, file_path: &str) -> TokenStream {
    let formatted_str = match syn::parse2(stream.clone()) {
        Ok(ast) => prettyplease::unparse(&ast),
        Err(_) => stream.to_string(),
    };

    let dir = env::temp_dir().join("redbit");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        let msg = e.to_string();
        return quote! {
                compile_error!(#msg);
            };
    }

    let path = dir.join(format!("{}.rs", file_path));
    let write_result = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .and_then(|mut file| file.write_all(formatted_str.as_bytes()));

    if let Err(e) = write_result {
        let msg = format!("Failed to write to {:?}: {}", path, e);
        return quote! {
                compile_error!(#msg);
            };
    }
    stream
}
