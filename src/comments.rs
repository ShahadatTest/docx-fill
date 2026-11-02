//! Comments a value such as `{ "text": "John", "comment": "Check" }` adds around its text. Built
//! from the WordprocessingML spec: comments live in `word/comments.xml`, and the body marks each
//! one's range and reference.

use std::borrow::Cow;

use serde_json::Value;

use crate::footnotes::NotesPart;
use crate::render::print_value;
use crate::xml::escape;

pub(crate) const COMMENTS_PART: &str = "word/comments.xml";

const COMMENTS: NotesPart = NotesPart {
    name: COMMENTS_PART,
    root: "w:comments",
    relationship: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments",
    relationship_id: "rIdDocxFillComments",
    content_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml",
    start: "",
};

/// The author and date of a comment whose value gives none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDefaults {
    pub author: String,
    pub initials: Option<String>,
    /// ISO 8601, such as `2026-11-02T09:00:00Z`. Comments carry no date when `None`.
    pub date: Option<String>,
}

impl Default for CommentDefaults {
    fn default() -> Self {
        CommentDefaults { author: "Author".to_string(), initials: None, date: None }
    }
}

/// A comment to write in the comments part.
#[derive(Debug, Clone)]
pub(crate) struct Comment {
    pub id: u32,
    pub text: String,
    pub author: String,
    pub initials: Option<String>,
    pub date: Option<String>,
}

/// The text and comments of a value with `text` and `comment` or `comments`, the value's author,
/// initials and date over the defaults. The comments' ids are not set yet.
pub(crate) fn read_commented(value: &Value, defaults: &CommentDefaults) -> Option<(String, Vec<Comment>)> {
    let object = value.as_object()?;
    let text = object.get("text").map(print_value)?;
    let read = |entry: &serde_json::Map<String, Value>| {
        let comment = entry.get("comment").filter(|comment| !comment.is_null()).map(print_value)?;
        let field = |name: &str| entry.get(name).or_else(|| object.get(name)).and_then(Value::as_str).map(str::to_string);
        Some(Comment {
            id: 0,
            text: comment,
            author: field("author").unwrap_or_else(|| defaults.author.clone()),
            initials: field("initials").or_else(|| defaults.initials.clone()),
            date: field("date").or_else(|| defaults.date.clone()),
        })
    };
    let comments: Vec<_> = match object.get("comments") {
        Some(Value::Array(entries)) => entries.iter().filter_map(Value::as_object).filter_map(read).collect(),
        _ => read(object).into_iter().collect(),
    };
    (!comments.is_empty()).then_some((text, comments))
}

/// The largest comment id in a comments part, at least 0.
pub(crate) fn read_max_id(xml: &str) -> u32 {
    crate::footnotes::read_max_element_id(xml, "w:comment")
}

/// Adds each comment to the comments part, creating it when the package has none.
pub(crate) fn add_comments(parts: &mut Vec<(String, Cow<[u8]>)>, comments: &[Comment]) {
    if comments.is_empty() {
        return;
    }
    let mut written = String::new();
    for comment in comments {
        written.push_str(&format!("<w:comment w:id=\"{}\" w:author=\"", comment.id));
        escape(&comment.author, &mut written);
        written.push('"');
        for (name, value) in [("w:initials", &comment.initials), ("w:date", &comment.date)] {
            if let Some(value) = value {
                written.push_str(&format!(" {name}=\""));
                escape(value, &mut written);
                written.push('"');
            }
        }
        written.push('>');
        for (index, line) in comment.text.split('\n').enumerate() {
            written.push_str("<w:p><w:pPr><w:pStyle w:val=\"CommentText\"/></w:pPr>");
            if index == 0 {
                written.push_str("<w:r><w:rPr><w:rStyle w:val=\"CommentReference\"/></w:rPr><w:annotationRef/></w:r>");
            }
            written.push_str("<w:r><w:t xml:space=\"preserve\">");
            escape(line, &mut written);
            written.push_str("</w:t></w:r></w:p>");
        }
        written.push_str("</w:comment>");
    }
    COMMENTS.append(parts, &written);
}
