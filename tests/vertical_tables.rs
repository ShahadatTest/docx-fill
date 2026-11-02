//! Mirrors `apps/web/src/components/pdf/wordcraft/__tests__/verticalTables.test.ts`, with the
//! `{-w:tc …}` cell loops `rewriteVerticalTables` writes.

mod common;

use common::{cell, fill, read_rows, row, table};
use serde_json::json;

#[test]
fn repeats_the_tagged_cell_once_per_item() {
    let xml = fill(
        &table(&[row(&[cell(&["Label"]), cell(&["{-w:tc grid:a.value}{name}{/grid:a.value}"])])]),
        json!({ "grid:a.value": [{ "name": "one" }, { "name": "two" }, { "name": "three" }] }),
    );

    assert_eq!(read_rows(&xml), [["Label", "one", "two", "three"]]);
}

#[test]
fn repeats_every_cell_from_the_opening_tags_cell_to_the_closing_tags() {
    let xml = fill(
        &table(&[row(&[cell(&["{-w:tc cols}{a}"]), cell(&["{b}{/cols}"]), cell(&["End"])])]),
        json!({ "cols": [{ "a": "a1", "b": "b1" }, { "a": "a2", "b": "b2" }] }),
    );

    assert_eq!(read_rows(&xml), [["a1", "b1", "a2", "b2", "End"]]);
}

#[test]
fn repeats_columns_in_each_row_of_a_row_loop_with_that_rows_items() {
    let xml = fill(
        &table(&[
            row(&[cell(&["Name"]), cell(&["{-w:tc vt}{header}{/vt}"])]),
            row(&[cell(&["{-w:tr rows}{label}"]), cell(&["{-w:tc vt}{value}{/vt}{/rows}"])]),
        ]),
        json!({
            "vt": [{ "header": "H1" }, { "header": "H2" }],
            "rows": [
                { "label": "r1", "vt": [{ "value": "1a" }, { "value": "1b" }] },
                { "label": "r2", "vt": [{ "value": "2a" }, { "value": "2b" }] }
            ]
        }),
    );

    assert_eq!(read_rows(&xml), [["Name", "H1", "H2"], ["r1", "1a", "1b"], ["r2", "2a", "2b"]]);
}

#[test]
fn reads_a_tag_word_split_across_runs() {
    let xml = fill(&table(&[row(&[cell(&["{-w:t", "c items}{x}{/it", "ems}"])])]), json!({ "items": [{ "x": "1" }, { "x": "2" }] }));

    assert_eq!(read_rows(&xml), [["1", "2"]]);
}

#[test]
fn leaves_the_row_without_the_cells_when_the_list_is_empty() {
    let xml = fill(&table(&[row(&[cell(&["Label"]), cell(&["{-w:tc items}{x}{/items}"])])]), json!({ "items": [] }));

    assert_eq!(read_rows(&xml), [["Label"]]);
}
