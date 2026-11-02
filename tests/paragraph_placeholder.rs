mod common;

use common::{fill, paragraph, paragraph_texts};
use serde_json::json;

fn texts(body: &str, data: serde_json::Value) -> Vec<String> {
    paragraph_texts(&fill(body, data))
}

#[test]
fn drops_the_paragraph_for_a_value_a_loop_skips() {
    let body = format!("{}{}", paragraph(&["Note: {?note}"]), paragraph(&["end"]));

    for value in [json!(null), json!(false), json!(""), json!(0), json!([])] {
        assert_eq!(texts(&body, json!({ "note": value })), vec!["end"]);
    }
    assert_eq!(texts(&body, json!({})), vec!["end"]);
}

#[test]
fn keeps_the_paragraph_printing_text_or_nothing_for_true() {
    let body = paragraph(&["Note: {?note}"]);

    assert_eq!(texts(&body, json!({ "note": "wet" })), vec!["Note: wet"]);
    assert_eq!(texts(&body, json!({ "note": true })), vec!["Note: "]);
}

#[test]
fn repeats_the_paragraph_for_each_item() {
    assert_eq!(texts(&paragraph(&["- {?users}"]), json!({ "users": ["John", "Jack"] })), vec!["- John", "- Jack"]);
    assert_eq!(
        texts(&paragraph(&["- {?users}{user} likes {sport}"]), json!({ "users": [{ "user": "John", "sport": "Golf" }, { "user": "Ann", "sport": "Chess" }] })),
        vec!["- John likes Golf", "- Ann likes Chess"]
    );
}

#[test]
fn drops_the_paragraph_when_any_of_its_placeholders_is_empty() {
    let body = paragraph(&["{?a} and {?b}"]);

    assert!(texts(&body, json!({ "a": "x", "b": "" })).is_empty());
    assert_eq!(texts(&body, json!({ "a": "x", "b": "y" })), vec!["x and y"]);
}

#[test]
fn works_inside_a_loop() {
    let body = format!("{}{}{}", paragraph(&["{#rows}"]), paragraph(&["{?note}"]), paragraph(&["{/rows}"]));

    assert_eq!(texts(&body, json!({ "rows": [{ "note": "a" }, { "note": null }, { "note": "c" }] })), vec!["a", "c"]);
}
