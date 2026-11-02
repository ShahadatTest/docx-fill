mod common;

use common::fill;
use serde_json::json;

#[test]
fn replaces_the_tags_run_with_raw_xml() {
    let body = fill(
        r#"<w:p><w:r><w:t>a</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>{r@run}</w:t></w:r><w:r><w:t>b</w:t></w:r></w:p>"#,
        json!({ "run": "<w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r>" }),
    );

    assert_eq!(body, r#"<w:p><w:r><w:t>a</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t>b</w:t></w:r></w:p>"#);
}

#[test]
fn drops_the_run_for_a_missing_value() {
    let body = fill(r#"<w:p><w:r><w:t>{r@run}</w:t></w:r></w:p>"#, json!({}));

    assert_eq!(body, "<w:p></w:p>");
}
