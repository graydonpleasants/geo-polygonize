use super::{FunctionDocs, ParamDocs, WasmApi};
use anyhow::Result;
use std::fs;
use syn::{Item, ReturnType};

pub fn parse_wasm(path: &str) -> Result<WasmApi> {
    let content = fs::read_to_string(path)?;
    let syntax = syn::parse_file(&content)?;

    let mut api = WasmApi::default();

    for item in syntax.items {
        if let Item::Fn(item_fn) = item {
            // Check for #[wasm_bindgen] attr
            let mut is_wasm = false;
            let mut js_name = item_fn.sig.ident.to_string();

            for attr in &item_fn.attrs {
                if attr.path().is_ident("wasm_bindgen") {
                    is_wasm = true;
                    // Try to parse js_name = ...
                    let _ = attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("js_name") {
                            let value = meta.value()?;
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(lit_str),
                                ..
                            }) = value.parse()?
                            {
                                js_name = lit_str.value();
                            }
                        }
                        Ok(())
                    });
                }
            }

            if is_wasm {
                let rust_name = item_fn.sig.ident.to_string();
                let docs = super::rust_options::extract_docs(&item_fn.attrs);

                let mut params = Vec::new();
                for arg in &item_fn.sig.inputs {
                    if let syn::FnArg::Typed(pat_type) = arg {
                        if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                            params.push(ParamDocs {
                                name: pat_ident.ident.to_string(),
                                ty: super::rust_options::type_to_string(&pat_type.ty),
                            });
                        }
                    }
                }

                let returns = match &item_fn.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => Some(super::rust_options::type_to_string(ty)),
                };

                api.functions.push(FunctionDocs {
                    name: js_name,
                    rust_name,
                    docs,
                    params,
                    returns,
                });
            }
        }
    }

    Ok(api)
}
