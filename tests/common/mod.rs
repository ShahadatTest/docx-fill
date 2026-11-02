#![allow(dead_code)]

use docx_fill::{FillOptions, fill_part};
use serde_json::Value;

pub const NAMESPACES: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

pub fn document(body: &str) -> String {
    format!("<w:document {NAMESPACES}><w:body>{body}</w:body></w:document>")
}

/// The filled body of a document holding `body`, with the preview's options.
pub fn fill(body: &str, data: Value) -> String {
    let xml = fill_part(&document(body), &data, &FillOptions::default()).expect("template parses");
    let start = xml.find("<w:body>").unwrap() + "<w:body>".len();
    xml[start..xml.rfind("</w:body>").unwrap()].to_string()
}

pub fn run(text: &str) -> String {
    format!("<w:r><w:t>{text}</w:t></w:r>")
}

/// A paragraph with one run per text.
pub fn paragraph(texts: &[&str]) -> String {
    format!("<w:p>{}</w:p>", texts.iter().map(|text| run(text)).collect::<String>())
}

/// A table cell holding one paragraph with one run per text.
pub fn cell(texts: &[&str]) -> String {
    format!("<w:tc>{}</w:tc>", paragraph(texts))
}

pub fn row(cells: &[String]) -> String {
    format!("<w:tr>{}</w:tr>", cells.concat())
}

pub fn table(rows: &[String]) -> String {
    format!("<w:tbl>{}</w:tbl>", rows.concat())
}

/// Each `<name>…</name>` in `xml`, not nested, in order.
pub fn elements<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{name}>"), format!("</{name}>"));
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let Some(length) = rest[start..].find(&close) else { break };
        let end = start + length + close.len();
        found.push(&rest[start..end]);
        rest = &rest[end..];
    }
    found
}

/// The joined `<w:t>` text of `xml`, as written.
pub fn text(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<w:t") {
        rest = &rest[start + 4..];
        if !(rest.starts_with('>') || rest.starts_with(' ')) {
            continue;
        }
        let open_end = rest.find('>').unwrap();
        if rest[..open_end].ends_with('/') {
            continue;
        }
        rest = &rest[open_end + 1..];
        let end = rest.find("</w:t>").unwrap();
        out.push_str(&rest[..end]);
        rest = &rest[end..];
    }
    out
}

/// Each paragraph's text.
pub fn paragraph_texts(xml: &str) -> Vec<String> {
    elements(xml, "w:p").into_iter().map(text).collect()
}

/// Each row's cell texts.
pub fn read_rows(xml: &str) -> Vec<Vec<String>> {
    elements(xml, "w:tr").into_iter().map(|each| elements(each, "w:tc").into_iter().map(text).collect()).collect()
}
