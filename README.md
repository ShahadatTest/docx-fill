# docx-fill

Fills DOCX templates written in docxtemplater's public tag syntax, with `{ }` delimiters. Pure Rust, no dependencies beyond `serde_json`, so it builds for wasm.

Core: value tags, `{#loop}` / `{^inverted}` sections, dash loops (`{-w:tr rows}`), paragraph loops, raw XML (`{@xml}`), line breaks in values, any delimiters (`[[ ]]`, `{{ }}`), and module tags handed to the caller by prefix. Body, headers and footers are filled.

Extensions, each on or off in `FillOptions::extensions`:

- `{:vt#cols}` … `{:vt/cols}` repeat table cells.
- `{:stylecell name}`, `{:stylepar name}`, `{:stylerow name}` and `{:stylerun name}` style their cell, paragraph, row or run with a value such as `{ "cellBackground": "#rrggbb", "textColor": "#f00", "textAlign": "center", "pStyle": "Heading1", "fontSize": 24, "fontFamily": "Arial", "bold": true, "italic": false, "underline": true, "borders": { "top": { "val": "single", "sz": 4, "color": "#000" } } }`. `fontSize` is in half-points.
- `{:merge-cells-col 1,2}`, `{:merge-cells-col rtl 1,2}` and `{:merge-cells-row 1,-1}` merge neighbouring cells of their table that print the same text.
- `{?name}` drops its paragraph for an empty value, repeats it per list item, and keeps it otherwise.
- `{r@name}` replaces its run with raw `<w:r>` XML.
- `{:footnote name}` adds a footnote, creating `word/footnotes.xml` when needed.
- `{~~html}` prints HTML (p, br, h1-h6, strong/b, em/i, u, ul, ol, li) as Word paragraphs; `{~html}` prints it inline.
- `{%image}` and `{%%image}` draw an image a `FillHooks::image` hook supplies, sized to its table cell or the page, adding its media part, relationship and content type.
- An empty last page left by a loop that printed nothing is dropped.

`FillOptions::document` edits the whole package: read-only protection, page margins and the default proofing language.

```rust
let template = docx_fill::PackageTemplate::parse(parts, &docx_fill::FillOptions::default())?;
let filled = template.render(&serde_json::json!({ "name": "Value" }));
```

Tests: `cargo test`.

## Sources

Behaviour is built from the WordprocessingML spec (ECMA-376), docxtemplater's public tag syntax and documentation, and black-box runs of the MIT-licensed docxtemplater core. Extension tags follow the paid modules' public documentation. No code is taken from docxtemplater, and nothing is tuned against the paid modules' output.

Not affiliated with docxtemplater.
