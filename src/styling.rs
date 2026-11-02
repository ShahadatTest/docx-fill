//! `{:stylepar}`, `{:stylerow}` and `{:stylerun}`, and the properties `{:stylecell}` sets beyond its
//! fill. Built from the WordprocessingML spec and the styling tags' documented values.

use serde_json::Value;

use crate::cell_styling::{read_fill, shade_cell};
use crate::properties::{CELL_ORDER, PARAGRAPH_ORDER, RUN_ORDER, edit_each, edit_properties, set_child};
use crate::xml::escape;

// Border sides each container takes, in schema order.
const PARAGRAPH_SIDES: &[&str] = &["top", "left", "bottom", "right", "between", "bar"];
const CELL_SIDES: &[&str] = &["top", "start", "left", "bottom", "end", "right", "insideH", "insideV", "tl2br", "tr2bl"];

/// What a styling tag's value asks for.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Style {
    /// Six-digit hex.
    pub fill: Option<String>,
    pub text_color: Option<String>,
    /// A `w:jc` value.
    pub align: Option<&'static str>,
    pub paragraph_style: Option<String>,
    /// Half-points, as `w:sz`.
    pub font_size: Option<u64>,
    pub font_family: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    /// Side and its `w:val`, `w:sz`, `w:space` and `w:color` attributes.
    pub borders: Vec<(String, String)>,
}

impl Style {
    fn has_run_properties(&self) -> bool {
        self.text_color.is_some() || self.font_size.is_some() || self.font_family.is_some() || self.bold.is_some() || self.italic.is_some() || self.underline.is_some()
    }

    fn has_paragraph_properties(&self) -> bool {
        self.align.is_some() || self.paragraph_style.is_some() || self.has_run_properties()
    }
}

/// The style an object value asks for; `None` for any other value.
///
/// Keys: `cellBackground`, `textColor` (hex), `textAlign` (`left`, `center`, `right`, `justify`),
/// `pStyle`, `fontSize` (half-points), `fontFamily`, `bold`, `italic`, `underline` and
/// `borders` (`{ "top": { "val", "sz", "space", "color" } }`).
pub(crate) fn read_style(value: Option<&Value>) -> Option<Style> {
    let value = value.filter(|value| value.is_object())?;
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::trim).filter(|text| !text.is_empty());
    let flag = |key: &str| value.get(key).and_then(Value::as_bool);
    let borders = value.get("borders").and_then(Value::as_object).map_or_else(Vec::new, |sides| {
        sides.iter().filter_map(|(side, border)| Some((side.clone(), read_border(border)?))).collect()
    });
    Some(Style {
        fill: read_fill(Some(value)),
        text_color: text("textColor").and_then(read_hex),
        align: text("textAlign").and_then(read_align),
        paragraph_style: text("pStyle").map(str::to_string),
        font_size: value.get("fontSize").and_then(Value::as_f64).filter(|size| *size > 0.0).map(|size| size.round() as u64),
        font_family: text("fontFamily").map(str::to_string),
        bold: flag("bold"),
        italic: flag("italic"),
        underline: flag("underline"),
        borders,
    })
}

fn read_hex(color: &str) -> Option<String> {
    let hex = color.strip_prefix('#').unwrap_or(color);
    if !(hex.len() == 6 || hex.len() == 3) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = if hex.len() == 3 { hex.chars().flat_map(|digit| [digit, digit]).collect() } else { hex.to_string() };
    Some(full.to_ascii_uppercase())
}

fn read_align(align: &str) -> Option<&'static str> {
    match align {
        "left" | "start" => Some("left"),
        "center" => Some("center"),
        "right" | "end" => Some("right"),
        "justify" | "both" => Some("both"),
        _ => None,
    }
}

/// A border's attributes, as written inside its element.
fn read_border(border: &Value) -> Option<String> {
    let mut attributes = String::new();
    for key in ["val", "sz", "space", "color"] {
        let Some(value) = border.get(key) else { continue };
        let value = match value {
            Value::String(text) if key == "color" => read_hex(text).unwrap_or_else(|| text.clone()),
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            _ => continue,
        };
        attributes.push_str(&format!(" w:{key}=\"{}\"", escape_attribute(&value)));
    }
    Some(attributes)
}

fn escape_attribute(text: &str) -> String {
    let mut out = String::new();
    escape(text, &mut out);
    out
}

fn write_borders(name: &str, sides: &[&str], borders: &[(String, String)]) -> String {
    let mut out = format!("<{name}>");
    for side in sides {
        if let Some((_, attributes)) = borders.iter().find(|(each, _)| each == side) {
            out.push_str(&format!("<w:{side}{attributes}/>"));
        }
    }
    out.push_str(&format!("</{name}>"));
    out
}

/// `<w:rPr>` content with the style's run properties set.
fn style_run_properties(inner: &str, style: &Style) -> String {
    let mut inner = inner.to_string();
    let toggle = |is_on: bool, name: &str| if is_on { format!("<{name}/>") } else { format!("<{name} w:val=\"0\"/>") };
    if let Some(family) = &style.font_family {
        let family = escape_attribute(family);
        inner = set_child(&inner, "w:rFonts", &format!("<w:rFonts w:ascii=\"{family}\" w:hAnsi=\"{family}\" w:cs=\"{family}\" w:eastAsia=\"{family}\"/>"), RUN_ORDER);
    }
    if let Some(bold) = style.bold {
        inner = set_child(&inner, "w:b", &toggle(bold, "w:b"), RUN_ORDER);
        inner = set_child(&inner, "w:bCs", &toggle(bold, "w:bCs"), RUN_ORDER);
    }
    if let Some(italic) = style.italic {
        inner = set_child(&inner, "w:i", &toggle(italic, "w:i"), RUN_ORDER);
        inner = set_child(&inner, "w:iCs", &toggle(italic, "w:iCs"), RUN_ORDER);
    }
    if let Some(color) = &style.text_color {
        inner = set_child(&inner, "w:color", &format!("<w:color w:val=\"{color}\"/>"), RUN_ORDER);
    }
    if let Some(size) = style.font_size {
        inner = set_child(&inner, "w:sz", &format!("<w:sz w:val=\"{size}\"/>"), RUN_ORDER);
        inner = set_child(&inner, "w:szCs", &format!("<w:szCs w:val=\"{size}\"/>"), RUN_ORDER);
    }
    if let Some(underline) = style.underline {
        let value = if underline { "single" } else { "none" };
        inner = set_child(&inner, "w:u", &format!("<w:u w:val=\"{value}\"/>"), RUN_ORDER);
    }
    inner
}

/// `run`, a whole `<w:r>`, with the style's run properties.
pub(crate) fn style_run(run: &str, style: &Style) -> String {
    if !style.has_run_properties() {
        return run.to_string();
    }
    edit_properties(run, "w:rPr", |inner| style_run_properties(inner, style))
}

/// `paragraph`, a whole `<w:p>`, with the style's paragraph properties, and its run properties on
/// every run and the paragraph mark. `is_own` adds the fill and borders, which a cell's style
/// keeps for the cell.
pub(crate) fn style_paragraph(paragraph: &str, style: &Style, is_own: bool) -> String {
    let has_own = is_own && (style.fill.is_some() || !style.borders.is_empty());
    if !style.has_paragraph_properties() && !has_own {
        return paragraph.to_string();
    }
    let styled = edit_properties(paragraph, "w:pPr", |inner| {
        let mut inner = inner.to_string();
        if let Some(name) = &style.paragraph_style {
            inner = set_child(&inner, "w:pStyle", &format!("<w:pStyle w:val=\"{}\"/>", escape_attribute(name)), PARAGRAPH_ORDER);
        }
        if is_own && !style.borders.is_empty() {
            inner = set_child(&inner, "w:pBdr", &write_borders("w:pBdr", PARAGRAPH_SIDES, &style.borders), PARAGRAPH_ORDER);
        }
        if let (true, Some(fill)) = (is_own, &style.fill) {
            inner = set_child(&inner, "w:shd", &format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{fill}\"/>"), PARAGRAPH_ORDER);
        }
        if let Some(align) = style.align {
            inner = set_child(&inner, "w:jc", &format!("<w:jc w:val=\"{align}\"/>"), PARAGRAPH_ORDER);
        }
        if style.has_run_properties() {
            let mark = crate::properties::read_child(&inner, "w:rPr").unwrap_or_default();
            let mark = format!("<w:rPr>{}</w:rPr>", style_run_properties(mark, style));
            inner = set_child(&inner, "w:rPr", &mark, PARAGRAPH_ORDER);
        }
        inner
    });
    edit_each(&styled, "w:r", |run| style_run(run, style))
}

/// `cell`, a whole `<w:tc>`, with the style's fill and borders, and its paragraph and run
/// properties on every paragraph in it.
pub(crate) fn style_cell(cell: &str, style: &Style) -> String {
    let mut cell = match &style.fill {
        Some(fill) => shade_cell(cell, fill),
        None => cell.to_string(),
    };
    if !style.borders.is_empty() {
        cell = edit_properties(&cell, "w:tcPr", |inner| set_child(inner, "w:tcBorders", &write_borders("w:tcBorders", CELL_SIDES, &style.borders), CELL_ORDER));
    }
    if style.has_paragraph_properties() {
        cell = edit_each(&cell, "w:p", |paragraph| style_paragraph(paragraph, style, false));
    }
    cell
}

/// `row`, a whole `<w:tr>`, with every cell styled.
pub(crate) fn style_row(row: &str, style: &Style) -> String {
    edit_each(row, "w:tc", |cell| style_cell(cell, style))
}
