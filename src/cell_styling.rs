//! `{:stylecell name}`: paints the table cell the tag is in. Built from the WordprocessingML
//! spec: a cell's shading is `<w:shd>` in its `<w:tcPr>`.

use serde_json::Value;

// Cell properties the schema orders after `<w:shd>`.
const AFTER_SHADING: [&str; 11] =
    ["<w:noWrap", "<w:tcMar", "<w:textDirection", "<w:tcFitText", "<w:vAlign", "<w:hideMark", "<w:headers", "<w:cellIns", "<w:cellDel", "<w:cellMerge", "<w:tcPrChange"];

/// The six-digit upper-case hex fill a styling value asks for: `{ "cellBackground": "#abc" }`.
pub(crate) fn read_fill(value: Option<&Value>) -> Option<String> {
    let color = value?.get("cellBackground")?.as_str()?.trim();
    let hex = color.strip_prefix('#').unwrap_or(color);
    if !(hex.len() == 6 || hex.len() == 3) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = if hex.len() == 3 { hex.chars().flat_map(|digit| [digit, digit]).collect() } else { hex.to_string() };
    Some(full.to_ascii_uppercase())
}

/// `cell`, a whole `<w:tc>`, with its shading set to `fill`.
pub(crate) fn shade_cell(cell: &str, fill: &str) -> String {
    let shading = format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fill}\"/>");
    let Some((start, end, inner)) = find_properties(cell) else {
        let open_end = cell.find('>').map_or(0, |at| at + 1);
        return format!("{}<w:tcPr>{shading}</w:tcPr>{}", &cell[..open_end], &cell[open_end..]);
    };

    let inner = remove_first_element(inner, "<w:shd");
    let placed = match AFTER_SHADING.iter().filter_map(|name| find_element_start(&inner, name)).min() {
        Some(at) => format!("{}{shading}{}", &inner[..at], &inner[at..]),
        None => format!("{inner}{shading}"),
    };
    format!("{}<w:tcPr>{placed}</w:tcPr>{}", &cell[..start], &cell[end..])
}

/// The first `<w:tcPr>…</w:tcPr>` or `<w:tcPr/>`: its start, end and content.
fn find_properties(cell: &str) -> Option<(usize, usize, &str)> {
    let full = cell.find("<w:tcPr>").and_then(|start| {
        let end = start + cell[start..].find("</w:tcPr>")? + "</w:tcPr>".len();
        Some((start, end, &cell[start + "<w:tcPr>".len()..end - "</w:tcPr>".len()]))
    });
    let empty = cell.find("<w:tcPr/>").map(|start| (start, start + "<w:tcPr/>".len(), ""));
    match (full, empty) {
        (Some(a), Some(b)) => Some(if a.0 < b.0 { a } else { b }),
        (a, b) => a.or(b),
    }
}

/// Where element `prefix` (such as `<w:vAlign`) starts, matching the whole name.
pub(crate) fn find_element_start(xml: &str, prefix: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(at) = xml[from..].find(prefix) {
        let at = from + at;
        let next = xml[at + prefix.len()..].chars().next();
        if next.is_some_and(|c| c.is_whitespace() || c == '/' || c == '>') {
            return Some(at);
        }
        from = at + prefix.len();
    }
    None
}

/// `xml` without the first self-closing element `prefix`.
pub(crate) fn remove_first_element(xml: &str, prefix: &str) -> String {
    let Some(start) = find_element_start(xml, prefix) else { return xml.to_string() };
    let Some(length) = xml[start..].find("/>") else { return xml.to_string() };
    if xml[start..start + length].contains('>') {
        return xml.to_string();
    }
    format!("{}{}", &xml[..start], &xml[start + length + 2..])
}
