//! Drops a last page that holds nothing, as a report whose last loop or condition prints empty
//! leaves one.

use crate::xml::{ElementKind, Token, offset_in, read_attribute, tokenize};

// Elements that show something on a page.
const VISIBLE_ELEMENTS: [&str; 5] = ["w:drawing", "w:pict", "w:object", "w:tbl", "w:sym"];

/// `document_xml`, a filled body, without its empty last page: when nothing visible follows the
/// body's last explicit page break, the break and the empty paragraphs after it go. A section
/// break after it starts a page of its own, so it stays.
pub fn drop_empty_last_page(document_xml: &str) -> String {
    try_drop(document_xml).unwrap_or_else(|| document_xml.to_string())
}

fn try_drop(xml: &str) -> Option<String> {
    let tokens = tokenize(xml).ok()?;
    let (break_start, break_end) = tokens.iter().rev().find_map(|token| match *token {
        Token::Element { kind: ElementKind::SelfClosing, name: "w:br", raw } if read_attribute(raw, "w:type") == Some("page") => {
            let start = offset_in(xml, raw);
            Some((start, start + raw.len()))
        }
        _ => None,
    })?;
    let body_end = find_body_end(xml)?;
    if break_start == 0 || body_end < break_end {
        return None;
    }

    let tail = &xml[break_end..body_end];
    if tail.contains("<w:sectPr") || is_visible(tail) {
        return None;
    }

    // The break's own paragraph ends the body; the empty ones after it go.
    let paragraph_end = tail.find("</w:p>")? + "</w:p>".len();
    if !is_only_paragraphs(&tail[paragraph_end..]) {
        return None;
    }
    Some(format!("{}{}{}", &xml[..break_start], &tail[..paragraph_end], &xml[body_end..]))
}

/// Where the body's content ends: its own section properties, else `</w:body>`.
fn find_body_end(xml: &str) -> Option<usize> {
    let body_close = xml.rfind("</w:body>")?;
    let last_section = xml[..body_close].rfind("<w:sectPr");
    let last_paragraph = xml[..body_close].rfind("</w:p>");
    Some(match (last_section, last_paragraph) {
        (Some(section), Some(paragraph)) if section > paragraph => section,
        (Some(section), None) => section,
        _ => body_close,
    })
}

fn is_visible(xml: &str) -> bool {
    let Ok(tokens) = tokenize(xml) else { return true };
    let mut is_in_text = false;
    tokens.iter().any(|token| match *token {
        Token::Element { kind: ElementKind::Open, name: "w:t", .. } => {
            is_in_text = true;
            false
        }
        Token::Element { kind: ElementKind::Close, name: "w:t", .. } => {
            is_in_text = false;
            false
        }
        Token::Element { kind: ElementKind::Open | ElementKind::SelfClosing, name, raw } => VISIBLE_ELEMENTS.contains(&name) || raw == "<w:tab/>",
        Token::Text(text) => is_in_text && !text.trim().is_empty(),
        _ => false,
    })
}

/// Whether `xml` holds only whitespace and whole paragraphs.
fn is_only_paragraphs(xml: &str) -> bool {
    let mut rest = xml.trim_start();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<w:p/>") {
            rest = after.trim_start();
            continue;
        }
        let is_paragraph = rest.starts_with("<w:p>") || (rest.starts_with("<w:p ") && !rest[..rest.find('>').unwrap_or(0)].ends_with('/'));
        if !is_paragraph {
            return false;
        }
        let Some(end) = rest.find("</w:p>") else { return false };
        rest = rest[end + "</w:p>".len()..].trim_start();
    }
    true
}
