mod common;

use docx_fill::{FillOptions, IncludeMode, IncludeStyles, PackageTemplate, Subtemplate};
use serde_json::{Value, json};

const RELS: &str = r#"<Relationships xmlns="r"><Relationship Id="rId1" Type="header" Target="header1.xml"/><Relationship Id="rId2" Type="footer" Target="footer1.xml"/></Relationships>"#;

fn main_parts(body: &str) -> Vec<(String, Vec<u8>)> {
    let document = format!(r#"<w:document xmlns:w="w" xmlns:r="r"><w:body>{body}</w:body></w:document>"#);
    [
        ("word/document.xml", document.as_str()),
        ("word/_rels/document.xml.rels", RELS),
        ("word/header1.xml", r#"<w:hdr xmlns:w="w"><w:p><w:r><w:t>Main header</w:t></w:r></w:p></w:hdr>"#),
        ("word/styles.xml", r#"<w:styles xmlns:w="w"></w:styles>"#),
        ("[Content_Types].xml", r#"<Types xmlns="t"></Types>"#),
    ]
    .into_iter()
    .map(|(name, xml)| (name.to_string(), xml.as_bytes().to_vec()))
    .collect()
}

fn sub_parts() -> Vec<(String, Vec<u8>)> {
    [
        ("word/document.xml", r#"<w:document xmlns:w="w" xmlns:r="r"><w:body><w:p/><w:sectPr><w:headerReference w:type="first" r:id="rId8"/><w:footerReference w:type="default" r:id="rId9"/><w:titlePg/></w:sectPr></w:body></w:document>"#),
        ("word/_rels/document.xml.rels", r#"<Relationships xmlns="r"><Relationship Id="rId8" Type="header" Target="header3.xml"/><Relationship Id="rId9" Type="footer" Target="footer2.xml"/></Relationships>"#),
        ("word/header3.xml", r#"<w:hdr xmlns:w="w"><w:p><w:pPr><w:pStyle w:val="SubHeader"/></w:pPr><w:r><w:t>Sub {title}</w:t></w:r></w:p></w:hdr>"#),
        ("word/footer2.xml", r#"<w:ftr xmlns:w="w"><w:p><w:r><w:t>Sub footer</w:t></w:r></w:p></w:ftr>"#),
        ("word/styles.xml", r#"<w:styles xmlns:w="w"><w:style w:type="paragraph" w:styleId="SubHeader"><w:name w:val="Sub header"/></w:style></w:styles>"#),
        ("[Content_Types].xml", r#"<Types xmlns="t"><Override PartName="/word/header3.xml" ContentType="header+xml"/><Override PartName="/word/footer2.xml" ContentType="footer+xml"/></Types>"#),
    ]
    .into_iter()
    .map(|(name, xml)| (name.to_string(), xml.as_bytes().to_vec()))
    .collect()
}

fn fill(body: &str, data: Value) -> Vec<(String, String)> {
    let options = FillOptions::default();
    let mut template = PackageTemplate::parse(main_parts(body), &options).unwrap();
    template.add_include("sub", Subtemplate::parse(sub_parts(), &options, IncludeMode::Templated, IncludeStyles::UseMain).unwrap());
    template.render(&data).into_iter().map(|(name, bytes)| (name, String::from_utf8(bytes.into_owned()).unwrap())).collect()
}

fn part(parts: &[(String, String)], name: &str) -> String {
    parts.iter().find(|(each, _)| each == name).unwrap_or_else(|| panic!("no {name}")).1.clone()
}

const MAIN_SECTION: &str = r#"<w:sectPr><w:headerReference w:type="default" r:id="rId1"/><w:footerReference w:type="default" r:id="rId2"/><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#;

fn body(tag: &str) -> String {
    format!(r#"<w:p><w:r><w:t>Before</w:t></w:r></w:p><w:p><w:r><w:t>{tag}</w:t></w:r></w:p><w:p><w:r><w:t>After</w:t></w:r></w:p>{MAIN_SECTION}"#)
}

#[test]
fn starts_a_section_with_the_subtemplates_headers_and_footers() {
    let parts = fill(&body("{:subsection doc}"), json!({ "doc": "sub", "title": "Report" }));

    let xml = part(&parts, "word/document.xml");
    let expected_break = format!(r#"<w:p><w:r><w:t>Before</w:t></w:r></w:p><w:p><w:pPr>{MAIN_SECTION}</w:pPr></w:p><w:p><w:r><w:t>After</w:t></w:r></w:p>"#);
    assert!(xml.contains(&expected_break), "{xml}");
    assert!(xml.contains(r#"<w:sectPr><w:headerReference w:type="first" r:id="rIdDocxFillInclude1_rId8"/><w:footerReference w:type="default" r:id="rIdDocxFillInclude1_rId9"/><w:pgSz w:w="11906" w:h="16838"/><w:titlePg/></w:sectPr></w:body>"#));
    assert!(part(&parts, "word/docx_fill_include1_header3.xml").contains("Sub Report"));
    assert!(part(&parts, "word/_rels/document.xml.rels").contains(r#"Id="rIdDocxFillInclude1_rId9" Type="footer" Target="docx_fill_include1_footer2.xml""#));
    assert!(part(&parts, "[Content_Types].xml").contains(r#"PartName="/word/docx_fill_include1_header3.xml""#));
    assert!(part(&parts, "word/styles.xml").contains(r#"w:styleId="SubHeader""#));
}

#[test]
fn replaces_the_sections_headers_and_keeps_them_for_the_next_section() {
    let next = r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="1"/></w:sectPr></w:pPr></w:p>"#;
    let tagged = format!(r#"<w:p><w:r><w:t>{{:replacesection doc}}</w:t></w:r></w:p><w:p><w:pPr>{MAIN_SECTION}</w:pPr></w:p>{next}<w:sectPr/>"#);
    let parts = fill(&tagged, json!({ "doc": "sub" }));

    let xml = part(&parts, "word/document.xml");
    assert!(!xml.contains("replacesection"));
    assert!(xml.starts_with(r#"<w:document xmlns:w="w" xmlns:r="r"><w:body><w:p><w:pPr><w:sectPr><w:headerReference w:type="first" r:id="rIdDocxFillInclude1_rId8"/>"#));
    assert!(xml.contains(r#"<w:sectPr><w:headerReference w:type="default" r:id="rId1"/><w:footerReference w:type="default" r:id="rId2"/><w:pgSz w:w="1"/></w:sectPr>"#));
}

#[test]
fn sets_the_page_orientation_size_and_margins() {
    let page = json!({ "orientation": "landscape", "size": "A3", "margins": { "top": 720, "left": 1440, "gutter": 0 } });
    let xml = part(&fill(&body("{:page p}"), json!({ "p": page })), "word/document.xml");

    assert!(xml.contains(r#"<w:pgSz w:w="23811" w:h="16838" w:orient="landscape"/><w:pgMar w:top="720" w:left="1440" w:gutter="0"/></w:sectPr>"#));
    assert!(!xml.contains("{:page"));

    let xml = part(&fill(&body("{:page p}"), json!({ "p": { "size": [100, 200] } })), "word/document.xml");
    assert!(xml.contains(r#"<w:pgSz w:w="100" w:h="200"/>"#));
}

#[test]
fn drops_the_tag_for_a_missing_or_unknown_value() {
    for data in [json!({}), json!({ "doc": "other" })] {
        let xml = part(&fill(&body("{:subsection doc}"), data), "word/document.xml");
        assert!(!xml.contains("subsection") && xml.contains(MAIN_SECTION));
        assert_eq!(xml.matches("<w:sectPr").count(), 1);
    }
}

#[test]
fn renames_the_styles_and_lists_its_headers_use() {
    let options = FillOptions::default();
    let mut main = main_parts(&body("{:subsection doc}"));
    let main_styles = r#"<w:styles xmlns:w="w"><w:style w:type="paragraph" w:styleId="SubHeader"><w:name w:val="Sub header"/><w:rPr><w:b/></w:rPr></w:style></w:styles>"#;
    main.iter_mut().find(|(name, _)| name == "word/styles.xml").unwrap().1 = main_styles.as_bytes().to_vec();

    let mut sub = sub_parts();
    let header = r#"<w:hdr xmlns:w="w"><w:p><w:pPr><w:pStyle w:val="SubHeader"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="3"/></w:numPr></w:pPr><w:r><w:t>Sub</w:t></w:r></w:p></w:hdr>"#;
    sub.iter_mut().find(|(name, _)| name == "word/header3.xml").unwrap().1 = header.as_bytes().to_vec();
    let numbering = r#"<w:numbering xmlns:w="w"><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="upperRoman"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="3"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#;
    sub.push(("word/numbering.xml".to_string(), numbering.as_bytes().to_vec()));

    let mut template = PackageTemplate::parse(main, &options).unwrap();
    template.add_include("sub", Subtemplate::parse(sub, &options, IncludeMode::Templated, IncludeStyles::KeepOwn).unwrap());
    let parts: Vec<(String, String)> = template.render(&json!({ "doc": "sub" })).into_iter().map(|(name, bytes)| (name, String::from_utf8(bytes.into_owned()).unwrap())).collect();

    let header = part(&parts, "word/docx_fill_include1_header3.xml");
    assert!(header.contains(r#"<w:pStyle w:val="SubHeadersub"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>"#), "{header}");
    assert!(part(&parts, "word/styles.xml").contains(r#"<w:style w:type="paragraph" w:styleId="SubHeadersub"><w:name w:val="Sub header (sub)"/>"#));
    assert!(part(&parts, "word/numbering.xml").contains(r#"<w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>"#));
}
