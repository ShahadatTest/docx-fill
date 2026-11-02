//! Fills a parsed template with tag values.

use serde_json::Value;

use crate::cell_styling::{read_fill, shade_cell};
use crate::images::{Image, MediaUse, body_or_default, drawing_size, drawing_xml};
use crate::rich_text::{self, RichValue};
use crate::template::{BlockKind, Part, TagKind, Template};
use crate::xml::{ElementKind, escape};

// A `\n` in a value outside a run: the text continues after a line break.
const LINE_BREAK: &str = "</w:t><w:br/><w:t xml:space=\"preserve\">";
const PARAGRAPH: &str = "w:p";
const CELL: &str = "w:tc";

/// A tag a module claims, by its prefix in `FillOptions::module_prefixes`.
pub struct ModuleTag<'a> {
    pub module: usize,
    /// The tag after its prefix.
    pub name: &'a str,
    /// The name's value, looked up as a plain tag's is.
    pub value: Option<&'a Value>,
}

/// What a fill asks of its caller.
pub trait FillHooks {
    /// Escaped XML text printed for a module tag in `part`. Prints nothing by default.
    fn module_tag(&mut self, _part: &str, _tag: ModuleTag) -> String {
        String::new()
    }

    /// The image an image tag in `part` shows for `value`, which is never missing, null, false,
    /// zero or empty text. `None` prints nothing, the default.
    fn image(&mut self, _part: &str, _value: &Value) -> Option<Image> {
        None
    }
}

/// Hooks that print nothing for module and image tags.
pub struct NoHooks;

impl FillHooks for NoHooks {}

/// Shared by the parts of one package fill.
pub(crate) struct FillState {
    pub next_drawing_id: u32,
    /// Page width and height less margins, in pixels, when the part does not give its own.
    pub body: Option<(f64, f64)>,
    /// Images `{%name}` sizes to fill, as a share of their container's width.
    pub container_fill: f64,
}

/// A filled part and the media its drawings show.
pub(crate) struct FilledPart {
    pub xml: String,
    pub media: Vec<MediaUse>,
}

struct Render<'a, 'h> {
    template: &'a Template,
    part: &'a str,
    hooks: &'h mut dyn FillHooks,
    state: &'h mut FillState,
    out: String,
    /// Output offsets of the paragraphs open now, with the rich text marks each holds.
    paragraphs: Vec<(usize, bool)>,
    /// Output offsets of the table cells open now, with the fill each was given.
    cells: Vec<(usize, Option<String>)>,
    rich_values: Vec<RichValue>,
    media: Vec<MediaUse>,
}

impl Template {
    /// The part filled with `data`. Module and image tags print nothing.
    pub fn render(&self, data: &Value) -> String {
        self.render_with(data, &mut NoHooks)
    }

    /// The part filled with `data`, asking `hooks` for module tags and images. An image's media
    /// part and relationship are not added: fill a [`crate::PackageTemplate`] for that.
    pub fn render_with(&self, data: &Value, hooks: &mut dyn FillHooks) -> String {
        let mut state = FillState { next_drawing_id: self.max_drawing_id + 1, body: None, container_fill: crate::CONTAINER_FILL };
        self.fill(data, "", hooks, &mut state).xml
    }

    pub(crate) fn fill(&self, data: &Value, part: &str, hooks: &mut dyn FillHooks, state: &mut FillState) -> FilledPart {
        let mut render = Render {
            template: self,
            part,
            hooks,
            state,
            out: String::with_capacity(self.parts.len() * 16),
            paragraphs: Vec::new(),
            cells: Vec::new(),
            rich_values: Vec::new(),
            media: Vec::new(),
        };
        let mut scopes = vec![data];
        render.render_range(0, self.parts.len(), &self.top, &mut scopes);
        FilledPart { xml: render.out, media: render.media }
    }
}

impl<'a> Render<'a, '_> {
    fn render_range(&mut self, from: usize, to: usize, blocks: &[usize], scopes: &mut Vec<&'a Value>) {
        let template = self.template;
        let mut at = from;
        for &index in blocks {
            let block = &template.blocks[index];
            if block.start < at || block.end > to {
                continue;
            }
            self.render_parts(at, block.start, scopes);
            at = block.end;

            match &block.kind {
                BlockKind::Raw { name } => {
                    if let Some(value) = look_up(scopes, name) {
                        self.out.push_str(&print_value(value));
                    }
                }
                BlockKind::Loop { name, is_inverted: true } => {
                    if !is_shown(look_up(scopes, name)) {
                        self.render_range(block.body_start, block.body_end, &block.children, scopes);
                    }
                }
                BlockKind::Loop { name, is_inverted: false } => {
                    let value = look_up(scopes, name);
                    match value {
                        Some(Value::Array(items)) => {
                            for item in items {
                                scopes.push(item);
                                self.render_range(block.body_start, block.body_end, &block.children, scopes);
                                scopes.pop();
                            }
                        }
                        Some(object @ Value::Object(_)) => {
                            scopes.push(object);
                            self.render_range(block.body_start, block.body_end, &block.children, scopes);
                            scopes.pop();
                        }
                        // Any other shown value prints the body once in the current scope.
                        _ if is_shown(value) => self.render_range(block.body_start, block.body_end, &block.children, scopes),
                        _ => {}
                    }
                }
            }
        }
        self.render_parts(at, to, scopes);
    }

    fn render_parts(&mut self, from: usize, to: usize, scopes: &[&'a Value]) {
        let template = self.template;
        for part in &template.parts[from..to] {
            match part {
                Part::Element { kind, name, raw } => self.print_element(*kind, name, raw),
                Part::Text(text) => self.out.push_str(text),
                Part::Tag(index) => {
                    let tag = &template.tags[*index];
                    let value = || look_up(scopes, &tag.name);
                    match tag.kind {
                        TagKind::Value => {
                            if let Some(value) = value() {
                                self.print_text(&print_value(value), tag.run_props.as_deref());
                            }
                        }
                        TagKind::Module(module) => {
                            let printed = self.hooks.module_tag(self.part, ModuleTag { module, name: &tag.name, value: value() });
                            self.out.push_str(&printed);
                        }
                        TagKind::CellStyle => {
                            if let (Some(fill), Some(cell)) = (read_fill(value()), self.cells.last_mut()) {
                                cell.1 = Some(fill);
                            }
                        }
                        TagKind::RichText { is_block } => {
                            let html = value().and_then(Value::as_str).unwrap_or_default().to_string();
                            self.out.push_str(&rich_text::mark(self.rich_values.len()));
                            self.rich_values.push(RichValue { html, is_block });
                            if let Some(paragraph) = self.paragraphs.last_mut() {
                                paragraph.1 = true;
                            }
                        }
                        TagKind::Image { cell_width } => self.print_image(value(), cell_width),
                        TagKind::Raw | TagKind::Open { .. } | TagKind::Close => {}
                    }
                }
            }
        }
    }

    /// Prints an element, rewriting a paragraph with rich text, or a styled cell, as it closes.
    fn print_element(&mut self, kind: ElementKind, name: &str, raw: &str) {
        let start = self.out.len();
        self.out.push_str(raw);
        match (kind, name) {
            (ElementKind::Open, PARAGRAPH) => self.paragraphs.push((start, false)),
            (ElementKind::Open, CELL) => self.cells.push((start, None)),
            (ElementKind::Close, PARAGRAPH) => {
                if let Some((opened, true)) = self.paragraphs.pop() {
                    let written = rich_text::write_paragraph(&self.out[opened..], &self.rich_values);
                    self.out.replace_range(opened.., &written);
                }
            }
            (ElementKind::Close, CELL) => {
                if let Some((opened, Some(fill))) = self.cells.pop() {
                    let shaded = shade_cell(&self.out[opened..], &fill);
                    self.out.replace_range(opened.., &shaded);
                }
            }
            _ => {}
        }
    }

    fn print_image(&mut self, value: Option<&Value>, cell_width: Option<f64>) {
        let Some(value) = value.filter(|value| is_truthy(value)) else { return };
        let Some(image) = self.hooks.image(self.part, value) else { return };

        let (body_width, body_height) = body_or_default(self.template.body.or(self.state.body));
        let width = cell_width.unwrap_or(body_width) * self.state.container_fill;
        let (width, height) = drawing_size(&image, width, cell_width.is_some(), body_height);
        let media = MediaUse::for_image(image);
        let id = self.state.next_drawing_id;
        self.state.next_drawing_id += 1;

        // The tag sits inside <w:t>; a drawing belongs in the run beside it.
        self.out.push_str("</w:t>");
        self.out.push_str(&drawing_xml(id, &media.relationship_id, width, height));
        self.out.push_str("<w:t xml:space=\"preserve\">");
        if !self.media.iter().any(|used| used.relationship_id == media.relationship_id) {
            self.media.push(media);
        }
    }

    /// Prints a value. With line breaks on, each `\n` ends the tag's run, adds a run holding a
    /// break and goes on in a run with the tag run's properties.
    fn print_text(&mut self, text: &str, run_props: Option<&str>) {
        if !self.template.linebreaks || !text.contains('\n') {
            escape(text, &mut self.out);
            return;
        }
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                match run_props {
                    Some(props) => {
                        self.out.push_str("</w:t></w:r><w:r><w:br/></w:r><w:r>");
                        self.out.push_str(props);
                        self.out.push_str("<w:t xml:space=\"preserve\">");
                    }
                    None => self.out.push_str(LINE_BREAK),
                }
            }
            escape(line.strip_suffix('\r').unwrap_or(line), &mut self.out);
        }
    }
}

/// `name`'s value in the innermost scope that has it, out to the root. A null counts as
/// missing, so an outer scope's value shows through.
fn look_up<'a>(scopes: &[&'a Value], name: &str) -> Option<&'a Value> {
    scopes.iter().rev().find_map(|scope| {
        let value = if name == "." { Some(*scope) } else { scope.get(name) };
        value.filter(|value| !value.is_null())
    })
}

/// Whether a loop over `value` prints: not missing, false, zero, empty text or an empty list.
fn is_shown(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(items)) => !items.is_empty(),
        Some(value) => is_truthy(value),
        None => false,
    }
}

/// JavaScript truthiness: not null, false, zero or empty text.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(is_true) => *is_true,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// `value` as text, as JavaScript's `String(value)` gives it.
pub(crate) fn print_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(is_true) => is_true.to_string(),
        Value::Number(number) => print_number(number),
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(print_value).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn print_number(number: &serde_json::Number) -> String {
    if number.is_i64() || number.is_u64() {
        return number.to_string();
    }
    let float = number.as_f64().unwrap_or_default();
    // Whole floats print without a fraction, as in JavaScript.
    if float.fract() == 0.0 && float.abs() < 1e21 {
        return format!("{float:.0}");
    }
    float.to_string()
}
