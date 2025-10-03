
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;


#[cfg(feature = "expansion_structured")]
fn write_to_local_file(lines: Vec<String>, module_name: &str, dir_name: &str, file_name: &str) {
    use std::env;
    use std::fs::OpenOptions;
    use std::io::Write;

    let module_dir_path = env::current_dir()
        .unwrap()
        .join("target")
        .join("macros")
        .join(module_name);

    let structured_dir_path = module_dir_path.join(dir_name);
    if let Err(e) = std::fs::create_dir_all(&structured_dir_path) {
        eprintln!("Failed to create directory {:?}: {}", structured_dir_path, e);
        return;
    }

    let structured_full_path = structured_dir_path.join(file_name);
    if let Err(e) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&structured_full_path)
        .and_then(|mut file| file.write_all(lines.join("\n").as_bytes()))
    {
        eprintln!("Failed to write to {:?}: {}", structured_full_path, e);
    }
}

#[cfg(not(feature = "expansion_structured"))]
fn write_to_local_file(_: Vec<String>, _: &str, _: &str, _: &str) {
    // do nothing
}

pub fn submit_struct_to_stream(stream: proc_macro2::TokenStream, dir: &str, struct_ident: &Ident, suffix: &str) -> TokenStream {
    let lines = vec![
        match syn::parse2::<syn::File>(stream.clone()) {
            Ok(ast) => prettyplease::unparse(&ast),
            Err(_) => stream.to_string(),
        }];
    let span = struct_ident.span();
    let source_file = span.unwrap().source().file();
    let file_name = format!("{}{}", struct_ident, suffix);
    write_to_local_file(lines, &source_file, dir, &file_name);
    quote! {
        #stream
    }.into()
}
