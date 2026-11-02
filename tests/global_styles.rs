mod common;

use common::{document, paragraph_texts};
use docx_fill::{FillOptions, PackageTemplate};
use serde_json::{Value, json};

const STYLES: &str = r#"<w:styles xmlns:w="x"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style><w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/></w:style></w:styles>"#;

fn fill_parts(body: &str, data: Value, options: FillOptions) -> (String, String) {
    let parts = [("word/document.xml".to_string(), document(body).into_bytes()), ("word/styles.xml".to_string(), STYLES.as_bytes().to_vec())];
    let template = PackageTemplate::parse(parts, &options).unwrap();
    let out: Vec<(String, String)> = template.render(&data).into_iter().map(|(name, bytes)| (name, String::from_utf8(bytes.into_owned()).unwrap())).collect();
    let read = |name: &str| out.iter().find(|(each, _)| each == name).unwrap().1.clone();
    (read("word/document.xml"), read("word/styles.xml"))
}

#[test]
fn restyles_the_paragraph_style_of_the_tags_paragraph() {
    let body = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>{::stylepar s}Title</w:t></w:r></w:p>"#;
    let (xml, styles) = fill_parts(body, json!({ "s": { "textColor": "#ff0000", "textAlign": "center" } }), FillOptions::default());

    assert!(styles.contains(r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:b/><w:color w:val="FF0000"/><w:sz w:val="32"/></w:rPr></w:style>"#));
    assert!(styles.contains(r#"<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>"#));
    assert_eq!(paragraph_texts(&xml), ["Title"]);
}

#[test]
fn restyles_the_default_paragraph_style_without_a_style_id() {
    let (_, styles) = fill_parts(r#"<w:p><w:r><w:t>{::stylepar s}x</w:t></w:r></w:p>"#, json!({ "s": { "italic": true } }), FillOptions::default());

    assert!(styles.contains(r#"w:styleId="Normal"><w:name w:val="Normal"/><w:rPr><w:i/><w:iCs/></w:rPr></w:style>"#));
}

#[test]
fn restyles_the_character_style_of_the_tags_run() {
    let body = r#"<w:p><w:r><w:rPr><w:rStyle w:val="Strong"/></w:rPr><w:t>{::stylerun s}x</w:t></w:r></w:p>"#;
    let (_, styles) = fill_parts(body, json!({ "s": { "underline": true } }), FillOptions::default());

    assert!(styles.contains(r#"w:styleId="Strong"><w:name w:val="Strong"/><w:rPr><w:u w:val="single"/></w:rPr></w:style>"#));
}

#[test]
fn drops_the_tags_paragraph_when_asked() {
    let body = r#"<w:p><w:r><w:t>{::stylepar s}</w:t></w:r></w:p><w:p><w:r><w:t>kept</w:t></w:r></w:p>"#;
    let options = FillOptions { drop_style_paragraphs: true, ..FillOptions::default() };
    let (xml, _) = fill_parts(body, json!({ "s": { "bold": true } }), options);

    assert_eq!(paragraph_texts(&xml), ["kept"]);
}
