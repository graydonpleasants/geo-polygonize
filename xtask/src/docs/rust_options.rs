use super::{EnumDocs, FieldDocs, OptionsSchema, StructDocs, VariantDocs};
use anyhow::Result;
use std::fs;
use syn::{Fields, Item, Type};

pub fn parse_options(path: &str) -> Result<OptionsSchema> {
    let content = fs::read_to_string(path)?;
    let syntax = syn::parse_file(&content)?;

    let mut schema = OptionsSchema::default();

    for item in syntax.items {
        match item {
            Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let docs = extract_docs(&item_struct.attrs);

                let mut fields = Vec::new();
                if let Fields::Named(named_fields) = item_struct.fields {
                    for field in named_fields.named {
                        let field_name = field.ident.unwrap().to_string();
                        let field_docs = extract_docs(&field.attrs);
                        let is_optional = has_ts_optional_attr(&field.attrs);
                        let ty = type_to_string(&field.ty);

                        fields.push(FieldDocs {
                            name: field_name,
                            docs: field_docs,
                            ty,
                            is_optional,
                        });
                    }
                }

                schema
                    .structs
                    .insert(name.clone(), StructDocs { name, docs, fields });
            }
            Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                let docs = extract_docs(&item_enum.attrs);

                let mut variants = Vec::new();
                for variant in item_enum.variants {
                    let variant_name = variant.ident.to_string();
                    let variant_docs = extract_docs(&variant.attrs);

                    variants.push(VariantDocs {
                        name: variant_name,
                        docs: variant_docs,
                    });
                }

                schema.enums.insert(
                    name.clone(),
                    EnumDocs {
                        name,
                        docs,
                        variants,
                    },
                );
            }
            _ => {}
        }
    }

    Ok(schema)
}

pub fn extract_docs(attrs: &[syn::Attribute]) -> String {
    let mut docs = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                }) = &nv.value
                {
                    let text = lit_str.value();
                    let trimmed = text.strip_prefix(' ').unwrap_or(&text);
                    docs.push(trimmed.to_string());
                }
            }
        }
    }
    docs.join("\n")
}

fn has_ts_optional_attr(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("ts") {
            let mut is_opt = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("optional") {
                    is_opt = true;
                }
                Ok(())
            });
            if is_opt {
                return true;
            }
        }
    }
    false
}

pub fn type_to_string(ty: &Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}
