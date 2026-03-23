pub mod render_markdown;
pub mod rust_options;
pub mod rust_wasm;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct DocsMetadata {
    pub options: OptionsSchema,
    pub wasm: WasmApi,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OptionsSchema {
    pub structs: HashMap<String, StructDocs>,
    pub enums: HashMap<String, EnumDocs>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StructDocs {
    pub name: String,
    pub docs: String,
    pub fields: Vec<FieldDocs>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FieldDocs {
    pub name: String,
    pub docs: String,
    pub ty: String,
    pub is_optional: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EnumDocs {
    pub name: String,
    pub docs: String,
    pub variants: Vec<VariantDocs>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariantDocs {
    pub name: String,
    pub docs: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct WasmApi {
    pub functions: Vec<FunctionDocs>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionDocs {
    pub name: String, // JS name
    pub rust_name: String,
    pub docs: String,
    pub params: Vec<ParamDocs>,
    pub returns: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParamDocs {
    pub name: String,
    pub ty: String,
}

pub fn extract_docs() -> anyhow::Result<DocsMetadata> {
    let options_path = "crates/geo-polygonize-core/src/options.rs";
    let wasm_path = "crates/geo-polygonize-wasm/src/lib.rs";

    let options = rust_options::parse_options(options_path)?;
    let wasm = rust_wasm::parse_wasm(wasm_path)?;

    Ok(DocsMetadata { options, wasm })
}
