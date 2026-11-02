mod common;

use common::{cell, elements, fill, row, table};
use serde_json::json;

#[test]
fn styles_the_tags_paragraph_and_its_runs() {
    let body = fill(
        r#"<w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:t>{:stylepar s}Title</w:t></w:r></w:p><w:p><w:r><w:t>plain</w:t></w:r></w:p>"#,
        json!({ "s": { "textAlign": "justify", "pStyle": "Heading1", "textColor": "#f00", "fontSize": 28, "bold": true, "cellBackground": "#00ff00" } }),
    );

    let [styled, plain] = elements(&body, "w:p")[..] else { panic!("two paragraphs") };
    assert!(styled.starts_with(
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:shd w:val="clear" w:color="auto" w:fill="00FF00"/><w:spacing w:after="0"/><w:jc w:val="both"/><w:rPr><w:b/><w:bCs/><w:color w:val="FF0000"/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr></w:pPr>"#
    ));
    assert!(styled.contains(r#"<w:r><w:rPr><w:b/><w:bCs/><w:color w:val="FF0000"/>"#));
    assert!(!plain.contains("w:rPr"));
}

#[test]
fn replaces_properties_the_run_already_has() {
    let body = fill(
        r#"<w:p><w:r><w:rPr><w:b/><w:color w:val="111111"/><w:u w:val="double"/></w:rPr><w:t>{:stylerun s}text</w:t></w:r></w:p>"#,
        json!({ "s": { "bold": false, "textColor": "#222222", "underline": true, "italic": true, "fontFamily": "Arial" } }),
    );

    assert!(body.contains(
        r#"<w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:cs="Arial" w:eastAsia="Arial"/><w:b w:val="0"/><w:bCs w:val="0"/><w:i/><w:iCs/><w:color w:val="222222"/><w:u w:val="single"/></w:rPr>"#
    ));
}

#[test]
fn styles_only_the_run_holding_the_tag() {
    let body = fill(r#"<w:p><w:r><w:t>before </w:t></w:r><w:r><w:t>{:stylerun s}styled</w:t></w:r></w:p>"#, json!({ "s": { "italic": true } }));

    let runs = elements(&body, "w:r");
    assert!(!runs[0].contains("<w:i/>"));
    assert!(runs[1].contains("<w:i/>"));
}

#[test]
fn styles_every_cell_of_the_tags_row() {
    let body = fill(
        &table(&[row(&[cell(&["{:stylerow s}a"]), cell(&["b"])]), row(&[cell(&["c"]), cell(&["d"])])]),
        json!({ "s": { "cellBackground": "#123456", "textColor": "#abcdef", "borders": { "bottom": { "val": "single", "sz": 4, "color": "#000000" } } } }),
    );

    let rows = elements(&body, "w:tr");
    for styled in elements(rows[0], "w:tc") {
        assert!(styled.contains(r#"<w:tcBorders><w:bottom w:val="single" w:sz="4" w:color="000000"/></w:tcBorders><w:shd w:val="clear" w:color="auto" w:fill="123456"/>"#));
        assert!(styled.contains(r#"<w:color w:val="ABCDEF"/>"#));
    }
    assert!(!rows[1].contains("w:shd"));
}

#[test]
fn styles_a_cells_text_as_well_as_its_fill() {
    let body = fill(&table(&[row(&[cell(&["{:stylecell s}a"])])]), json!({ "s": { "cellBackground": "#fff", "textAlign": "center" } }));

    assert!(body.contains(r#"w:fill="FFFFFF""#));
    assert!(body.contains(r#"<w:jc w:val="center"/>"#));
}

#[test]
fn ignores_a_style_that_is_not_an_object() {
    let template = r#"<w:p><w:r><w:t>{:stylepar s}x</w:t></w:r></w:p>"#;

    for value in [json!("bold"), json!(null), json!([1])] {
        assert_eq!(fill(template, json!({ "s": value })), r#"<w:p><w:r><w:t xml:space="preserve">x</w:t></w:r></w:p>"#);
    }
}
