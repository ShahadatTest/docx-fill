mod common;

use std::borrow::Cow;

use common::{document, fill};
use docx_fill::{FillOptions, PackageTemplate};
use serde_json::json;

const TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"></Types>"#;
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#;

fn package(body: &str, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut parts = vec![
        ("[Content_Types].xml".to_string(), TYPES.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels".to_string(), RELS.as_bytes().to_vec()),
        ("word/document.xml".to_string(), document(body).into_bytes()),
    ];
    parts.extend(extra.iter().map(|(name, xml)| (name.to_string(), xml.as_bytes().to_vec())));
    let template = PackageTemplate::parse(parts, &FillOptions::default()).unwrap();
    template.render(&json!({ "note": "See <annex>" })).into_iter().map(|(name, bytes): (String, Cow<[u8]>)| (name, String::from_utf8(bytes.into_owned()).unwrap())).collect()
}

fn part<'a>(parts: &'a [(String, String)], name: &str) -> &'a str {
    &parts.iter().find(|(each, _)| each == name).unwrap_or_else(|| panic!("{name} is missing")).1
}

#[test]
fn references_the_footnote_in_a_superscript_run_between_runs_formatted_as_the_tags() {
    let body = fill(r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Text{:footnote note} more</w:t></w:r></w:p>"#, json!({ "note": "n" }));

    assert_eq!(
        body,
        r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Text</w:t></w:r><w:r><w:rPr><w:vertAlign w:val="superscript"/></w:rPr><w:footnoteReference w:id="1"/></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve"> more</w:t></w:r></w:p>"#
    );
}

#[test]
fn prints_nothing_for_an_empty_note() {
    let body = fill(r#"<w:p><w:r><w:t>a{:footnote note}</w:t></w:r></w:p>"#, json!({ "note": "" }));

    assert!(!body.contains("footnoteReference"));
}

#[test]
fn creates_the_footnotes_part_its_relationship_and_content_type() {
    let parts = package(r#"<w:p><w:r><w:t>a{:footnote note}</w:t></w:r></w:p>"#, &[]);

    let notes = part(&parts, "word/footnotes.xml");
    assert!(notes.contains(r#"w:type="separator" w:id="-1""#));
    assert!(notes.contains(r#"<w:footnote w:id="1">"#));
    assert!(notes.contains("> See &lt;annex&gt;</w:t>"));
    assert!(part(&parts, "word/_rels/document.xml.rels").contains(r#"Target="footnotes.xml""#));
    assert!(part(&parts, "[Content_Types].xml").contains(r#"PartName="/word/footnotes.xml""#));
}

#[test]
fn numbers_new_footnotes_after_the_existing_ones() {
    let existing = r#"<w:footnotes xmlns:w="x"><w:footnote w:type="separator" w:id="-1"/><w:footnote w:id="0"/><w:footnote w:id="4"><w:p/></w:footnote></w:footnotes>"#;
    let parts = package(r#"<w:p><w:r><w:t>{:footnote note}</w:t></w:r></w:p>"#, &[("word/footnotes.xml", existing)]);

    assert!(part(&parts, "word/document.xml").contains(r#"<w:footnoteReference w:id="5"/>"#));
    assert!(part(&parts, "word/footnotes.xml").contains(r#"<w:footnote w:id="4"><w:p/></w:footnote><w:footnote w:id="5">"#));
    assert!(!part(&parts, "word/_rels/document.xml.rels").contains("footnotes"));
}

#[test]
fn leaves_headers_without_footnotes() {
    let header = r#"<w:hdr xmlns:w="x"><w:p><w:r><w:t>{:footnote note}</w:t></w:r></w:p></w:hdr>"#;
    let parts = package("<w:p/>", &[("word/header1.xml", header)]);

    assert!(!part(&parts, "word/header1.xml").contains("footnote"));
    assert!(!parts.iter().any(|(name, _)| name == "word/footnotes.xml"));
}
