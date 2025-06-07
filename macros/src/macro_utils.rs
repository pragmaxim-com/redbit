use proc_macro2::{Ident, TokenStream};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;

pub fn write_to_local_file(lines: Vec<String>, dir_name: &str, entity: &Ident) {
    let dir_path = env::current_dir().unwrap().join("target/debug/examples").join(dir_name);
    if let Err(e) = std::fs::create_dir_all(&dir_path) {
        eprintln!("Failed to create directory {:?}: {}", dir_path, e);
        return;
    }
    let full_path = dir_path.join(entity.to_string());

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

pub fn write_stream_and_return(stream: TokenStream, entity: &Ident) -> TokenStream {
    let formatted_str = match syn::parse2(stream.clone()) {
        Ok(ast) => prettyplease::unparse(&ast),
        Err(_) => stream.to_string(),
    };
    write_to_local_file(vec![formatted_str], "macros", entity);
    stream
}
