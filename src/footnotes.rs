//! The footnotes `{:footnote name}` tags add. Built from the WordprocessingML spec: footnotes live
//! in `word/footnotes.xml`, which the body references by relationship.

use std::borrow::Cow;

use crate::xml::{escape, read_attribute};
use crate::{CONTENT_TYPES_PART, insert_before_last};

pub(crate) const FOOTNOTES_PART: &str = "word/footnotes.xml";
const DOCUMENT_RELATIONSHIPS: &str = "word/_rels/document.xml.rels";
const FOOTNOTES_RELATIONSHIP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
const FOOTNOTES_CONTENT_TYPE: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const MAIN_NAMESPACE: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
// Word's separator footnotes, which a new footnotes part needs before any note.
const SEPARATORS: &str = "<w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>\
<w:footnote w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>";

/// The largest footnote id in a footnotes part, at least 0.
pub(crate) fn read_max_id(xml: &str) -> u32 {
    let mut max = 0;
    let mut rest = xml;
    while let Some(at) = rest.find("<w:footnote ") {
        rest = &rest[at + 1..];
        let raw = &rest[..rest.find('>').unwrap_or(rest.len())];
        if let Some(id) = read_attribute(raw, "w:id").and_then(|id| id.parse::<u32>().ok()) {
            max = max.max(id);
        }
    }
    max
}

/// Adds each footnote to the footnotes part, creating the part, its relationship and content
/// type when the package has none.
pub(crate) fn add_footnotes(parts: &mut Vec<(String, Cow<[u8]>)>, notes: &[(u32, String)]) {
    if notes.is_empty() {
        return;
    }
    let mut written = String::new();
    for (id, text) in notes {
        written.push_str(&format!(
            "<w:footnote w:id=\"{id}\"><w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr><w:r><w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr><w:footnoteRef/></w:r><w:r><w:t xml:space=\"preserve\"> "
        ));
        escape(text, &mut written);
        written.push_str("</w:t></w:r></w:p></w:footnote>");
    }
    append_notes(parts, &written);
}

/// Adds `written`, whole `<w:footnote>` elements, to the footnotes part, creating the part, its
/// relationship and content type when the package has none.
pub(crate) fn append_notes(parts: &mut Vec<(String, Cow<[u8]>)>, written: &str) {
    if let Some(part) = parts.iter_mut().find(|(name, _)| name == FOOTNOTES_PART) {
        let xml = String::from_utf8_lossy(&part.1).into_owned();
        part.1 = Cow::Owned(insert_before_last(&xml, "</w:footnotes>", written).into_bytes());
        return;
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:footnotes xmlns:w=\"{MAIN_NAMESPACE}\">{SEPARATORS}{written}</w:footnotes>"
    );
    parts.push((FOOTNOTES_PART.to_string(), Cow::Owned(xml.into_bytes())));

    if let Some(rels) = parts.iter_mut().find(|(name, _)| name == DOCUMENT_RELATIONSHIPS) {
        let xml = String::from_utf8_lossy(&rels.1).into_owned();
        if !xml.contains(FOOTNOTES_RELATIONSHIP) {
            let relationship = format!("<Relationship Id=\"rIdDocxFillFootnotes\" Type=\"{FOOTNOTES_RELATIONSHIP}\" Target=\"footnotes.xml\"/>");
            rels.1 = Cow::Owned(insert_before_last(&xml, "</Relationships>", &relationship).into_bytes());
        }
    }
    if let Some(types) = parts.iter_mut().find(|(name, _)| name == CONTENT_TYPES_PART) {
        let xml = String::from_utf8_lossy(&types.1).into_owned();
        if !xml.contains("PartName=\"/word/footnotes.xml\"") {
            let override_ = format!("<Override PartName=\"/word/footnotes.xml\" ContentType=\"{FOOTNOTES_CONTENT_TYPE}\"/>");
            types.1 = Cow::Owned(insert_before_last(&xml, "</Types>", &override_).into_bytes());
        }
    }
}
