# docx-fill

Fills DOCX templates written in docxtemplater's public tag syntax, with `{ }` delimiters. Pure Rust, no dependencies beyond `serde_json`, so it builds for wasm.

Core: value tags, `{#loop}` / `{^inverted}` sections, dash loops (`{-w:tr rows}`), paragraph loops, raw XML (`{@xml}`), line breaks in values, any delimiters (`[[ ]]`, `{{ }}`), and module tags handed to the caller by prefix. Body, headers and footers are filled.

Extensions, each on or off in `FillOptions::extensions`:

- `{:vt#cols}` … `{:vt/cols}` repeat table cells.
- `{:stylecell name}`, `{:stylepar name}`, `{:stylerow name}` and `{:stylerun name}` style their cell, paragraph, row or run with a value such as `{ "cellBackground": "#rrggbb", "textColor": "#f00", "textAlign": "center", "pStyle": "Heading1", "fontSize": 24, "fontFamily": "Arial", "bold": true, "italic": false, "underline": true, "borders": { "top": { "val": "single", "sz": 4, "color": "#000" } } }`. `fontSize` is in half-points. `{:stylebullets name}` colours a list paragraph's bullet (`bulletColor`), `{:styleshape name}` a shape's fill and outline (`fillColor`, `strokeColor`), and `{:stylestart name}` … `{:/style}` styles the runs between. `{::stylepar name}` and `{::stylerun name}` restyle the definition of their paragraph's or run's style in `word/styles.xml`; `FillOptions::drop_style_paragraphs` drops the `{::stylepar}` paragraphs.
- `{:merge-cells-col 1,2}`, `{:merge-cells-col rtl 1,2}` and `{:merge-cells-row 1,-1}` merge neighbouring cells of their table that print the same text.
- `{?name}` drops its paragraph for an empty value, repeats it per list item, and keeps it otherwise.
- `{r@name}` replaces its run with raw `<w:r>` XML.
- `{:footnote name}` adds a footnote, creating `word/footnotes.xml` when needed.
- `{~~html}` prints HTML (p, br, h1-h6, strong/b, em/i, u, ul, ol, li) as Word paragraphs; `{~html}` prints it inline. With `html_extras`, also links, tables, s/del/ins, sup/sub, mark, code, pre, hr, blockquote, checkboxes, list numbering attributes and inline CSS.
- `{:table name}` builds a table from data; `{:#grid name}` … `{:/grid}` prints its paragraphs once per item in a grid of cells.
- `{%image}` and `{%%image}` draw an image a `FillHooks::image` hook supplies, sized to its table cell or the page, adding its media part, relationship and content type. With `centered_images`, `{%%image}` alone in its paragraph centres it.
- `{:include name}` prints the bodies of subtemplates added with `PackageTemplate::add_include`, as given or filled with the tag's scope. Their styles (the main document's win, or `IncludeStyles::KeepOwn` renames clashes), lists (one the main document has level by level continues there), media, links, footnotes and the parts copied parts relate to, such as a chart's workbook, come along; section properties, comments and endnotes do not. `{:segment name}` … `{:segment/}` defines a segment printed only by `{:includesegment name}` or `{:includesegment* expression}`; `{:inlinesegment}` and `{:includeinlinesegment}` do the same inside a paragraph.
- `{:subsection name}` starts a section with a subtemplate's headers and footers; `{:replacesection name}` gives them to the tag's section; `{:page props}` sets its orientation, size and margins.
- A value `{ "text": …, "comment": … }` (or `"comments": [...]`) prints its text with Word comments on it, with `FillOptions::comments` as the default author.
- An empty last page left by a loop that printed nothing is dropped.

`FillOptions::document` edits the whole package: read-only or password protection (the hash Word checks: legacy key, then SHA-1/256/384/512 with salt and spin count), a text watermark in every header, page margins and the default proofing language.

```rust
let template = docx_fill::PackageTemplate::parse(parts, &docx_fill::FillOptions::default())?;
let filled = template.render(&serde_json::json!({ "name": "Value" }));
```

Tests: `cargo test`.

## Sources

Behaviour is built from the WordprocessingML spec (ECMA-376), docxtemplater's public tag syntax and documentation, and black-box runs of the MIT-licensed docxtemplater core. Extension tags follow the paid modules' public documentation. No code is taken from docxtemplater, and nothing is tuned against the paid modules' output. Password hash test vectors come from black-box runs of Apache POI; for characters over 0x7F the crate follows the spec's unsigned bytes where POI sign-extends them.

There is no link module: docxtemplater's site has none. HTML `<a>` links cover it.

Not affiliated with docxtemplater.
