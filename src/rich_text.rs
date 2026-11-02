//! `{~~name}` and `{~name}`: HTML printed as Word paragraphs and runs. Built from the
//! WordprocessingML spec and the HTML a sanitiser lets through: p, br, h1-h6, strong/b, em/i,
//! u, ul, ol and li, without attributes.
//!
//! A block tag alone in its paragraph prints each HTML paragraph, heading or list item as a Word
//! paragraph with the tag paragraph's properties and the tag run's font; bold, italic and
//! underline carry over. A tag sharing its paragraph with other text, or an inline tag, prints
//! inline, paragraphs joined by line breaks. An empty value leaves the tag's paragraph empty.

use crate::cell_styling::{find_element_start, remove_first_element};

// Stands in for a value in the filled XML until its paragraph closes.
pub(crate) const MARK_START: char = '\u{E000}';
pub(crate) const MARK_END: char = '\u{E001}';

const LIST_INDENT_TWIPS: u32 = 360;
const BULLET: &str = "\u{2022}";
// Half-points, h1 to h6.
const HEADING_SIZES: [u32; 6] = [32, 28, 26, 24, 24, 24];

pub(crate) struct RichValue {
    pub html: String,
    pub is_block: bool,
}

#[derive(Debug, Clone, Default)]
struct Run {
    text: Option<String>,
    is_break: bool,
    is_bold: bool,
    is_italic: bool,
    is_underlined: bool,
}

#[derive(Debug, Clone, Default)]
struct Block {
    runs: Vec<Run>,
    heading_level: Option<usize>,
    list: Option<(usize, String)>,
}

pub(crate) fn mark(index: usize) -> String {
    format!("{MARK_START}{index}{MARK_END}")
}

/// A filled paragraph holding rich text marks, rewritten with the values' runs and paragraphs.
pub(crate) fn write_paragraph(paragraph: &str, values: &[RichValue]) -> String {
    let Some((mark, index)) = find_mark(paragraph) else { return paragraph.to_string() };
    let value = &values[index];
    let other_text = read_text(paragraph).replacen(&mark, "", 1);
    let run = find_run(paragraph, &mark);
    let run_props = run.and_then(|run| find_between(run, "<w:rPr>", "</w:rPr>")).unwrap_or("");
    let blocks = read_html(&value.html);

    // Alone in its paragraph: the paragraph becomes the value's paragraphs.
    if value.is_block && other_text.trim().is_empty() && !blocks.is_empty() && run.is_some() {
        let open = &paragraph[..paragraph.find('>').map_or(0, |at| at + 1)];
        let paragraph_props = find_between(paragraph, "<w:pPr>", "</w:pPr>").unwrap_or("");
        let bookmarks = find_bookmarks(paragraph);
        return blocks
            .iter()
            .enumerate()
            .map(|(index, block)| {
                let bookmarks = if index == 0 { bookmarks.as_str() } else { "" };
                format!("{open}{}{bookmarks}{}</w:p>", write_paragraph_props(paragraph_props, block), write_runs(block, run_props))
            })
            .collect();
    }

    // Beside other text: the runs go where the tag was, joined by breaks.
    let inline = blocks.iter().map(|block| write_runs(block, run_props)).collect::<Vec<_>>().join(&format!("<w:r>{run_props}<w:br/></w:r>"));
    let replaced = match run {
        Some(run) => paragraph.replacen(run, &split_run(run, &mark, &inline), 1),
        None => paragraph.replacen(&mark, "", 1),
    };
    write_paragraph(&replaced, values)
}

fn find_mark(xml: &str) -> Option<(String, usize)> {
    let start = xml.find(MARK_START)?;
    let length = xml[start..].find(MARK_END)?;
    let index = xml[start + MARK_START.len_utf8()..start + length].parse().ok()?;
    Some((xml[start..start + length + MARK_END.len_utf8()].to_string(), index))
}

/// The run whose text holds `mark`.
fn find_run<'a>(paragraph: &'a str, mark: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(at) = find_element_start(&paragraph[from..], "<w:r") {
        let start = from + at;
        let end = start + paragraph[start..].find("</w:r>")? + "</w:r>".len();
        let run = &paragraph[start..end];
        if run.contains(mark) {
            return Some(run);
        }
        from = end;
    }
    None
}

/// `run` with `mark` replaced by `runs`, its text either side kept.
fn split_run(run: &str, mark: &str, runs: &str) -> String {
    let open = &run[..run.find('>').map_or(0, |at| at + 1)];
    let props = find_between(run, "<w:rPr>", "</w:rPr>").unwrap_or("");
    let text = read_text(run);
    let mut pieces = text.split(mark);
    let (before, after) = (pieces.next().unwrap_or(""), pieces.next().unwrap_or(""));
    let text_run = |text: &str| if text.is_empty() { String::new() } else { format!("{open}{props}<w:t xml:space=\"preserve\">{text}</w:t></w:r>") };
    format!("{}{runs}{}", text_run(before), text_run(after))
}

/// The joined `<w:t>` text of `xml`, as written.
fn read_text(xml: &str) -> String {
    let mut text = String::new();
    let mut from = 0;
    while let Some(at) = find_element_start(&xml[from..], "<w:t") {
        let start = from + at;
        let Some(open_end) = xml[start..].find('>').map(|end| start + end) else { break };
        from = open_end + 1;
        if xml[..open_end].ends_with('/') {
            continue;
        }
        let Some(close) = xml[from..].find("</w:t>") else { break };
        text.push_str(&xml[from..from + close]);
        from += close;
    }
    text
}

/// The first `open…close` element, whole.
fn find_between<'a>(xml: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = xml.find(open)?;
    let end = start + xml[start..].find(close)? + close.len();
    Some(&xml[start..end])
}

/// Every self-closing `<w:bookmarkStart>` and `<w:bookmarkEnd>`, in order.
fn find_bookmarks(paragraph: &str) -> String {
    let mut found = String::new();
    let mut from = 0;
    while let Some(at) = paragraph[from..].find("<w:bookmark") {
        let start = from + at;
        let Some(end) = paragraph[start..].find('>').map(|end| start + end + 1) else { break };
        let element = &paragraph[start..end];
        if element.ends_with("/>") && (element.starts_with("<w:bookmarkStart") || element.starts_with("<w:bookmarkEnd")) {
            found.push_str(element);
        }
        from = end;
    }
    found
}

/// The HTML's paragraphs, headings and list items, with their runs.
fn read_html(html: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut lists: Vec<(bool, usize)> = Vec::new();
    let (mut bold, mut italic, mut underline) = (0i32, 0i32, 0i32);
    let mut current: Option<usize> = None;

    let add_run = |blocks: &mut Vec<Block>, current: &mut Option<usize>, text: Option<String>, is_break: bool, format: (i32, i32, i32)| {
        let index = *current.get_or_insert_with(|| {
            blocks.push(Block::default());
            blocks.len() - 1
        });
        blocks[index].runs.push(Run { text, is_break, is_bold: format.0 > 0, is_italic: format.1 > 0, is_underlined: format.2 > 0 });
    };

    for token in read_html_tokens(html) {
        let (is_closing, tag) = match token {
            HtmlToken::Text(text) => {
                let decoded = decode_entities(text);
                if current.is_some() || !decoded.trim().is_empty() {
                    add_run(&mut blocks, &mut current, Some(decoded), false, (bold, italic, underline));
                }
                continue;
            }
            HtmlToken::Tag { is_closing, name } => (is_closing, name.to_ascii_lowercase()),
        };
        let step = if is_closing { -1 } else { 1 };
        let heading = tag.strip_prefix('h').and_then(|level| level.parse::<usize>().ok()).filter(|level| (1..=6).contains(level));
        let is_in_list = current.is_some_and(|index| blocks[index].list.is_some());

        match tag.as_str() {
            "strong" | "b" => bold += step,
            "em" | "i" => italic += step,
            "u" => underline += step,
            // A <br> between paragraphs is a blank line.
            "br" if current.is_some() => add_run(&mut blocks, &mut current, None, true, (bold, italic, underline)),
            "br" => {
                blocks.push(Block::default());
                current = None;
            }
            "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" if !is_in_list => {
                if is_closing {
                    current = None;
                } else {
                    blocks.push(Block { heading_level: heading, ..Block::default() });
                    current = Some(blocks.len() - 1);
                }
            }
            "ul" | "ol" => {
                current = None;
                if is_closing {
                    lists.pop();
                } else {
                    lists.push((tag == "ol", 0));
                }
            }
            "li" => match lists.last_mut() {
                Some((is_ordered, count)) if !is_closing => {
                    *count += 1;
                    let marker = if *is_ordered { format!("{count}.") } else { BULLET.to_string() };
                    blocks.push(Block { list: Some((lists.len() - 1, marker)), ..Block::default() });
                    current = Some(blocks.len() - 1);
                }
                _ => current = None,
            },
            _ => {}
        }
    }
    blocks
}

enum HtmlToken<'a> {
    Tag { is_closing: bool, name: &'a str },
    Text(&'a str),
}

/// `<name>`, `</name>`, `<name/>` and the text between. A `<` starting anything else is skipped.
fn read_html_tokens(html: &str) -> Vec<HtmlToken<'_>> {
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < html.len() {
        let rest = &html[at..];
        if !rest.starts_with('<') {
            let end = rest.find('<').unwrap_or(rest.len());
            tokens.push(HtmlToken::Text(&rest[..end]));
            at += end;
            continue;
        }
        match read_html_tag(rest) {
            Some((token, length)) => {
                tokens.push(token);
                at += length;
            }
            None => at += 1,
        }
    }
    tokens
}

fn read_html_tag(text: &str) -> Option<(HtmlToken<'_>, usize)> {
    let is_closing = text[1..].starts_with('/');
    let name_start = if is_closing { 2 } else { 1 };
    let name_length = text[name_start..].bytes().take_while(u8::is_ascii_alphanumeric).count();
    if name_length == 0 {
        return None;
    }
    let mut at = name_start + name_length;
    at += text[at..].bytes().take_while(u8::is_ascii_whitespace).count();
    if text[at..].starts_with('/') {
        at += 1;
    }
    if !text[at..].starts_with('>') {
        return None;
    }
    Some((HtmlToken::Tag { is_closing, name: &text[name_start..name_start + name_length] }, at + 1))
}

fn write_paragraph_props(paragraph_props: &str, block: &Block) -> String {
    let Some((level, _)) = &block.list else { return paragraph_props.to_string() };
    let indent = format!("<w:ind w:left=\"{}\" w:hanging=\"{LIST_INDENT_TWIPS}\"/>", LIST_INDENT_TWIPS * (*level as u32 + 1));
    let inner = paragraph_props.strip_prefix("<w:pPr>").and_then(|props| props.strip_suffix("</w:pPr>")).unwrap_or("");
    let inner = remove_first_element(inner, "<w:ind");
    let inner = match find_between(&inner, "<w:numPr>", "</w:numPr>") {
        Some(numbering) => inner.replacen(numbering, "", 1),
        None => inner,
    };
    format!("<w:pPr>{inner}{indent}</w:pPr>")
}

fn write_runs(block: &Block, run_props: &str) -> String {
    let size = block.heading_level.map(|level| HEADING_SIZES[level - 1]);
    let mut out = match &block.list {
        Some((_, marker)) => format!("<w:r>{run_props}<w:t>{marker}</w:t></w:r><w:r>{run_props}<w:tab/></w:r>"),
        None => String::new(),
    };
    for run in &block.runs {
        let props = write_run_props(run_props, run.is_bold || block.heading_level.is_some(), run.is_italic, run.is_underlined, size);
        out.push_str("<w:r>");
        out.push_str(&props);
        if run.is_break {
            out.push_str("<w:br/>");
        } else {
            out.push_str("<w:t xml:space=\"preserve\">");
            escape_text(run.text.as_deref().unwrap_or(""), &mut out);
            out.push_str("</w:t>");
        }
        out.push_str("</w:r>");
    }
    out
}

fn write_run_props(run_props: &str, is_bold: bool, is_italic: bool, is_underlined: bool, size: Option<u32>) -> String {
    let mut inner = run_props.strip_prefix("<w:rPr>").and_then(|props| props.strip_suffix("</w:rPr>")).unwrap_or("").to_string();
    if let Some(size) = size {
        while let Some(start) = find_element_start(&inner, "<w:sz").or_else(|| find_element_start(&inner, "<w:szCs")) {
            let Some(end) = inner[start..].find("/>") else { break };
            inner.replace_range(start..start + end + 2, "");
        }
        inner.push_str(&format!("<w:sz w:val=\"{size}\"/><w:szCs w:val=\"{size}\"/>"));
    }
    if is_bold && find_element_start(&inner, "<w:b").is_none() {
        inner.push_str("<w:b/>");
    }
    if is_italic && find_element_start(&inner, "<w:i").is_none() {
        inner.push_str("<w:i/>");
    }
    if is_underlined && !inner.contains("<w:u ") {
        inner.push_str("<w:u w:val=\"single\"/>");
    }
    if inner.is_empty() { String::new() } else { format!("<w:rPr>{inner}</w:rPr>") }
}

/// HTML text escaped for `<w:t>`: `&`, `<` and `>` only.
fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let name_length = rest[1..].bytes().take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'#').count();
        let decoded = if rest[1 + name_length..].starts_with(';') { decode_entity(&rest[1..1 + name_length]) } else { None };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[name_length + 2..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_entity(name: &str) -> Option<char> {
    if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    if let Some(decimal) = name.strip_prefix('#') {
        return decimal.parse().ok().and_then(char::from_u32);
    }
    match name.to_ascii_lowercase().as_str() {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{A0}'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_entities_and_leaves_unknown_ones() {
        assert_eq!(decode_entities("a &amp; b &#65;&#x42; &bogus; &"), "a & b AB &bogus; &");
    }

    #[test]
    fn skips_a_tag_with_attributes_as_text() {
        let blocks = read_html("<p class=\"x\">t</p>");
        assert_eq!(blocks[0].runs[0].text.as_deref(), Some("p class=\"x\">t"));
    }
}
