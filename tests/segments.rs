mod common;

use common::{fill, paragraph_texts};
use docx_fill::{FillOptions, fill_part};
use serde_json::json;

fn paragraph(text: &str) -> String {
    format!("<w:p><w:r><w:t>{text}</w:t></w:r></w:p>")
}

#[test]
fn prints_a_segment_only_where_included_with_that_scope() {
    let body = [
        paragraph("{:segment user}"),
        paragraph("Hello {name}!"),
        paragraph("{:segment/}"),
        paragraph("{#users}"),
        paragraph("{:includesegment user}"),
        paragraph("{/users}"),
    ]
    .concat();

    let xml = fill(&body, json!({ "users": [{ "name": "Ada" }, { "name": "Bo" }] }));
    assert_eq!(paragraph_texts(&xml), ["Hello Ada!", "Hello Bo!"]);
}

#[test]
fn includes_segments_named_by_the_value() {
    let body = [
        paragraph("{:segment part1}"),
        paragraph("One"),
        paragraph("{:segment/}"),
        paragraph("{:segment part2}"),
        paragraph("Two"),
        paragraph("{:segment/}"),
        paragraph("{#parts}"),
        paragraph("{:includesegment* .}"),
        paragraph("{/parts}"),
    ]
    .concat();

    let xml = fill(&body, json!({ "parts": ["part2", "part2", "part1", "none"] }));
    assert_eq!(paragraph_texts(&xml), ["Two", "Two", "One"]);
}

#[test]
fn includes_inline_segments_within_a_paragraph() {
    let body = [paragraph("{:inlinesegment first}ABC{:inlinesegment/}{:inlinesegment second}D{x}{:inlinesegment/}"), paragraph("Alphabet: {:includeinlinesegment first}{:includeinlinesegment second}")].concat();

    let xml = fill(&body, json!({ "x": "EF" }));
    assert_eq!(paragraph_texts(&xml), ["", "Alphabet: ABCDEF"]);
}

#[test]
fn stops_a_segment_that_includes_itself() {
    let body = [paragraph("{:segment loop}"), paragraph("x"), paragraph("{:includesegment loop}"), paragraph("{:segment/}"), paragraph("{:includesegment loop}")].concat();

    assert_eq!(paragraph_texts(&fill(&body, json!({}))).len(), 32);
}

#[test]
fn rejects_an_undefined_segment_and_a_segment_tag_with_other_text() {
    for (body, message) in [
        (paragraph("{:includesegment missing}"), "The subsegment for \"missing\" must be defined in order to include it"),
        ([paragraph("{:segment s} text"), paragraph("{:segment/}")].concat(), "Segment tag should be the only text in a paragraph"),
        (paragraph("a {:includesegment s}"), "Segment tag should be the only text in a paragraph"),
    ] {
        let error = fill_part(&common::document(&body), &json!({}), &FillOptions::default()).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}
