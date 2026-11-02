# docx-fill

Fills DOCX templates written in docxtemplater's public tag syntax, with `{ }` delimiters. Pure Rust, no dependencies beyond `serde_json`, so it builds for wasm.

Supported: value tags, `{#loop}` / `{^inverted}` sections, dash loops (`{-w:tr rows}`), paragraph loops, raw XML (`{@xml}`), line breaks in values, and module tags handed to the caller by prefix. Body, headers and footers are filled.

```rust
let template = docx_fill::PackageTemplate::parse(parts, &docx_fill::FillOptions::default())?;
let filled = template.render(&serde_json::json!({ "name": "Value" }));
```

Tests: `cargo test`.

## Sources

Behaviour is built from the WordprocessingML spec (ECMA-376), docxtemplater's public tag syntax and documentation, and black-box runs of the MIT-licensed docxtemplater core. No code is taken from docxtemplater, and no feature is built from or tuned against its paid modules.

Not affiliated with docxtemplater.
