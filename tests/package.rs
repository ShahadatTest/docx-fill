//! Filling a whole package: the body, headers and footers.

mod common;

use std::borrow::Cow;

use common::{document, paragraph, text};
use docx_fill::{FillOptions, PackageTemplate, is_template_part};
use serde_json::json;

fn header(body: &str) -> Vec<u8> {
    format!("<w:hdr {}>{body}</w:hdr>", common::NAMESPACES).into_bytes()
}

#[test]
fn fills_the_body_headers_and_footers_and_keeps_other_parts() {
    let styles = b"<w:styles>{v}</w:styles>".to_vec();
    let parts = vec![
        ("[Content_Types].xml".to_string(), b"<Types/>".to_vec()),
        ("word/document.xml".to_string(), document(&paragraph(&["body {v}"])).into_bytes()),
        ("word/header1.xml".to_string(), header(&paragraph(&["header {v}"]))),
        ("word/footer2.xml".to_string(), header(&paragraph(&["footer {v}"]))),
        ("word/styles.xml".to_string(), styles.clone()),
    ];
    let template = PackageTemplate::parse(parts, &FillOptions::default()).unwrap();

    let filled = template.render(&json!({ "v": "V" }));

    let names: Vec<&str> = filled.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["[Content_Types].xml", "word/document.xml", "word/header1.xml", "word/footer2.xml", "word/styles.xml"]);
    let texts: Vec<String> = filled[1..4].iter().map(|(_, bytes)| text(std::str::from_utf8(bytes).unwrap())).collect();
    assert_eq!(texts, ["body V", "header V", "footer V"]);
    assert!(matches!(&filled[4].1, Cow::Borrowed(bytes) if *bytes == styles.as_slice()));
}

#[test]
fn names_the_part_a_template_error_is_in() {
    let parts = vec![("word/footer1.xml".to_string(), header(&paragraph(&["{#open}"])))];

    let error = PackageTemplate::parse(parts, &FillOptions::default()).unwrap_err();

    assert_eq!(error.to_string(), r#"word/footer1.xml: The loop with tag "open" is unclosed"#);
}

#[test]
fn recognises_template_parts() {
    for name in ["word/document.xml", "word/header.xml", "word/header12.xml", "word/footer3.xml"] {
        assert!(is_template_part(name), "{name}");
    }
    for name in ["word/styles.xml", "word/headerx.xml", "word/glossary/document.xml", "customXml/item1.xml"] {
        assert!(!is_template_part(name), "{name}");
    }
}
