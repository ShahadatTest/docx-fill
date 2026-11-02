//! Mirrors the loop shapes of `apps/web/src/components/pdf/wordcraft/__tests__/markTemplateTags.test.ts`.

mod common;

use common::{elements, fill, paragraph, paragraph_texts, row, table, text};
use serde_json::json;

#[test]
fn repeats_a_loop_spanning_paragraphs_between_its_tags() {
    let template = paragraph(&["{#table:t.value}", "{textinput:first}"])
        + &paragraph(&["{textinput:second}"])
        + &paragraph(&["{numberinput:last}{/table:t.value}"])
        + &paragraph(&["{textinput:after.value}"]);

    let body = fill(
        &template,
        json!({
            "table:t.value": [
                { "textinput:first": "f1", "textinput:second": "s1", "numberinput:last": 1 },
                { "textinput:first": "f2", "textinput:second": "s2", "numberinput:last": 2 }
            ],
            "textinput:after.value": "after"
        }),
    );

    assert_eq!(paragraph_texts(&body), ["f1", "s1", "1f2", "s2", "2", "after"]);
}

#[test]
fn repeats_a_row_whose_loop_tags_sit_in_separate_paragraphs() {
    let cell = format!("<w:tc>{}{}</w:tc>", paragraph(&["{-w:tr table:r.value}{textinput:row}"]), paragraph(&["{textinput:cell}{/table:r.value}"]));
    let body = fill(
        &table(&[row(&[cell])]),
        json!({ "table:r.value": [{ "textinput:row": "r1", "textinput:cell": "c1" }, { "textinput:row": "r2", "textinput:cell": "c2" }] }),
    );

    let rows: Vec<Vec<String>> = elements(&body, "w:tr").into_iter().map(paragraph_texts).collect();
    assert_eq!(rows, [["r1", "c1"], ["r2", "c2"]]);
}

#[test]
fn prints_nothing_for_image_tags_without_their_module() {
    let body = fill(&paragraph(&["[{%photo:p.value.image}]"]), json!({ "photo:p.value": { "image": "x" } }));

    assert_eq!(paragraph_texts(&body), ["[]"]);
}

#[test]
fn replaces_a_raw_tags_paragraph_with_its_xml() {
    let template = paragraph(&["before"]) + &paragraph(&["{@textinput:h.value}"]) + &paragraph(&["after"]);

    let body = fill(&template, json!({ "textinput:h.value": "<w:p><w:r><w:t>raw</w:t></w:r></w:p>" }));

    assert_eq!(paragraph_texts(&body), ["before", "raw", "after"]);
}

#[test]
fn leaves_a_self_closing_paragraph_and_the_next_one_intact() {
    let body = fill(&format!("<w:p/>{}", paragraph(&["{textinput:x.value}"])), json!({ "textinput:x.value": "x" }));

    assert!(body.starts_with("<w:p/><w:p>"));
    assert_eq!(text(&body), "x");
}

#[test]
fn fills_a_text_boxs_paragraph_and_keeps_the_anchoring_one() {
    let template = format!(
        "<w:p><w:r><w:t>anchor </w:t></w:r><w:r><w:drawing><w:txbxContent>{}</w:txbxContent></w:drawing></w:r></w:p>",
        paragraph(&["{textinput:boxed.value}"])
    );

    let body = fill(&template, json!({ "textinput:boxed.value": "boxed" }));

    assert_eq!(text(&body), "anchor boxed");
    assert!(body.contains("<w:txbxContent><w:p><w:r><w:t xml:space=\"preserve\">boxed</w:t></w:r></w:p></w:txbxContent>"));
}

#[test]
fn repeats_a_loop_inside_a_text_box() {
    let boxed = paragraph(&["{#items}"]) + &paragraph(&["{.}"]) + &paragraph(&["{/items}"]);
    let template = format!("<w:p><w:r><w:drawing><w:txbxContent>{boxed}</w:txbxContent></w:drawing></w:r></w:p>");

    let body = fill(&template, json!({ "items": ["a", "b"] }));

    assert_eq!(text(&body), "ab");
}
