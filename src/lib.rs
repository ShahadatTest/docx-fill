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

mod render;
mod template;
mod xml;

use std::borrow::Cow;
use std::fmt;

use serde_json::Value;

pub use render::ModuleTag;
pub use template::Template;

const DOCUMENT_PART: &str = "word/document.xml";

/// How a template is read and filled.
#[derive(Debug, Clone)]
pub struct FillOptions {
    /// A loop whose tags are each alone in their own paragraph repeats the paragraphs between
    /// them, and its tags' paragraphs go.
    pub paragraph_loop: bool,
    /// A `\n` in a value prints as a line break.
    pub linebreaks: bool,
    /// Tag prefixes handed to a module instead of printed, such as `~~` for `{~~notes}`.
    pub module_prefixes: Vec<String>,
}

impl Default for FillOptions {
    /// The live preview's options: paragraph loops and line breaks on, no modules.
    fn default() -> Self {
        FillOptions { paragraph_loop: true, linebreaks: true, module_prefixes: Vec::new() }
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
        Ok(PackageTemplate { parts: parsed })
    }

    /// Every part, in the order given, with the template parts filled with `data`.
    pub fn render(&self, data: &Value) -> Vec<(&str, Cow<'_, [u8]>)> {
        self.render_with(data, &mut |_, _| String::new())
    }

    /// [`PackageTemplate::render`], with `module` printing each module tag as `Template::render_with`
    /// describes. The part name comes first.
    pub fn render_with(&self, data: &Value, module: &mut dyn FnMut(&str, ModuleTag) -> String) -> Vec<(&str, Cow<'_, [u8]>)> {
        self.parts
            .iter()
            .map(|(name, part)| {
                let bytes = match part {
                    PackagePart::Bytes(bytes) => Cow::Borrowed(bytes.as_slice()),
                    PackagePart::Template(template) => Cow::Owned(template.render_with(data, &mut |tag| module(name, tag)).into_bytes()),
                };
                (name.as_str(), bytes)
            })
            .collect()
    }
}
