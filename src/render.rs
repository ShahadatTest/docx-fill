//! Fills a parsed template with tag values.

use serde_json::Value;

use crate::template::{BlockKind, Part, TagKind, Template};
use crate::xml::escape;

// A `\n` in a value: the text run continues after a line break.
const LINE_BREAK: &str = "</w:t><w:br/><w:t xml:space=\"preserve\">";

/// A tag a module claims, by its prefix in `FillOptions::module_prefixes`.
pub struct ModuleTag<'a> {
    pub module: usize,
    /// The tag after its prefix.
    pub name: &'a str,
    /// The name's value, looked up as a plain tag's is.
    pub value: Option<&'a Value>,
}

impl Template {
    /// The part filled with `data`. Module tags print nothing.
    pub fn render(&self, data: &Value) -> String {
        self.render_with(data, &mut |_| String::new())
    }

    /// The part filled with `data`; `module` prints each module tag, as escaped XML text.
    pub fn render_with(&self, data: &Value, module: &mut dyn FnMut(ModuleTag) -> String) -> String {
        let mut out = String::with_capacity(self.parts.len() * 16);
        let mut scopes = vec![data];
        self.render_range(0, self.parts.len(), &self.top, &mut scopes, &mut out, module);
        out
    }

    fn render_range<'a>(&'a self, from: usize, to: usize, blocks: &[usize], scopes: &mut Vec<&'a Value>, out: &mut String, module: &mut dyn FnMut(ModuleTag) -> String) {
        let mut at = from;
        for &index in blocks {
            let block = &self.blocks[index];
            if block.start < at || block.end > to {
                continue;
            }
            self.render_parts(at, block.start, scopes, out, module);
            at = block.end;

            match &block.kind {
                BlockKind::Raw { name } => {
                    if let Some(value) = look_up(scopes, name) {
                        out.push_str(&print_value(value));
                    }
                }
                BlockKind::Loop { name, is_inverted: true } => {
                    if !is_shown(look_up(scopes, name)) {
                        self.render_range(block.body_start, block.body_end, &block.children, scopes, out, module);
                    }
                }
                BlockKind::Loop { name, is_inverted: false } => {
                    let value = look_up(scopes, name);
                    match value {
                        Some(Value::Array(items)) => {
                            for item in items {
                                scopes.push(item);
                                self.render_range(block.body_start, block.body_end, &block.children, scopes, out, module);
                                scopes.pop();
                            }
                        }
                        Some(object @ Value::Object(_)) => {
                            scopes.push(object);
                            self.render_range(block.body_start, block.body_end, &block.children, scopes, out, module);
                            scopes.pop();
                        }
                        // Any other shown value prints the body once in the current scope.
                        _ if is_shown(value) => self.render_range(block.body_start, block.body_end, &block.children, scopes, out, module),
                        _ => {}
                    }
                }
            }
        }
        self.render_parts(at, to, scopes, out, module);
    }

    fn render_parts<'a>(&'a self, from: usize, to: usize, scopes: &[&'a Value], out: &mut String, module: &mut dyn FnMut(ModuleTag) -> String) {
        for part in &self.parts[from..to] {
            match part {
                Part::Element { raw, .. } => out.push_str(raw),
                Part::Text(text) => out.push_str(text),
                Part::Tag(index) => {
                    let tag = &self.tags[*index];
                    match tag.kind {
                        TagKind::Value => {
                            if let Some(value) = look_up(scopes, &tag.name) {
                                self.print_text(&print_value(value), out);
                            }
                        }
                        TagKind::Module(prefix) => out.push_str(&module(ModuleTag { module: prefix, name: &tag.name, value: look_up(scopes, &tag.name) })),
                        TagKind::Raw | TagKind::Open { .. } | TagKind::Close => {}
                    }
                }
            }
        }
    }

    fn print_text(&self, text: &str, out: &mut String) {
        if !self.linebreaks || !text.contains('\n') {
            escape(text, out);
            return;
        }
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                out.push_str(LINE_BREAK);
            }
            escape(line.strip_suffix('\r').unwrap_or(line), out);
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
        None | Some(Value::Null) => false,
        Some(Value::Bool(is_true)) => *is_true,
        Some(Value::Number(number)) => number.as_f64().is_some_and(|n| n != 0.0),
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(_)) => true,
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
