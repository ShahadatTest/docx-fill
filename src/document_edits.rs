//! Whole-document edits: read-only protection, page margins and the proofing language. Built from
//! the WordprocessingML spec.

use std::borrow::Cow;

use crate::DOCUMENT_PART;
use crate::cell_styling::find_element_start;
use crate::properties::{RUN_ORDER, SETTINGS_FROM_PROTECTION, edit_each, edit_properties, set_attribute, set_child};
use crate::xml::escape;

const SETTINGS_PART: &str = "word/settings.xml";
const STYLES_PART: &str = "word/styles.xml";
const TWIPS_PER_INCH: f64 = 1440.0;

/// Edits a package fill makes to the whole document. None by default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocumentEdits {
    /// Opens the document read-only in Word, as "Restrict Editing" does, without a password.
    pub read_only: bool,
    /// Every section's page margins.
    pub margins: Option<Margins>,
    /// The default proofing language, such as `en-GB`.
    pub language: Option<String>,
}

/// Page margins in inches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl DocumentEdits {
    pub(crate) fn apply(&self, parts: &mut [(String, Cow<[u8]>)]) {
        if self.read_only {
            edit_part(parts, SETTINGS_PART, protect);
        }
        if let Some(margins) = self.margins {
            edit_part(parts, DOCUMENT_PART, |xml| set_margins(xml, margins));
        }
        if let Some(language) = &self.language {
            edit_part(parts, STYLES_PART, |xml| set_language(xml, language));
        }
    }
}

fn edit_part(parts: &mut [(String, Cow<[u8]>)], name: &str, edit: impl FnOnce(&str) -> String) {
    let Some(part) = parts.iter_mut().find(|(each, _)| each == name) else { return };
    let xml = String::from_utf8_lossy(&part.1).into_owned();
    part.1 = Cow::Owned(edit(&xml).into_bytes());
}

/// `xml`'s root element content rewritten by `edit`.
fn edit_root(xml: &str, root: &str, edit: impl FnOnce(&str) -> String) -> String {
    let Some(start) = find_element_start(xml, &format!("<{root}")) else { return xml.to_string() };
    let Some(open_end) = xml[start..].find('>').map(|at| start + at + 1) else { return xml.to_string() };
    let Some(close) = xml.rfind(&format!("</{root}>")) else { return xml.to_string() };
    format!("{}{}{}", &xml[..open_end], edit(&xml[open_end..close]), &xml[close..])
}

fn protect(xml: &str) -> String {
    let protection = "<w:documentProtection w:edit=\"readOnly\" w:enforcement=\"1\"/>";
    edit_root(xml, "w:settings", |inner| set_child(inner, "w:documentProtection", protection, SETTINGS_FROM_PROTECTION))
}

fn set_margins(xml: &str, margins: Margins) -> String {
    let twips = |inches: f64| ((inches * TWIPS_PER_INCH).round() as i64).to_string();
    edit_each(xml, "w:pgMar", |raw| {
        let raw = set_attribute(raw, "w:top", &twips(margins.top));
        let raw = set_attribute(&raw, "w:right", &twips(margins.right));
        let raw = set_attribute(&raw, "w:bottom", &twips(margins.bottom));
        set_attribute(&raw, "w:left", &twips(margins.left))
    })
}

fn set_language(xml: &str, language: &str) -> String {
    let mut escaped = String::new();
    escape(language, &mut escaped);
    let set = |inner: &str| match find_element_start(inner, "<w:lang") {
        Some(start) => {
            let end = start + inner[start..].find("/>").map_or(0, |at| at + 2);
            format!("{}{}{}", &inner[..start], set_attribute(&inner[start..end], "w:val", &escaped), &inner[end..])
        }
        None => set_child(inner, "w:lang", &format!("<w:lang w:val=\"{escaped}\"/>"), RUN_ORDER),
    };
    if find_element_start(xml, "<w:rPrDefault").is_some() {
        return edit_each(xml, "w:rPrDefault", |defaults| edit_properties(defaults, "w:rPr", set));
    }
    let defaults = format!("<w:rPrDefault><w:rPr><w:lang w:val=\"{escaped}\"/></w:rPr></w:rPrDefault>");
    if find_element_start(xml, "<w:docDefaults").is_some() {
        return edit_each(xml, "w:docDefaults", |element| match element.find('>') {
            Some(at) if !element[..at].ends_with('/') => format!("{}{defaults}{}", &element[..=at], &element[at + 1..]),
            _ => format!("<w:docDefaults>{defaults}</w:docDefaults>"),
        });
    }
    edit_root(xml, "w:styles", |inner| format!("<w:docDefaults>{defaults}</w:docDefaults>{inner}"))
}
