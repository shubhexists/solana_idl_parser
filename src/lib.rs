mod generator;
#[allow(dead_code)]
mod parser;

use proc_macro::TokenStream;
use std::path::PathBuf;

#[proc_macro]
pub fn parse_idl(input: TokenStream) -> TokenStream {
    let input_str = input.to_string();
    let path_str = input_str.trim().trim_matches('"');

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let idl_path = if PathBuf::from(path_str).is_absolute() {
        PathBuf::from(path_str)
    } else {
        PathBuf::from(&manifest_dir).join(path_str)
    };

    let idl_content = std::fs::read_to_string(&idl_path)
        .unwrap_or_else(|e| panic!("Failed to read IDL file at {:?}: {}", idl_path, e));

    let idl: parser::Idl =
        serde_json::from_str(&idl_content).unwrap_or_else(|e| panic!("Failed to parse IDL: {}", e));

    let generated = generator::generate_idl_code(&idl);
    generated.into()
}
