//! Delimiters other than `{ }`, as a signatures pass uses `[[ ]]`.

mod common;

use common::{document, paragraph, text};
use docx_fill::{Delimiters, FillOptions, fill_part};
use serde_json::json;

fn fill_with(body: &str, start: &str, end: &str) -> Result<String, String> {
    let options = FillOptions { delimiters: Delimiters { start: start.into(), end: end.into() }, ..FillOptions::default() };
    fill_part(&document(body), &json!({ "name": "N", "rows": [{ "x": 1 }, { "x": 2 }] }), &options).map(|xml| text(&xml)).map_err(|error| error.message)
}

#[test]
fn fills_tags_between_two_character_delimiters() {
    assert_eq!(fill_with(&paragraph(&["[[name]] [[#rows]][[x]][[/rows]]"]), "[[", "]]").unwrap(), "N 12");
}

#[test]
fn leaves_the_other_delimiters_as_text() {
    assert_eq!(fill_with(&paragraph(&["{name} [ x ] [[na", "me]]"]), "[[", "]]").unwrap(), "{name} [ x ] N");
}

#[test]
fn rejects_an_unclosed_tag() {
    assert_eq!(fill_with(&paragraph(&["[[name"]), "[[", "]]").unwrap_err(), r#"The tag beginning with "[[name" is unclosed"#);
}

#[test]
fn fills_tags_between_doubled_braces() {
    assert_eq!(fill_with(&paragraph(&["{{name}}"]), "{{", "}}").unwrap(), "N");
}

#[test]
fn fills_tags_when_start_and_end_delimiter_are_the_same() {
    assert_eq!(fill_with(&paragraph(&["%name%"]), "%", "%").unwrap(), "N");
}

#[test]
fn fills_several_same_delimiter_tags_in_one_paragraph() {
    let options = FillOptions { delimiters: Delimiters { start: "%".into(), end: "%".into() }, ..FillOptions::default() };
    let xml = fill_part(&document(&paragraph(&["%a% and %b%"])), &json!({ "a": "one", "b": "two" }), &options)
        .map(|xml| text(&xml))
        .unwrap();
    assert_eq!(xml, "one and two");
}
