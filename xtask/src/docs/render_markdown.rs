use super::DocsMetadata;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn render(metadata: &DocsMetadata) -> Result<()> {
    let out_dir = Path::new("docs/reference");
    fs::create_dir_all(out_dir)?;

    render_options(metadata, out_dir)?;
    render_wasm_api(metadata, out_dir)?;

    Ok(())
}

fn render_options(metadata: &DocsMetadata, out_dir: &Path) -> Result<()> {
    let mut index = String::new();
    index.push_str("# Configuration Options\n\n");
    index.push_str("The following reference docs are auto-generated from the Rust source.\n\n");

    // Sort structs
    let structs: Vec<_> = metadata.options.structs.values().collect();

    for s in structs {
        index.push_str(&format!("* [`{}`](./options/{}.md)\n", s.name, s.name));

        let mut page = String::new();
        page.push_str(&format!("# {}\n\n", s.name));
        if !s.docs.is_empty() {
            page.push_str(&format!("{}\n\n", s.docs));
        }

        page.push_str("## Fields\n\n");
        for f in &s.fields {
            page.push_str(&format!("### `{}`\n\n", f.name));
            page.push_str(&format!("**Type:** `{}`", f.ty));
            if f.is_optional {
                page.push_str(" (Optional)");
            }
            page.push_str("\n\n");

            if !f.docs.is_empty() {
                page.push_str(&format!("{}\n\n", f.docs));
            }
        }

        let dir = out_dir.join("options");
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(format!("{}.md", s.name)), page)?;
    }

    // Sort enums
    let enums: Vec<_> = metadata.options.enums.values().collect();

    index.push_str("\n## Enums\n\n");
    for e in enums {
        index.push_str(&format!("* [`{}`](./options/{}.md)\n", e.name, e.name));

        let mut page = String::new();
        page.push_str(&format!("# {}\n\n", e.name));
        if !e.docs.is_empty() {
            page.push_str(&format!("{}\n\n", e.docs));
        }

        page.push_str("## Variants\n\n");
        for v in &e.variants {
            page.push_str(&format!("### `{}`\n\n", v.name));
            if !v.docs.is_empty() {
                page.push_str(&format!("{}\n\n", v.docs));
            }
        }

        let dir = out_dir.join("options");
        fs::create_dir_all(&dir)?;
        fs::write(dir.join(format!("{}.md", e.name)), page)?;
    }

    fs::write(out_dir.join("options.md"), index)?;
    Ok(())
}

fn render_wasm_api(metadata: &DocsMetadata, out_dir: &Path) -> Result<()> {
    let mut page = String::new();
    page.push_str(
        "# WASM API Reference

",
    );
    page.push_str(
        "Auto-generated reference for the `geo-polygonize` WebAssembly bindings.

",
    );

    for f in &metadata.wasm.functions {
        page.push_str(&format!(
            "## `{}`

",
            f.name
        ));
        if !f.docs.is_empty() {
            page.push_str(&format!(
                "{}

",
                f.docs
            ));
        }

        page.push_str(
            "### Signature

```typescript
",
        );
        page.push_str(&format!("function {}(", f.name));

        let params: Vec<_> = f
            .params
            .iter()
            .map(|p| format!("{}: {}", p.name, map_rust_ty_to_ts(&p.ty)))
            .collect();
        page.push_str(&params.join(", "));

        page.push(')');
        if let Some(ret) = &f.returns {
            page.push_str(&format!(": {}", map_rust_ty_to_ts(ret)));
        } else {
            page.push_str(": void");
        }
        page.push_str(
            "
```

",
        );
    }

    fs::write(out_dir.join("wasm-api.md"), page)?;
    Ok(())
}

fn map_rust_ty_to_ts(ty: &str) -> String {
    if ty.contains("JsValue") {
        "any".to_string()
    } else if ty.contains("String") || ty.contains("&str") {
        "string".to_string()
    } else if ty.contains("bool") {
        "boolean".to_string()
    } else if ty.contains("f64")
        || ty.contains("u32")
        || ty.contains("usize")
        || ty.contains("i32")
        || ty.contains("u8")
    {
        "number".to_string()
    } else if ty.contains("Vec<u8>") || ty.contains("&[u8]") {
        "Uint8Array".to_string()
    } else {
        ty.to_string()
    }
}
