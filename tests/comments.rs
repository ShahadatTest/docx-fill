mod common;

use common::{document, elements};
use docx_fill::{CommentDefaults, FillOptions, PackageTemplate};
use serde_json::{Value, json};

fn fill(body: &str, data: Value, extra: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut parts = vec![("word/document.xml".to_string(), document(body).into_bytes())];
    parts.extend(extra.iter().map(|(name, xml)| (name.to_string(), xml.as_bytes().to_vec())));
    let options = FillOptions { comments: CommentDefaults { author: "Scope".to_string(), initials: Some("S".to_string()), date: None }, ..FillOptions::default() };
    let template = PackageTemplate::parse(parts, &options).unwrap();
    template.render(&data).into_iter().map(|(name, bytes)| (name, String::from_utf8(bytes.into_owned()).unwrap())).collect()
}

fn part(parts: &[(String, String)], name: &str) -> String {
    parts.iter().find(|(each, _)| each == name).unwrap_or_else(|| panic!("no {name}")).1.clone()
}

const PARTS: [(&str, &str); 2] = [("[Content_Types].xml", r#"<Types xmlns="t"></Types>"#), ("word/_rels/document.xml.rels", r#"<Relationships xmlns="r"></Relationships>"#)];

#[test]
fn marks_the_values_text_with_its_comment() {
    let parts = fill(r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Hi {user}!</w:t></w:r></w:p>"#, json!({ "user": { "text": "John", "comment": "Check <spelling>" } }), &PARTS);

    let xml = part(&parts, "word/document.xml");
    assert!(xml.contains(
        r#"<w:t xml:space="preserve">Hi </w:t></w:r><w:commentRangeStart w:id="0"/><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">John</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="0"/></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">!</w:t></w:r>"#
    ));
    let comments = part(&parts, "word/comments.xml");
    assert!(comments.contains(r#"<w:comment w:id="0" w:author="Scope" w:initials="S"><w:p><w:pPr><w:pStyle w:val="CommentText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:annotationRef/></w:r><w:r><w:t xml:space="preserve">Check &lt;spelling&gt;</w:t></w:r></w:p></w:comment>"#));
    assert!(part(&parts, "word/_rels/document.xml.rels").contains(r#"Target="comments.xml""#));
    assert!(part(&parts, "[Content_Types].xml").contains(r#"PartName="/word/comments.xml""#));
}

#[test]
fn adds_each_comment_of_a_list_after_existing_ones() {
    let existing = r#"<w:comments xmlns:w="w"><w:comment w:id="4" w:author="A"><w:p/></w:comment></w:comments>"#;
    let value = json!({ "user": { "text": "John", "author": "Mary", "comments": [{ "comment": "One", "date": "2026-01-01T00:00:00Z" }, { "comment": "Two", "author": "Bo" }] } });
    let parts = fill(r#"<w:p><w:r><w:t>{user}</w:t></w:r></w:p>"#, value, &[("word/comments.xml", existing)]);

    let comments = part(&parts, "word/comments.xml");
    assert!(comments.contains(r#"<w:comment w:id="5" w:author="Mary" w:initials="S" w:date="2026-01-01T00:00:00Z">"#));
    assert!(comments.contains(r#"<w:comment w:id="6" w:author="Bo" w:initials="S">"#));
    let xml = part(&parts, "word/document.xml");
    assert!(xml.contains(r#"<w:commentRangeStart w:id="5"/><w:commentRangeStart w:id="6"/><w:r>"#));
}

#[test]
fn prints_plain_values_and_other_objects_as_before() {
    let parts = fill(r#"<w:p><w:r><w:t>{a} {b}</w:t></w:r></w:p>"#, json!({ "a": "x", "b": { "text": "y" } }), &PARTS);

    assert!(!parts.iter().any(|(name, _)| name == "word/comments.xml"));
    assert_eq!(elements(&part(&parts, "word/document.xml"), "w:r").len(), 1);
}
