# docx-fill

Fills DOCX templates written in docxtemplater's public tag syntax, with `{ }` delimiters. Built for Scope's live DOCX preview, where it runs inside the WordCraft wasm.

Supported: value tags, `{#loop}` / `{^inverted}` sections, dash loops (`{-w:tr rows}`), paragraph loops, raw XML (`{@xml}`), line breaks in values, and module tags handed to the caller by prefix. Body, headers and footers are filled.

```rust
let template = docx_fill::PackageTemplate::parse(parts, &docx_fill::FillOptions::default())?;
let filled = template.render(&serde_json::json!({ "name": "Value" }));
```

Tests: `cargo test`.
