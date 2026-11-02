# docx-fill

Fills DOCX templates written in docxtemplater's public tag syntax, with `{ }` delimiters. Pure Rust, no dependencies beyond `serde_json`, so it builds for wasm.

Core: value tags, `{#loop}` / `{^inverted}` sections, dash loops (`{-w:tr rows}`), paragraph loops, raw XML (`{@xml}`), line breaks in values, any delimiters (`[[ ]]`, `{{ }}`), and module tags handed to the caller by prefix. Body, headers and footers are filled.

Extensions, each on or off in `FillOptions::extensions`:

- `{:vt#cols}` … `{:vt/cols}` repeat table cells.
- `{:stylecell name}` fills its table cell with `{ "cellBackground": "#rrggbb" }`.
- `{~~html}` prints HTML (p, br, h1-h6, strong/b, em/i, u, ul, ol, li) as Word paragraphs; `{~html}` prints it inline.
- `{%image}` and `{%%image}` draw an image a `FillHooks::image` hook supplies, sized to its table cell or the page, adding its media part, relationship and content type.
- An empty last page left by a loop that printed nothing is dropped.

```rust
let template = docx_fill::PackageTemplate::parse(parts, &docx_fill::FillOptions::default())?;
let filled = template.render(&serde_json::json!({ "name": "Value" }));
```

Tests: `cargo test`.

## Sources

Behaviour is built from the WordprocessingML spec (ECMA-376), docxtemplater's public tag syntax and documentation, and black-box runs of the MIT-licensed docxtemplater core. No code is taken from docxtemplater, and no feature is built from or tuned against its paid modules.

Not affiliated with docxtemplater.
