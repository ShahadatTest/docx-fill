//! Mirrors `apps/web/src/components/pdf/wordcraft/__tests__/cellStyling.test.ts` in Scope's
//! frontend: `{:stylecell name}` fills its table cell.

mod common;

use common::{elements, fill};
use serde_json::{Value, json};

fn cell(text: &str, properties: &str) -> String {
    format!("<w:tc>{properties}<w:p><w:r><w:t>{text}</w:t></w:r></w:p></w:tc>")
}

fn fill_row(row: &str, tags: Value) -> Vec<String> {
    let body = fill(&format!("<w:tbl><w:tr>{row}</w:tr></w:tbl>"), tags);
    elements(&body, "w:tc").into_iter().map(str::to_string).collect()
}

fn fill_of(cell: &str) -> Option<&str> {
    let at = cell.find("<w:shd ")?;
    let fill = &cell[at..][cell[at..].find("w:fill=\"")? + 8..];
    Some(&fill[..fill.find('"')?])
}

#[test]
fn fills_the_tags_cell_with_its_background_and_prints_nothing() {
    let cells = fill_row(&(cell("{:stylecell statusColor}Status", "") + &cell("Other", "")), json!({ "statusColor": { "cellBackground": "#ff0000" } }));

    assert_eq!(fill_of(&cells[0]), Some("FF0000"));
    assert!(cells[0].contains(">Status<"));
    assert_eq!(fill_of(&cells[1]), None);
}

#[test]
fn leaves_the_cell_alone_when_the_value_has_no_colour() {
    for value in [json!({}), json!({ "cellBackground": "red" }), json!("#ff0000"), json!(null)] {
        let cells = fill_row(&cell("{:stylecell statusColor}", ""), json!({ "statusColor": value }));
        assert_eq!(fill_of(&cells[0]), None, "{value}");
    }
}

#[test]
fn keeps_the_schemas_order_in_the_cells_properties() {
    let properties = r#"<w:tcPr><w:tcW w:w="100" w:type="dxa"/><w:shd w:val="clear" w:fill="00FF00"/><w:vAlign w:val="center"/></w:tcPr>"#;

    let cells = fill_row(&cell("{:stylecell c}", properties), json!({ "c": { "cellBackground": "#abc" } }));

    assert!(cells[0].contains(r#"<w:tcW w:w="100" w:type="dxa"/><w:shd w:val="clear" w:color="auto" w:fill="AABBCC"/><w:vAlign "#));
}

#[test]
fn adds_properties_to_a_cell_without_them() {
    let cells = fill_row(&cell("{:stylecell c}", ""), json!({ "c": { "cellBackground": "123456" } }));

    assert!(cells[0].starts_with(r#"<w:tc><w:tcPr><w:shd w:val="clear" w:color="auto" w:fill="123456"/></w:tcPr><w:p>"#));
}

#[test]
fn styles_each_repeated_cell_of_a_vertical_table_with_its_own_item() {
    let cells = fill_row(
        &cell("{:vt#cols}{:stylecell color}{name}{:vt/cols}", ""),
        json!({ "cols": [{ "name": "a", "color": { "cellBackground": "#111111" } }, { "name": "b", "color": {} }] }),
    );

    assert_eq!(cells.iter().map(|each| fill_of(each)).collect::<Vec<_>>(), [Some("111111"), None]);
}

#[test]
fn styles_the_innermost_cell_of_a_nested_table() {
    let inner = format!("<w:tbl><w:tr>{}</w:tr></w:tbl>", cell("{:stylecell c}", ""));
    let body = fill(&format!("<w:tbl><w:tr><w:tc>{inner}<w:p/></w:tc></w:tr></w:tbl>"), json!({ "c": { "cellBackground": "#00ff00" } }));

    assert!(body.starts_with("<w:tbl><w:tr><w:tc><w:tbl><w:tr><w:tc><w:tcPr><w:shd "));
}
