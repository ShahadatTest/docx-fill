mod common;

use common::{document, paragraph_texts};
use docx_fill::{FillOptions, IncludeMode, IncludeStyles, PackageTemplate, Subtemplate};
use serde_json::{Value, json};

const TYPES: &str = r#"<Types xmlns="t"><Default Extension="xml" ContentType="application/xml"/></Types>"#;
const MAIN_STYLES: &str = r#"<w:styles xmlns:w="w"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:b/></w:rPr></w:style></w:styles>"#;
const SUB_STYLES: &str = r#"<w:styles xmlns:w="w"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:i/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Base"/><w:pPr><w:numPr><w:numId w:val="3"/></w:numPr></w:pPr></w:style><w:style w:type="paragraph" w:styleId="Base"><w:name w:val="Base"/></w:style><w:style w:type="paragraph" w:styleId="Unused"><w:name w:val="Unused"/></w:style></w:styles>"#;
const SUB_NUMBERING: &str = r#"<w:numbering xmlns:w="w"><w:abstractNum w:abstractNumId="0"><w:nsid w:val="1"/><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:num w:numId="3"><w:abstractNumId w:val="0"/></w:num><w:num w:numId="4"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#;
const SUB_RELS: &str = r#"<Relationships xmlns="r"><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/><Relationship Id="rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/?a=1&amp;b=2" TargetMode="External"/><Relationship Id="rId9" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#;
const SUB_FOOTNOTES: &str = r#"<w:footnotes xmlns:w="w"><w:footnote w:type="separator" w:id="-1"><w:p/></w:footnote><w:footnote w:id="1"><w:p><w:r><w:t>Sub note</w:t></w:r></w:p></w:footnote></w:footnotes>"#;

fn sub_body() -> String {
    [
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Hello {name}</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Quote"/></w:pPr><w:r><w:t>Quoted</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="4"/></w:numPr></w:pPr><w:r><w:t>Item</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:drawing><wp:inline><wp:docPr id="1" name="Picture"/><a:blip r:embed="rId5"/></wp:inline></w:drawing></w:r><w:hyperlink r:id="rId6"><w:r><w:t>link</w:t></w:r></w:hyperlink></w:p>"#,
        r#"<w:p><w:commentRangeStart w:id="0"/><w:r><w:t>Noted</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:footnoteReference w:id="1"/></w:r></w:p>"#,
        r#"<w:sectPr><w:headerReference w:type="default" r:id="rId9"/></w:sectPr>"#,
    ]
    .concat()
}

fn sub_parts() -> Vec<(String, Vec<u8>)> {
    let document = format!(r#"<w:document xmlns:w="w" xmlns:r="r" xmlns:wp14="x14" xmlns:mc="mc" mc:Ignorable="wp14"><w:body>{}</w:body></w:document>"#, sub_body());
    [
        ("word/document.xml", document.into_bytes()),
        ("word/styles.xml", SUB_STYLES.as_bytes().to_vec()),
        ("word/numbering.xml", SUB_NUMBERING.as_bytes().to_vec()),
        ("word/_rels/document.xml.rels", SUB_RELS.as_bytes().to_vec()),
        ("word/footnotes.xml", SUB_FOOTNOTES.as_bytes().to_vec()),
        ("word/media/image1.png", b"png bytes".to_vec()),
        ("[Content_Types].xml", r#"<Types xmlns="t"><Default Extension="png" ContentType="image/png"/></Types>"#.as_bytes().to_vec()),
    ]
    .into_iter()
    .map(|(name, bytes)| (name.to_string(), bytes))
    .collect()
}

fn main_parts(body: &str) -> Vec<(String, Vec<u8>)> {
    let document = format!(r#"<w:document xmlns:w="w" xmlns:r="r" xmlns:mc="mc" mc:Ignorable="w14"><w:body>{body}<w:sectPr/></w:body></w:document>"#);
    [("word/document.xml", document), ("word/styles.xml", MAIN_STYLES.to_string()), ("[Content_Types].xml", TYPES.to_string()), ("word/_rels/document.xml.rels", r#"<Relationships xmlns="r"></Relationships>"#.to_string())]
        .into_iter()
        .map(|(name, xml)| (name.to_string(), xml.into_bytes()))
        .collect()
}

fn fill(body: &str, data: Value, mode: IncludeMode, styles: IncludeStyles) -> Vec<(String, String)> {
    let options = FillOptions::default();
    let mut template = PackageTemplate::parse(main_parts(body), &options).unwrap();
    template.add_include("part", Subtemplate::parse(sub_parts(), &options, mode, styles).unwrap());
    template.render(&data).into_iter().map(|(name, bytes)| (name, String::from_utf8_lossy(&bytes).into_owned())).collect()
}

fn part(parts: &[(String, String)], name: &str) -> String {
    parts.iter().find(|(each, _)| each == name).unwrap_or_else(|| panic!("no {name}")).1.clone()
}

#[test]
fn prints_the_body_in_place_of_the_tags_paragraph() {
    let parts = fill(r#"<w:p><w:r><w:t>Before</w:t></w:r></w:p><w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p><w:p><w:r><w:t>After</w:t></w:r></w:p>"#, json!({ "doc": "part", "name": "Ada" }), IncludeMode::AsIs, IncludeStyles::UseMain);

    let xml = part(&parts, "word/document.xml");
    assert_eq!(paragraph_texts(&xml), ["Before", "Hello {name}", "Quoted", "Item", "link", "Noted", "After"]);
    assert!(!xml.contains("headerReference") && !xml.contains("commentRange"));
    assert_eq!(xml.matches("<w:sectPr").count(), 1);
}

#[test]
fn fills_a_templated_include_with_the_scope_at_its_tag() {
    let body = r#"<w:p><w:r><w:t>{#people}</w:t></w:r></w:p><w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p><w:p><w:r><w:t>{/people}</w:t></w:r></w:p>"#;
    let parts = fill(body, json!({ "doc": "part", "people": [{ "name": "Ada" }, { "name": "Bo" }] }), IncludeMode::Templated, IncludeStyles::UseMain);

    let texts: Vec<String> = paragraph_texts(&part(&parts, "word/document.xml")).into_iter().filter(|text| text.starts_with("Hello")).collect();
    assert_eq!(texts, ["Hello Ada", "Hello Bo"]);
}

#[test]
fn prints_each_name_of_a_list_and_nothing_for_unknown_or_missing() {
    let body = r#"<w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p>"#;
    for (data, count) in [(json!({ "doc": ["part", "part"] }), 2), (json!({ "doc": "other" }), 0), (json!({}), 0), (json!({ "doc": null }), 0)] {
        let xml = part(&fill(body, data, IncludeMode::AsIs, IncludeStyles::UseMain), "word/document.xml");
        assert_eq!(xml.matches("Quoted").count(), count);
        assert!(!xml.contains(['\u{E005}', '\u{E006}']));
    }
}

#[test]
fn copies_missing_styles_and_keeps_the_main_definitions() {
    let parts = fill(r#"<w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p>"#, json!({ "doc": "part" }), IncludeMode::AsIs, IncludeStyles::UseMain);

    let styles = part(&parts, "word/styles.xml");
    assert_eq!(styles.matches("w:styleId=\"Heading1\"").count(), 1);
    assert!(styles.contains(r#"w:styleId="Heading1"><w:name w:val="heading 1"/><w:rPr><w:b/>"#));
    assert!(styles.contains(r#"w:styleId="Quote""#) && styles.contains(r#"w:styleId="Base""#));
    assert!(!styles.contains("Unused"));
    assert_eq!(styles.matches("w:default=").count(), 1);
}

#[test]
fn renames_clashing_styles_when_keeping_its_own() {
    let parts = fill(r#"<w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p>"#, json!({ "doc": ["part", "part"] }), IncludeMode::AsIs, IncludeStyles::KeepOwn);

    let styles = part(&parts, "word/styles.xml");
    assert!(styles.contains(r#"<w:style w:type="paragraph" w:styleId="Heading1part"><w:name w:val="heading 1 (part)"/><w:rPr><w:i/>"#));
    assert_eq!(styles.matches("w:styleId=\"Heading1part\"").count(), 1);
    let xml = part(&parts, "word/document.xml");
    assert_eq!(xml.matches(r#"<w:pStyle w:val="Heading1part"/>"#).count(), 2);
}

#[test]
fn copies_lists_with_new_ids_and_creates_the_numbering_part() {
    let parts = fill(r#"<w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p>"#, json!({ "doc": "part" }), IncludeMode::AsIs, IncludeStyles::UseMain);

    let numbering = part(&parts, "word/numbering.xml");
    assert_eq!(numbering.matches("<w:abstractNum ").count(), 1);
    assert!(!numbering.contains("w:nsid"));
    assert!(numbering.contains(r#"<w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num><w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>"#));
    assert!(part(&parts, "word/document.xml").contains(r#"<w:numId w:val="2"/>"#));
    assert!(part(&parts, "word/styles.xml").contains(r#"<w:numId w:val="1"/>"#));
    assert!(part(&parts, "word/_rels/document.xml.rels").contains("Target=\"numbering.xml\""));
    assert!(part(&parts, "[Content_Types].xml").contains("PartName=\"/word/numbering.xml\""));
}

#[test]
fn copies_media_and_links_under_new_relationships() {
    let parts = fill(r#"<w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p>"#, json!({ "doc": "part" }), IncludeMode::AsIs, IncludeStyles::UseMain);

    let rels = part(&parts, "word/_rels/document.xml.rels");
    assert!(rels.contains(r#"Id="rIdDocxFillInclude1_rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/docx_fill_include1_image1.png""#));
    assert!(rels.contains(r#"Id="rIdDocxFillInclude1_rId6" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com/?a=1&amp;b=2" TargetMode="External""#));
    assert!(!rels.contains("header1.xml"));
    assert_eq!(part(&parts, "word/media/docx_fill_include1_image1.png"), "png bytes");
    assert!(part(&parts, "[Content_Types].xml").contains(r#"<Default Extension="png" ContentType="image/png"/>"#));
    let xml = part(&parts, "word/document.xml");
    assert!(xml.contains(r#"r:embed="rIdDocxFillInclude1_rId5""#) && xml.contains(r#"r:id="rIdDocxFillInclude1_rId6""#));
}

#[test]
fn copies_footnotes_and_namespaces() {
    let parts = fill(r#"<w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p>"#, json!({ "doc": "part" }), IncludeMode::AsIs, IncludeStyles::UseMain);

    let notes = part(&parts, "word/footnotes.xml");
    assert!(notes.contains(r#"<w:footnote w:id="1"><w:p><w:r><w:t>Sub note</w:t></w:r></w:p></w:footnote>"#));
    let xml = part(&parts, "word/document.xml");
    assert!(xml.contains(r#"<w:footnoteReference w:id="1"/>"#));
    assert!(xml.starts_with(r#"<w:document xmlns:w="w" xmlns:r="r" xmlns:mc="mc" mc:Ignorable="w14 wp14" xmlns:wp14="x14">"#));
}

#[test]
fn renumbers_drawings_after_the_main_documents() {
    let main_drawing = r#"<w:p><w:r><w:drawing><wp:inline><wp:docPr id="7" name="Main"/></wp:inline></w:drawing></w:r></w:p>"#;
    let parts = fill(&format!("{main_drawing}<w:p><w:r><w:t>{{:include doc}}</w:t></w:r></w:p>"), json!({ "doc": ["part", "part"] }), IncludeMode::AsIs, IncludeStyles::UseMain);

    let xml = part(&parts, "word/document.xml");
    assert!(xml.contains(r#"<wp:docPr id="8" name="Picture"/>"#) && xml.contains(r#"<wp:docPr id="9" name="Picture"/>"#));
}

#[test]
fn requires_the_include_tag_alone_in_its_paragraph() {
    let error = PackageTemplate::parse(main_parts(r#"<w:p><w:r><w:t>x {:include doc}</w:t></w:r></w:p>"#), &FillOptions::default()).unwrap_err();

    assert!(error.to_string().contains("Segment tag should be the only text in a paragraph"));
}

#[test]
fn rejects_a_subtemplate_without_a_body() {
    let error = Subtemplate::parse([("word/styles.xml".to_string(), Vec::new())], &FillOptions::default(), IncludeMode::AsIs, IncludeStyles::UseMain).unwrap_err();

    assert!(error.to_string().contains("The subtemplate is not valid"));
}

#[test]
fn a_single_part_fill_prints_no_include_marks() {
    let xml = docx_fill::fill_part(&document(r#"<w:p><w:r><w:t>{:include doc}</w:t></w:r></w:p>"#), &json!({ "doc": "part" }), &FillOptions::default()).unwrap();

    assert!(!xml.contains(['\u{E005}', '\u{E006}']) && !xml.contains("<w:p>"));
}
