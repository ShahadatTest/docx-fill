//! Plain tags, loop values, scopes, line breaks and template errors.

mod common;

use common::{cell, document, fill, paragraph, paragraph_texts, read_rows, row, table, text};
use docx_fill::{FillOptions, ModuleTag, Template, fill_part};
use serde_json::{Value, json};

fn fill_error(body: &str) -> String {
    fill_part(&document(body), &json!({}), &FillOptions::default()).unwrap_err().message
}

#[test]
fn prints_nothing_for_a_missing_or_null_value() {
    let body = fill(&paragraph(&["[{missing}|{empty}]"]), json!({ "empty": null }));

    assert_eq!(text(&body), "[|]");
}

#[test]
fn looks_a_name_up_whole_including_colons_and_dots() {
    let body = fill(&paragraph(&["{table:abc.value}"]), json!({ "table:abc.value": "flat", "table:abc": { "value": "nested" } }));

    assert_eq!(text(&body), "flat");
}

#[test]
fn reads_the_tag_with_its_spaces() {
    let body = fill(&paragraph(&["{ name }"]), json!({ "name": "N", " name ": "S" }));

    assert_eq!(text(&body), "S");
}

#[test]
fn looks_a_name_up_from_the_innermost_item_out_to_the_root() {
    let body = fill(
        &paragraph(&["{#a}{#b}{x}{y}{z}{/b}{/a}"]),
        json!({ "y": "root", "z": "rootz", "a": [{ "x": "ax", "z": "az", "b": [{ "z": null }] }] }),
    );

    assert_eq!(text(&body), "axrootaz");
}

#[test]
fn prints_values_as_javascript_does() {
    let body = fill(&paragraph(&["{n}|{f}|{w}|{b}|{o}|{a}|{z}"]), json!({ "n": 1, "f": 1.5, "w": 2.0, "b": false, "o": { "x": 1 }, "a": [1, 2], "z": 0 }));

    assert_eq!(text(&body), "1|1.5|2|false|[object Object]|1,2|0");
}

#[test]
fn escapes_values() {
    let body = fill(&paragraph(&["{v}"]), json!({ "v": "a<b & \"c\"" }));

    assert_eq!(text(&body), "a&lt;b &amp; &quot;c&quot;");
}

#[test]
fn prints_a_newline_as_a_line_break_in_the_same_run() {
    let body = fill(r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>{v}</w:t></w:r></w:p>"#, json!({ "v": "one\ntwo" }));

    assert_eq!(body, r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">one</w:t><w:br/><w:t xml:space="preserve">two</w:t></w:r></w:p>"#);
}

#[test]
fn keeps_the_first_runs_formatting_for_a_split_tag() {
    let body = fill(r#"<w:p><w:r><w:t>{na</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>me} tail</w:t></w:r></w:p>"#, json!({ "name": "V" }));

    assert_eq!(body, r#"<w:p><w:r><w:t xml:space="preserve">V</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t xml:space="preserve"> tail</w:t></w:r></w:p>"#);
}

#[test]
fn leaves_text_without_tags_as_written() {
    let template = r#"<w:p><w:r><w:t>plain &amp; text</w:t></w:r><w:r><w:instrText>{ not a tag</w:instrText></w:r></w:p>"#;

    assert_eq!(fill(template, json!({})), template);
}

#[test]
fn loops_over_each_kind_of_value() {
    let template = paragraph(&["{#v}[{.}|{x}]{/v}"]);
    let cases: [(Value, &str); 9] = [
        (json!([{ "x": 1 }, { "x": 2 }]), "[[object Object]|1][[object Object]|2]"),
        (json!(["a", "b"]), "[a|X][b|X]"),
        (json!({ "x": "in" }), "[[object Object]|in]"),
        (json!(true), "[[object Object]|X]"),
        (json!("text"), "[[object Object]|X]"),
        (json!(0), ""),
        (json!(""), ""),
        (json!([]), ""),
        (json!(null), ""),
    ];
    for (value, expected) in cases {
        let body = fill(&template, json!({ "v": value, "x": "X" }));
        assert_eq!(text(&body), expected, "{value}");
    }
}

#[test]
fn prints_an_inverted_section_only_for_a_value_a_loop_skips() {
    let template = paragraph(&["{^v}none{/v}"]);
    for (value, expected) in [(json!([]), "none"), (json!(false), "none"), (json!(0), "none"), (json!([1]), ""), (json!({}), ""), (json!("x"), "")] {
        assert_eq!(text(&fill(&template, json!({ "v": value }))), expected, "{value}");
    }
}

#[test]
fn closes_any_loop_with_an_empty_closing_tag() {
    assert_eq!(text(&fill(&paragraph(&["{#a}x{/}"]), json!({ "a": true }))), "x");
}

#[test]
fn joins_the_paragraphs_around_a_loop_spanning_them_that_prints_nothing() {
    let body = fill(&(paragraph(&["A{#a}B"]) + &paragraph(&["C{/a}D"])), json!({ "a": [] }));

    assert_eq!(paragraph_texts(&body), ["AD"]);
}

#[test]
fn repeats_rows_for_a_loop_whose_tags_sit_in_different_cells() {
    let body = fill(&table(&[row(&[cell(&["{#a}{x}"]), cell(&["{/a}"])])]), json!({ "a": [{ "x": 1 }, { "x": 2 }] }));

    assert_eq!(read_rows(&body), [["1", ""], ["2", ""]]);
}

#[test]
fn repeats_a_dash_paragraph_loop() {
    let body = fill(&(paragraph(&["{-w:p items}{.}{/items}"]) + &paragraph(&["end"])), json!({ "items": ["a", "b"] }));

    assert_eq!(paragraph_texts(&body), ["a", "b", "end"]);
}

#[test]
fn hands_module_tags_to_the_module() {
    let options = FillOptions { module_prefixes: vec!["~~".into(), ":stylecell ".into()], ..FillOptions::default() };
    let template = Template::parse(&document(&paragraph(&["{~~notes}|{:stylecell fill}|{plain}"])), &options).unwrap();

    let xml = template.render_with(&json!({ "notes": "<p>n</p>", "plain": "p" }), &mut |tag: ModuleTag| {
        format!("{}:{}={}", tag.module, tag.name, tag.value.map_or("none".to_string(), |value| value.to_string()))
    });

    assert_eq!(text(&xml), r#"0:notes="<p>n</p>"|1:fill=none|p"#);
}

#[test]
fn reuses_a_parsed_template() {
    let template = Template::parse(&document(&paragraph(&["{v}"])), &FillOptions::default()).unwrap();

    assert_eq!(text(&template.render(&json!({ "v": "one" }))), "one");
    assert_eq!(text(&template.render(&json!({ "v": "two" }))), "two");
}

#[test]
fn rejects_a_broken_template() {
    assert_eq!(fill_error(&paragraph(&["{#a}x"])), r#"The loop with tag "a" is unclosed"#);
    assert_eq!(fill_error(&paragraph(&["{#a}x{/b}"])), r#"The tag "a" is closed by the tag "b""#);
    assert_eq!(fill_error(&paragraph(&["a } b"])), r#"The tag beginning with "a " is unopened"#);
    assert_eq!(fill_error(&paragraph(&["a { b"])), r#"The tag beginning with "{ b" is unclosed"#);
    assert_eq!(fill_error(&paragraph(&["x{/a}"])), r#"The tag "/a" has no opening loop tag"#);
    assert_eq!(fill_error(&paragraph(&["{-w:tr a}{/a}"])), r#"The tag "-w:tr a" is not inside a <w:tr>"#);
}
