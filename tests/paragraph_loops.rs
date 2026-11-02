//! Mirrors the loop cases of `apps/web/src/components/pdf/wordcraft/__tests__/richText.test.ts`:
//! a tag paragraph repeated by a loop whose tags sit alone in their paragraphs.

mod common;

use common::{fill, paragraph, paragraph_texts};
use serde_json::json;

const BASE_PPR: &str = r#"<w:pPr><w:pStyle w:val="Body"/></w:pPr>"#;
const BASE_RPR: &str = r#"<w:rPr><w:rFonts w:ascii="Arial"/></w:rPr>"#;

fn tag_paragraph(tag: &str) -> String {
    format!(
        r#"<w:p>{BASE_PPR}<w:bookmarkStart w:id="1" w:name="scope_tag_0"/><w:bookmarkEnd w:id="1"/><w:r>{BASE_RPR}<w:t xml:space="preserve">{{{tag}}}</w:t></w:r></w:p>"#
    )
}

fn looped(tag: &str) -> String {
    paragraph(&["{#rows}"]) + &tag_paragraph(tag) + &paragraph(&["{/rows}"])
}

#[test]
fn fills_every_repeat_of_a_tag_inside_a_loop() {
    let body = fill(&looped("note"), json!({ "rows": [{ "note": "a" }, { "note": "b" }] }));

    assert_eq!(paragraph_texts(&body), ["a", "b"]);
}

#[test]
fn keeps_the_repeated_paragraphs_style_run_font_and_bookmark() {
    let body = fill(&looped("note"), json!({ "rows": [{ "note": "a" }, { "note": "b" }] }));

    let printed: Vec<&str> = body.split("</w:p>").filter(|each| !each.is_empty()).collect();
    assert_eq!(printed.len(), 2);
    for paragraph in printed {
        assert!(paragraph.contains(r#"<w:pStyle w:val="Body"/>"#));
        assert!(paragraph.contains(r#"<w:rFonts w:ascii="Arial"/>"#));
        assert!(paragraph.contains(r#"w:name="scope_tag_0""#));
    }
}

#[test]
fn drops_the_loop_and_its_tag_paragraphs_for_an_empty_list() {
    let body = fill(&(looped("note") + &paragraph(&["after"])), json!({ "rows": [] }));

    assert_eq!(paragraph_texts(&body), ["after"]);
}

#[test]
fn repeats_inside_the_tag_paragraphs_when_a_tag_shares_its_paragraph() {
    let template = paragraph(&["Rows: {#rows}"]) + &paragraph(&["[{note}]"]) + &paragraph(&["{/rows}"]);

    let body = fill(&template, json!({ "rows": [{ "note": "a" }, { "note": "b" }] }));

    assert_eq!(paragraph_texts(&body), ["Rows: ", "[a]", "", "[b]", ""]);
}

#[test]
fn counts_spaces_beside_a_tag_as_other_text() {
    let template = paragraph(&[" {#rows} "]) + &paragraph(&["[{note}]"]) + &paragraph(&["{/rows}"]);

    let body = fill(&template, json!({ "rows": [{ "note": "a" }] }));

    assert_eq!(paragraph_texts(&body), ["  ", "[a]", ""]);
}
