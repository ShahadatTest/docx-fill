//! Fills DOCX templates written in docxtemplater's public tag syntax, with `{ }` delimiters:
//!
//! - `{name}` prints a value; a missing or null value prints nothing.
//! - `{#name}…{/name}` repeats once per item of a list, once for an object or any other shown
//!   value, and not at all for a missing, false, zero, empty or null one. `{^name}…{/name}` prints
//!   only when `{#name}` would not.
//! - `{-w:tr name}…{/name}` repeats whole elements (`w:tr`, `w:tc`, `w:p`, …).
//! - `{@name}` replaces its paragraph with the value as raw XML.
//!
//! Names are looked up whole (`table:abc.value` is one key), from the innermost loop item out to
//! the root. Tags are found in each paragraph's `<w:t>` text joined, since Word splits a tag
//! across runs; a tag's value prints in the run its `{` is in, keeping that run's formatting.
//!
//! Built from the WordprocessingML spec and docxtemplater's documented behaviour.

mod cell_styling;
mod empty_last_page;
mod images;
mod render;
mod rich_text;
mod template;
mod xml;

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;

use serde_json::Value;

pub use empty_last_page::drop_empty_last_page;
pub use images::Image;
pub use render::{FillHooks, ModuleTag, NoHooks};
pub use template::Template;

use images::MediaUse;
use render::FillState;

const DOCUMENT_PART: &str = "word/document.xml";
const CONTENT_TYPES_PART: &str = "[Content_Types].xml";
const IMAGE_RELATIONSHIP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const RELATIONSHIPS_NAMESPACE: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
// Images fill their container less 5% for cell padding.
pub(crate) const CONTAINER_FILL: f64 = 0.95;

/// The strings around a tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delimiters {
    pub start: String,
    pub end: String,
}

impl Default for Delimiters {
    fn default() -> Self {
        Delimiters { start: "{".to_string(), end: "}".to_string() }
    }
}

/// Tag kinds beyond the core syntax, each on or off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extensions {
    /// `{:vt#cols}` … `{:vt/cols}`: repeats table cells, as `{-w:tc cols}` … `{/cols}` does.
    pub vertical_tables: bool,
    /// `{:stylecell name}`: a value of `{ "cellBackground": "#rrggbb" }` fills the tag's cell.
    pub cell_styling: bool,
    /// `{~~name}` and `{~name}`: HTML printed as Word paragraphs and runs.
    pub rich_text: bool,
    /// `{%name}` and `{%%name}`: an image from [`FillHooks::image`].
    pub images: bool,
}

impl Extensions {
    pub const ALL: Extensions = Extensions { vertical_tables: true, cell_styling: true, rich_text: true, images: true };
    pub const NONE: Extensions = Extensions { vertical_tables: false, cell_styling: false, rich_text: false, images: false };
}

/// How a template is read and filled.
#[derive(Debug, Clone)]
pub struct FillOptions {
    /// A loop whose tags are each alone in their own paragraph repeats the paragraphs between
    /// them, and its tags' paragraphs go.
    pub paragraph_loop: bool,
    /// A `\n` in a value prints as a line break.
    pub linebreaks: bool,
    pub delimiters: Delimiters,
    pub extensions: Extensions,
    /// When filling a package, drops an empty last page from the body (see
    /// [`drop_empty_last_page`]).
    pub drop_empty_last_page: bool,
    /// Tag prefixes handed to [`FillHooks::module_tag`] instead of printed. They are checked
    /// before the extensions'.
    pub module_prefixes: Vec<String>,
}

impl Default for FillOptions {
    /// Paragraph loops, line breaks, every extension and empty-last-page dropping on, with `{ }`
    /// delimiters.
    fn default() -> Self {
        FillOptions {
            paragraph_loop: true,
            linebreaks: true,
            delimiters: Delimiters::default(),
            extensions: Extensions::ALL,
            drop_empty_last_page: true,
            module_prefixes: Vec::new(),
        }
    }
}

impl FillOptions {
    /// Core tags only: no extensions, and the last page kept.
    pub fn core() -> FillOptions {
        FillOptions { extensions: Extensions::NONE, drop_empty_last_page: false, ..FillOptions::default() }
    }
}

/// Why a template could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillError {
    /// The package part, when known.
    pub part: Option<String>,
    pub message: String,
}

impl FillError {
    pub(crate) fn new(message: impl Into<String>) -> FillError {
        FillError { part: None, message: message.into() }
    }
}

impl fmt::Display for FillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.part {
            Some(part) => write!(f, "{part}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for FillError {}

/// Whether a package part holds tags: the body, a header or a footer.
pub fn is_template_part(name: &str) -> bool {
    if name == DOCUMENT_PART {
        return true;
    }
    let Some(file) = name.strip_prefix("word/").and_then(|file| file.strip_suffix(".xml")) else { return false };
    let Some(number) = file.strip_prefix("header").or_else(|| file.strip_prefix("footer")) else { return false };
    number.bytes().all(|byte| byte.is_ascii_digit())
}

/// Fills one part's XML.
pub fn fill_part(xml: &str, data: &Value, options: &FillOptions) -> Result<String, FillError> {
    Ok(Template::parse(xml, options)?.render(data))
}

/// A parsed DOCX package: its template parts parsed, every other part kept as it is.
#[derive(Debug)]
pub struct PackageTemplate {
    parts: Vec<(String, PackagePart)>,
    drop_empty_last_page: bool,
    max_drawing_id: u32,
    body: Option<(f64, f64)>,
}

#[derive(Debug)]
enum PackagePart {
    Template(Template),
    Bytes(Vec<u8>),
}

impl PackageTemplate {
    /// Parses the body, headers and footers among `parts`, given by name without a leading `/`.
    pub fn parse(parts: impl IntoIterator<Item = (String, Vec<u8>)>, options: &FillOptions) -> Result<PackageTemplate, FillError> {
        let mut parsed = Vec::new();
        for (name, bytes) in parts {
            if !is_template_part(&name) {
                parsed.push((name, PackagePart::Bytes(bytes)));
                continue;
            }

            let in_part = |mut error: FillError| {
                error.part = Some(name.clone());
                error
            };
            let xml = String::from_utf8(bytes).map_err(|_| in_part(FillError::new("The part is not UTF-8")))?;
            let template = Template::parse(&xml, options).map_err(in_part)?;
            parsed.push((name, PackagePart::Template(template)));
        }

        let templates = || parsed.iter().filter_map(|(name, part)| match part {
            PackagePart::Template(template) => Some((name, template)),
            PackagePart::Bytes(_) => None,
        });
        let max_drawing_id = templates().map(|(_, template)| template.max_drawing_id).max().unwrap_or(0);
        let body = templates().find(|(name, _)| *name == DOCUMENT_PART).and_then(|(_, template)| template.body);
        Ok(PackageTemplate { drop_empty_last_page: options.drop_empty_last_page, max_drawing_id, body, parts: parsed })
    }

    /// Every part, in the order given, with the template parts filled with `data`.
    pub fn render(&self, data: &Value) -> Vec<(String, Cow<'_, [u8]>)> {
        self.render_with(data, &mut NoHooks)
    }

    /// [`PackageTemplate::render`], asking `hooks` for module tags and images. Images add their
    /// media parts, relationships and content types.
    pub fn render_with(&self, data: &Value, hooks: &mut dyn FillHooks) -> Vec<(String, Cow<'_, [u8]>)> {
        let mut state = FillState { next_drawing_id: self.max_drawing_id + 1, body: self.body, container_fill: CONTAINER_FILL };
        let mut out: Vec<(String, Cow<[u8]>)> = Vec::with_capacity(self.parts.len());
        let mut media: Vec<(String, MediaUse)> = Vec::new();
        for (name, part) in &self.parts {
            let bytes = match part {
                PackagePart::Bytes(bytes) => Cow::Borrowed(bytes.as_slice()),
                PackagePart::Template(template) => {
                    let filled = template.fill(data, name, hooks, &mut state);
                    media.extend(filled.media.into_iter().map(|used| (name.clone(), used)));
                    let xml = if self.drop_empty_last_page && name == DOCUMENT_PART { drop_empty_last_page(&filled.xml) } else { filled.xml };
                    Cow::Owned(xml.into_bytes())
                }
            };
            out.push((name.clone(), bytes));
        }
        add_media(&mut out, &media);
        out
    }
}

/// Adds each image's media part, the relationship its part shows it by, and its content type.
fn add_media(parts: &mut Vec<(String, Cow<[u8]>)>, media: &[(String, MediaUse)]) {
    let mut written = BTreeSet::new();
    for (part, used) in media {
        let folder = part.rfind('/').map_or("", |at| &part[..at + 1]);
        let path = format!("{folder}{}", used.target);
        if written.insert(path.clone()) {
            parts.push((path, Cow::Owned(used.image.bytes.to_vec())));
        }

        let file = &part[folder.len()..];
        let rels_path = format!("{folder}_rels/{file}.rels");
        let relationship = format!("<Relationship Id=\"{}\" Type=\"{IMAGE_RELATIONSHIP}\" Target=\"{}\"/>", used.relationship_id, used.target);
        let rels = match parts.iter().position(|(name, _)| *name == rels_path) {
            Some(index) => &mut parts[index].1,
            None => {
                let empty = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{RELATIONSHIPS_NAMESPACE}\"></Relationships>");
                parts.push((rels_path, Cow::Owned(empty.into_bytes())));
                &mut parts.last_mut().expect("just pushed").1
            }
        };
        let xml = String::from_utf8_lossy(rels).into_owned();
        if !xml.contains(&format!("Id=\"{}\"", used.relationship_id)) {
            *rels = Cow::Owned(insert_before_last(&xml, "</Relationships>", &relationship).into_bytes());
        }

        let Some(types) = parts.iter_mut().find(|(name, _)| name == CONTENT_TYPES_PART) else { continue };
        let xml = String::from_utf8_lossy(&types.1).into_owned();
        let extension = used.image.extension.to_ascii_lowercase();
        if !xml.to_ascii_lowercase().contains(&format!("extension=\"{extension}\"")) {
            let default = format!("<Default Extension=\"{extension}\" ContentType=\"{}\"/>", image_content_type(&extension));
            types.1 = Cow::Owned(insert_before_last(&xml, "</Types>", &default).into_bytes());
        }
    }
}

fn insert_before_last(xml: &str, close: &str, added: &str) -> String {
    match xml.rfind(close) {
        Some(at) => format!("{}{added}{}", &xml[..at], &xml[at..]),
        None => xml.to_string(),
    }
}

fn image_content_type(extension: &str) -> String {
    match extension {
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "tif" | "tiff" => "image/tiff".to_string(),
        other => format!("image/{other}"),
    }
}
