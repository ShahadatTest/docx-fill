//! Parses a part into XML, text and tags, and pairs its loops with the XML they repeat.

use crate::styling::StyleKind;
use crate::table_merge::{Merge, read_merge};
use crate::xml::{ElementKind, Token, read_attribute, tokenize, unescape};
use crate::{Delimiters, Extensions, FillError, FillOptions};

const PARAGRAPH: &str = "w:p";
const TEXT: &str = "w:t";
const CELL: &str = "w:tc";
const ROW: &str = "w:tr";
const RUN: &str = "w:r";
const RUN_PROPS: &str = "w:rPr";
const PRESERVE_SPACE: &str = " xml:space=\"preserve\"";
// Word lengths are twentieths of a point; pixels are 1/96 inch.
pub(crate) const PX_PER_TWIP: f64 = 96.0 / 1440.0;

#[derive(Debug)]
pub(crate) enum Part {
    Element { kind: ElementKind, name: String, raw: String },
    Text(String),
    Tag(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TagKind {
    /// `{name}`
    Value,
    /// `{@name}`: raw XML in place of the tag's paragraph.
    Raw,
    /// `{#name}`, `{^name}` (inverted) or `{-w:tr name}` (repeats the named element).
    Open { is_inverted: bool, element: Option<String> },
    /// `{/name}`, or `{/}` for any loop.
    Close,
    /// A tag starting with `FillOptions::module_prefixes[index]`.
    Module(usize),
    /// `{:stylecell name}`: a background for the table cell the tag is in.
    CellStyle,
    /// `{~~name}` (block) or `{~name}` (inline): HTML printed as Word runs and paragraphs.
    RichText { is_block: bool },
    /// `{%name}` or `{%%name}`: an image filling its table cell, else the page's width. With
    /// centred images on, `{%%name}` centres its paragraph.
    Image { cell_width: Option<f64>, is_centered: bool },
    /// `{:stylepar name}`, `{:stylerow name}`, `{:stylerun name}`, `{:stylebullets name}`,
    /// `{:styleshape name}` or `{:stylestart name}`.
    Style(StyleTarget),
    /// `{:/style}`: ends the innermost `{:stylestart}` range.
    StyleEnd,
    /// `{::stylepar name}` or `{::stylerun name}`: restyles the definition of the tag
    /// paragraph's or run's style.
    GlobalStyle(StyleKind),
    /// `{:merge-cells-col 1,2}` or `{:merge-cells-row}`: merges equal cells of the tag's table.
    TableMerge(Merge),
    /// `{r@name}`: replaces its run with the value as raw XML.
    RawRun,
    /// `{?name}`: keeps, drops or repeats its paragraph by the value.
    Placeholder,
    /// `{:footnote name}`: a footnote holding the value.
    Footnote,
    /// `{:table name}`: a table built from the value, in place of the tag's paragraph.
    FullTable,
    /// `{:#grid name}`: the paragraphs up to `{:/grid}` printed once per item in a grid of cells.
    GridOpen,
}

/// What a styling tag styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StyleTarget {
    /// `{:stylepar}` and `{:stylebullets}`.
    Paragraph,
    Row,
    Run,
    Shape,
    /// `{:stylestart}`: the runs up to `{:/style}`.
    Range,
}

#[derive(Debug)]
pub(crate) struct Tag {
    pub kind: TagKind,
    pub name: String,
    /// The tag as written, without its delimiters.
    pub text: String,
    pub part: usize,
    pub paragraph: usize,
    /// Nothing else is in its paragraph's text.
    pub is_alone: bool,
    /// A value tag's `<w:rPr>`, empty when its run has none; `None` outside a run or without
    /// line breaks.
    pub run_props: Option<String>,
}

#[derive(Debug)]
pub(crate) enum BlockKind {
    Loop { name: String, is_inverted: bool },
    Raw { name: String },
    /// A paragraph holding `{?name}` tags, printed by their values.
    Placeholder { names: Vec<String> },
    /// A `{:table}` tag's paragraph.
    FullTable { name: String, is_in_cell: bool },
    /// A grid's tag paragraphs and the paragraphs between them, the body.
    Grid { name: String },
}

/// A range of parts a loop or raw tag replaces: `start..end` is printed as `body_start..body_end`
/// once per item.
#[derive(Debug)]
pub(crate) struct Block {
    pub kind: BlockKind,
    pub start: usize,
    pub end: usize,
    pub body_start: usize,
    pub body_end: usize,
    pub children: Vec<usize>,
    open_tag: usize,
}

/// A parsed template part, ready to fill any number of times.
#[derive(Debug)]
pub struct Template {
    pub(crate) parts: Vec<Part>,
    pub(crate) tags: Vec<Tag>,
    pub(crate) blocks: Vec<Block>,
    /// Blocks not inside another, in document order.
    pub(crate) top: Vec<usize>,
    pub(crate) linebreaks: bool,
    pub(crate) html_extras: bool,
    pub(crate) drop_style_paragraphs: bool,
    /// The largest drawing id the part uses, so new drawings take ids after it.
    pub(crate) max_drawing_id: u32,
    /// Page width and height in pixels, less margins, from the part's last section.
    pub(crate) body: Option<(f64, f64)>,
}

/// A tag found in a paragraph's joined text: byte offsets of its opening delimiter and after its
/// closing one.
struct FoundTag {
    start: usize,
    end: usize,
}

struct Paragraph {
    start: usize,
    end: usize,
}

/// A paragraph's start token and, once it closes, its end token.
type ParagraphTokens = (usize, Option<usize>);

impl Template {
    /// Parses `xml`, a body, header or footer using `{ }` delimiters.
    ///
    /// @returns An error for an unclosed or unopened tag or loop, a loop closed by another loop's
    ///   tag, or a dash loop outside the element it names.
    pub fn parse(xml: &str, options: &FillOptions) -> Result<Template, FillError> {
        let tokens = tokenize(xml).map_err(FillError::new)?;
        let (paragraph_tokens, text_tokens) = scan_paragraphs(&tokens);

        // Tags by paragraph, found in its `<w:t>` text joined, since Word splits a tag across runs.
        let mut found = Vec::with_capacity(text_tokens.len());
        let mut joined_texts = Vec::with_capacity(text_tokens.len());
        for texts in &text_tokens {
            let joined: String = texts.iter().map(|&i| text_of(&tokens[i])).collect();
            found.push(find_tags(&joined, &options.delimiters)?);
            joined_texts.push(joined);
        }

        let mut builder = Builder { parts: Vec::with_capacity(tokens.len()), tags: Vec::new() };
        let mut paragraphs: Vec<Paragraph> = paragraph_tokens.iter().map(|_| Paragraph { start: 0, end: 0 }).collect();
        let mut paragraph_starts = vec![None; tokens.len()];
        let mut paragraph_ends = vec![None; tokens.len()];
        for (paragraph, &(start, end)) in paragraph_tokens.iter().enumerate() {
            paragraph_starts[start] = Some(paragraph);
            if let Some(end) = end {
                paragraph_ends[end] = Some(paragraph);
            }
        }
        let mut text_owner = vec![None; tokens.len()];
        for (paragraph, texts) in text_tokens.iter().enumerate() {
            let mut offset = 0;
            for &token in texts {
                let length = text_of(&tokens[token]).len();
                text_owner[token] = Some((paragraph, offset));
                offset += length;
            }
        }

        for (index, token) in tokens.iter().enumerate() {
            match *token {
                Token::Element { kind, name, raw } => {
                    if let Some(paragraph) = paragraph_starts[index] {
                        paragraphs[paragraph].start = builder.parts.len();
                    }
                    if let Some(paragraph) = paragraph_ends[index] {
                        paragraphs[paragraph].end = builder.parts.len();
                    }
                    let touched = kind == ElementKind::Open && name == TEXT && is_touched(&tokens, index + 1, &text_owner, &found);
                    let raw = if touched { preserve_space(raw) } else { raw.to_string() };
                    builder.parts.push(Part::Element { kind, name: name.to_string(), raw });
                }
                Token::Text(text) => match text_owner[index] {
                    Some((paragraph, offset)) => {
                        builder.split_text(text, offset, paragraph, &found[paragraph], &joined_texts[paragraph], options)
                    }
                    None => builder.parts.push(Part::Text(text.to_string())),
                },
            }
        }

        let Builder { parts, mut tags } = builder;
        for tag in &mut tags {
            let paragraph = &found[tag.paragraph];
            tag.is_alone = paragraph.len() == 1 && paragraph[0].end - paragraph[0].start == joined_texts[tag.paragraph].len();
        }

        for tag in &mut tags {
            match &mut tag.kind {
                TagKind::Image { cell_width, .. } => *cell_width = read_cell_width(&parts, tag.part),
                TagKind::Value if options.linebreaks => tag.run_props = read_run_props(&parts, tag.part),
                TagKind::Footnote => tag.run_props = read_run_props(&parts, tag.part),
                _ => {}
            }
        }

        let blocks = pair_blocks(&parts, &tags, &paragraphs, options)?;
        let mut template = Template {
            max_drawing_id: crate::images::read_max_drawing_id(xml),
            body: crate::images::read_body_size(xml),
            parts,
            tags,
            blocks,
            top: Vec::new(),
            linebreaks: options.linebreaks,
            html_extras: options.extensions.html_extras,
            drop_style_paragraphs: options.drop_style_paragraphs,
        };
        template.nest_blocks()?;
        Ok(template)
    }

    /// Puts each block under the innermost block containing it.
    fn nest_blocks(&mut self) -> Result<(), FillError> {
        let mut order: Vec<usize> = (0..self.blocks.len()).collect();
        order.sort_by_key(|&b| (self.blocks[b].start, std::cmp::Reverse(self.blocks[b].end), self.blocks[b].open_tag));

        let mut open: Vec<usize> = Vec::new();
        for block in order {
            while open.last().is_some_and(|&outer| self.blocks[outer].end <= self.blocks[block].start) {
                open.pop();
            }
            match open.last() {
                Some(&outer) if self.blocks[block].end > self.blocks[outer].end => {
                    return Err(FillError::new(format!("The loops \"{}\" and \"{}\" overlap", self.block_name(outer), self.block_name(block))));
                }
                Some(&outer) => self.blocks[outer].children.push(block),
                None => self.top.push(block),
            }
            open.push(block);
        }
        Ok(())
    }

    fn block_name(&self, block: usize) -> &str {
        match &self.blocks[block].kind {
            BlockKind::Loop { name, .. } | BlockKind::Raw { name } => name,
            BlockKind::Placeholder { names } => names.first().map_or("", String::as_str),
            BlockKind::FullTable { name, .. } | BlockKind::Grid { name } => name,
        }
    }
}

struct Builder {
    parts: Vec<Part>,
    tags: Vec<Tag>,
}

impl Builder {
    /// Pushes one `<w:t>` text as text and tags. A tag goes where its `{` is; the text of a tag
    /// that spilled into this run from an earlier one is dropped.
    fn split_text(&mut self, text: &str, offset: usize, paragraph: usize, found: &[FoundTag], joined: &str, options: &FillOptions) {
        let end = offset + text.len();
        let mut cursor = offset;
        for tag in found.iter().filter(|tag| tag.end > offset && tag.start < end) {
            if tag.start > cursor {
                self.parts.push(Part::Text(joined[cursor..tag.start].to_string()));
            }
            if tag.start >= offset {
                let written = &joined[tag.start + options.delimiters.start.len()..tag.end - options.delimiters.end.len()];
                let (kind, name) = read_tag(&unescape(written), options);
                self.tags.push(Tag { kind, name, text: written.to_string(), part: self.parts.len(), paragraph, is_alone: false, run_props: None });
                self.parts.push(Part::Tag(self.tags.len() - 1));
            }
            cursor = tag.end.min(end);
        }
        if cursor < end {
            self.parts.push(Part::Text(joined[cursor..end].to_string()));
        }
    }
}

/// Each innermost paragraph's start and end token, and its `<w:t>` text tokens.
fn scan_paragraphs(tokens: &[Token]) -> (Vec<ParagraphTokens>, Vec<Vec<usize>>) {
    let mut paragraphs = Vec::new();
    let mut texts: Vec<Vec<usize>> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut is_in_text = false;
    for (index, token) in tokens.iter().enumerate() {
        match *token {
            Token::Element { kind: ElementKind::Open, name: PARAGRAPH, .. } => {
                open.push(paragraphs.len());
                paragraphs.push((index, None));
                texts.push(Vec::new());
            }
            Token::Element { kind: ElementKind::Close, name: PARAGRAPH, .. } => {
                if let Some(paragraph) = open.pop() {
                    paragraphs[paragraph].1 = Some(index);
                }
            }
            Token::Element { kind: ElementKind::Open, name: TEXT, .. } => is_in_text = true,
            Token::Element { kind: ElementKind::Close, name: TEXT, .. } => is_in_text = false,
            Token::Text(_) if is_in_text => {
                if let Some(&paragraph) = open.last() {
                    texts[paragraph].push(index);
                }
            }
            _ => {}
        }
    }
    (paragraphs, texts)
}

fn text_of<'a>(token: &Token<'a>) -> &'a str {
    match *token {
        Token::Text(text) => text,
        Token::Element { .. } => "",
    }
}

/// Whether the text token at `index` holds part of a tag.
fn is_touched(tokens: &[Token], index: usize, owner: &[Option<(usize, usize)>], found: &[Vec<FoundTag>]) -> bool {
    let Some(Some((paragraph, offset))) = owner.get(index) else { return false };
    let end = offset + text_of(&tokens[index]).len();
    found[*paragraph].iter().any(|tag| tag.end > *offset && tag.start < end)
}

fn preserve_space(raw: &str) -> String {
    if raw.contains("xml:space=") {
        return raw.to_string();
    }
    let at = raw.len() - 1;
    format!("{}{PRESERVE_SPACE}{}", &raw[..at], &raw[at..])
}

/// The tags in a paragraph's joined text.
fn find_tags(text: &str, delimiters: &Delimiters) -> Result<Vec<FoundTag>, FillError> {
    let (start, end) = (delimiters.start.as_str(), delimiters.end.as_str());
    let mut tags = Vec::new();
    let mut open = None;
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        if rest.starts_with(start) {
            if let Some(opened) = open {
                return Err(unclosed_tag(&text[opened..at]));
            }
            open = Some(at);
            at += start.len();
        } else if rest.starts_with(end) {
            let Some(opened) = open.take() else {
                let context_start = tags.last().map_or(0, |tag: &FoundTag| tag.end);
                return Err(FillError::new(format!("The tag beginning with \"{}\" is unopened", unescape(&text[context_start..at]))));
            };
            at += end.len();
            tags.push(FoundTag { start: opened, end: at });
        } else {
            at += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    match open {
        Some(opened) => Err(unclosed_tag(&text[opened..])),
        None => Ok(tags),
    }
}

fn unclosed_tag(context: &str) -> FillError {
    FillError::new(format!("The tag beginning with \"{}\" is unclosed", unescape(context)))
}

/// A tag's kind and the name it looks up.
fn read_tag(text: &str, options: &FillOptions) -> (TagKind, String) {
    if let Some((index, prefix)) = options.module_prefixes.iter().enumerate().find(|(_, prefix)| text.starts_with(prefix.as_str())) {
        return (TagKind::Module(index), text[prefix.len()..].to_string());
    }
    if let Some(found) = read_extension_tag(text, &options.extensions) {
        return found;
    }

    let rest = text.get(1..).unwrap_or_default().to_string();
    match text.as_bytes().first() {
        Some(b'#') => (TagKind::Open { is_inverted: false, element: None }, rest),
        Some(b'^') => (TagKind::Open { is_inverted: true, element: None }, rest),
        Some(b'/') => (TagKind::Close, rest),
        Some(b'@') => (TagKind::Raw, rest),
        // `-w:tr rows`: the element, then the name.
        Some(b'-') => {
            let (element, name) = rest.split_once(char::is_whitespace).unwrap_or((&rest, ""));
            (TagKind::Open { is_inverted: false, element: Some(element.to_string()) }, name.trim_start().to_string())
        }
        _ => (TagKind::Value, text.to_string()),
    }
}

fn read_extension_tag(text: &str, extensions: &Extensions) -> Option<(TagKind, String)> {
    if extensions.full_tables {
        if let Some(name) = text.strip_prefix(":table ") {
            return Some((TagKind::FullTable, name.trim().to_string()));
        }
        if let Some(name) = text.strip_prefix(":#grid ") {
            return Some((TagKind::GridOpen, name.trim().to_string()));
        }
        if text.trim() == ":/grid" {
            return Some((TagKind::Close, String::new()));
        }
    }
    if extensions.table_merge {
        for (prefix, is_columns) in [(":merge-cells-col", true), (":merge-cells-row", false)] {
            let Some(rest) = text.strip_prefix(prefix) else { continue };
            if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                return Some((TagKind::TableMerge(read_merge(is_columns, rest)), String::new()));
            }
        }
    }
    if extensions.styling {
        for (prefix, kind) in [("::stylepar ", StyleKind::Paragraph), ("::stylerun ", StyleKind::Character)] {
            if let Some(name) = text.strip_prefix(prefix) {
                return Some((TagKind::GlobalStyle(kind), name.trim().to_string()));
            }
        }
        if text.trim() == ":/style" {
            return Some((TagKind::StyleEnd, String::new()));
        }
        let targets = [
            (":stylepar ", StyleTarget::Paragraph),
            (":stylebullets ", StyleTarget::Paragraph),
            (":stylerow ", StyleTarget::Row),
            (":stylerun ", StyleTarget::Run),
            (":styleshape ", StyleTarget::Shape),
            (":stylestart ", StyleTarget::Range),
        ];
        for (prefix, target) in targets {
            if let Some(name) = text.strip_prefix(prefix) {
                return Some((TagKind::Style(target), name.trim().to_string()));
            }
        }
    }
    if extensions.footnotes
        && let Some(name) = text.strip_prefix(":footnote ")
    {
        return Some((TagKind::Footnote, name.trim().to_string()));
    }
    if extensions.word_run
        && let Some(name) = text.strip_prefix("r@")
    {
        return Some((TagKind::RawRun, name.trim().to_string()));
    }
    if extensions.paragraph_placeholder
        && let Some(name) = text.strip_prefix('?')
    {
        return Some((TagKind::Placeholder, name.trim().to_string()));
    }
    if extensions.vertical_tables {
        // `{:vt#cols}` … `{:vt/cols}` repeat the cells from the opening tag's to the closing tag's.
        if let Some(name) = text.strip_prefix(":vt#") {
            return Some((TagKind::Open { is_inverted: false, element: Some(CELL.to_string()) }, name.trim().to_string()));
        }
        if let Some(name) = text.strip_prefix(":vt/") {
            return Some((TagKind::Close, name.trim().to_string()));
        }
    }
    if extensions.cell_styling
        && let Some(name) = text.strip_prefix(":stylecell ")
    {
        return Some((TagKind::CellStyle, name.trim().to_string()));
    }
    if extensions.rich_text {
        if let Some(name) = text.strip_prefix("~~") {
            return Some((TagKind::RichText { is_block: true }, name.to_string()));
        }
        if let Some(name) = text.strip_prefix('~') {
            return Some((TagKind::RichText { is_block: false }, name.to_string()));
        }
    }
    if extensions.images {
        if let Some(name) = text.strip_prefix("%%") {
            return Some((TagKind::Image { cell_width: None, is_centered: extensions.centered_images }, name.trim().to_string()));
        }
        let name = text.strip_prefix('%')?;
        return Some((TagKind::Image { cell_width: None, is_centered: false }, name.trim().to_string()));
    }
    None
}

/// The width in pixels of the innermost table cell around part `at`, when its `<w:tcW>` gives
/// one in twips.
fn read_cell_width(parts: &[Part], at: usize) -> Option<f64> {
    let (start, _) = find_enclosing(parts, at, CELL)?;
    for part in &parts[start + 1..at] {
        let Part::Element { name, raw, .. } = part else { continue };
        if name == PARAGRAPH {
            return None;
        }
        if name == "w:tcW" {
            if read_attribute(raw, "w:type") != Some("dxa") {
                return None;
            }
            return read_attribute(raw, "w:w")?.parse::<f64>().ok().map(|twips| twips * PX_PER_TWIP);
        }
    }
    None
}

/// The raw `<w:rPr>` opening the run around part `at`, empty when there is none.
fn read_run_props(parts: &[Part], at: usize) -> Option<String> {
    let (start, _) = find_enclosing(parts, at, RUN)?;
    let mut props = String::new();
    for part in &parts[start + 1..at] {
        let Part::Element { kind, name, raw } = part else { break };
        match (kind, name.as_str()) {
            (ElementKind::SelfClosing, RUN_PROPS) => return Some(String::new()),
            (ElementKind::Close, RUN_PROPS) => {
                props.push_str(raw);
                return Some(props);
            }
            _ if props.is_empty() && name != RUN_PROPS => break,
            _ => props.push_str(raw),
        }
    }
    Some(String::new())
}

/// Pairs each loop's tags and works out the parts every loop and raw tag replaces.
fn pair_blocks(parts: &[Part], tags: &[Tag], paragraphs: &[Paragraph], options: &FillOptions) -> Result<Vec<Block>, FillError> {
    let mut blocks = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    for (index, tag) in tags.iter().enumerate() {
        match tag.kind {
            TagKind::Open { .. } | TagKind::GridOpen => open.push(index),
            TagKind::Close => {
                let Some(opening) = open.pop() else {
                    return Err(FillError::new(format!("The tag \"{}\" has no opening loop tag", tag.text)));
                };
                if !tag.name.is_empty() && tag.name != tags[opening].name {
                    return Err(FillError::new(format!("The tag \"{}\" is closed by the tag \"{}\"", tags[opening].name, tag.name)));
                }
                if tags[opening].kind == TagKind::GridOpen {
                    blocks.push(expand_grid(tags, paragraphs, opening, index)?);
                    continue;
                }
                blocks.push(expand_loop(parts, tags, paragraphs, opening, index, options)?);
            }
            TagKind::Raw | TagKind::RawRun => {
                let element = if tag.kind == TagKind::Raw { PARAGRAPH } else { RUN };
                let (start, end) = find_enclosing(parts, tag.part, element).ok_or_else(|| not_inside(tag, element))?;
                blocks.push(Block {
                    kind: BlockKind::Raw { name: tag.name.clone() },
                    start,
                    end: end + 1,
                    body_start: end + 1,
                    body_end: end + 1,
                    children: Vec::new(),
                    open_tag: index,
                });
            }
            TagKind::Image { is_centered: true, .. } if !tag.is_alone => {
                return Err(FillError::new(format!("Centered Images should be placed in empty paragraphs, but the tag \"{}\" is not", tag.text)));
            }
            TagKind::FullTable => {
                if !tag.is_alone {
                    return Err(FillError::new(format!("The table tag \"{}\" should be the only text in its paragraph", tag.text)));
                }
                let (start, end) = find_enclosing(parts, tag.part, PARAGRAPH).ok_or_else(|| not_inside(tag, PARAGRAPH))?;
                blocks.push(Block {
                    kind: BlockKind::FullTable { name: tag.name.clone(), is_in_cell: find_enclosing(parts, tag.part, CELL).is_some() },
                    start,
                    end: end + 1,
                    body_start: end + 1,
                    body_end: end + 1,
                    children: Vec::new(),
                    open_tag: index,
                });
            }
            TagKind::Placeholder => {
                let (start, end) = find_enclosing(parts, tag.part, PARAGRAPH).ok_or_else(|| not_inside(tag, PARAGRAPH))?;
                // One block per paragraph, naming each of its placeholders.
                if let Some(Block { kind: BlockKind::Placeholder { names }, .. }) = blocks.iter_mut().find(|block| block.start == start && matches!(block.kind, BlockKind::Placeholder { .. })) {
                    names.push(tag.name.clone());
                    continue;
                }
                blocks.push(Block {
                    kind: BlockKind::Placeholder { names: vec![tag.name.clone()] },
                    start,
                    end: end + 1,
                    body_start: start,
                    body_end: end + 1,
                    children: Vec::new(),
                    open_tag: index,
                });
            }
            TagKind::Value
            | TagKind::Module(_)
            | TagKind::CellStyle
            | TagKind::RichText { .. }
            | TagKind::Image { .. }
            | TagKind::Style(_)
            | TagKind::StyleEnd
            | TagKind::GlobalStyle(_)
            | TagKind::TableMerge(_)
            | TagKind::Footnote => {}
        }
    }
    if let Some(&opening) = open.last() {
        return Err(FillError::new(format!("The loop with tag \"{}\" is unclosed", tags[opening].name)));
    }
    Ok(blocks)
}

/// A grid's parts: its tags' paragraphs, each holding only its tag, and the paragraphs between.
fn expand_grid(tags: &[Tag], paragraphs: &[Paragraph], opening: usize, closing: usize) -> Result<Block, FillError> {
    let (open, close) = (&tags[opening], &tags[closing]);
    let (first, last) = (&paragraphs[open.paragraph], &paragraphs[close.paragraph]);
    if open.paragraph == close.paragraph || !open.is_alone || !close.is_alone || first.end <= first.start || last.end <= last.start {
        return Err(FillError::new(format!("The grid tags \"{}\" should each be alone in their own paragraph", open.name)));
    }
    Ok(Block {
        kind: BlockKind::Grid { name: open.name.clone() },
        start: first.start,
        end: last.end + 1,
        body_start: first.end + 1,
        body_end: last.start,
        children: Vec::new(),
        open_tag: opening,
    })
}

/// The parts a loop repeats:
/// - a dash loop's element, from the one holding the opening tag to the one holding the closing;
/// - the table rows it spans, when its tags sit in different cells;
/// - with `paragraph_loop`, the paragraphs between its tags, when each tag is alone in its own
///   paragraph (the tags' paragraphs go);
/// - otherwise the XML between its tags.
fn expand_loop(parts: &[Part], tags: &[Tag], paragraphs: &[Paragraph], opening: usize, closing: usize, options: &FillOptions) -> Result<Block, FillError> {
    let (open, close) = (&tags[opening], &tags[closing]);
    let TagKind::Open { is_inverted, element } = &open.kind else { unreachable!("pairs start at an opening tag") };
    let kind = BlockKind::Loop { name: open.name.clone(), is_inverted: *is_inverted };
    let block = |start, end, body_start, body_end| Block { kind, start, end, body_start, body_end, children: Vec::new(), open_tag: opening };

    let element = match element {
        Some(element) => Some(element.clone()),
        None => is_crossing_cells(parts, open.part, close.part).map_err(|_| misplaced_loop(open))?.then(|| ROW.to_string()),
    };
    if let Some(element) = element {
        let (start, _) = find_enclosing(parts, open.part, &element).ok_or_else(|| not_inside(open, &element))?;
        let (_, end) = find_enclosing(parts, close.part, &element).ok_or_else(|| not_inside(close, &element))?;
        return Ok(block(start, end + 1, start, end + 1));
    }

    let is_paragraph_loop = options.paragraph_loop && open.paragraph != close.paragraph && open.is_alone && close.is_alone;
    if is_paragraph_loop {
        let (first, last) = (&paragraphs[open.paragraph], &paragraphs[close.paragraph]);
        if first.end > first.start && last.end > last.start {
            return Ok(block(first.start, last.end + 1, first.end + 1, last.start));
        }
    }
    Ok(block(open.part, close.part + 1, open.part + 1, close.part))
}

/// Whether a loop's tags sit in different table cells. An error when only one is in a cell.
fn is_crossing_cells(parts: &[Part], from: usize, to: usize) -> Result<bool, ()> {
    match (find_enclosing(parts, from, CELL), find_enclosing(parts, to, CELL)) {
        (Some(open), Some(close)) => Ok(open != close),
        (None, None) => Ok(false),
        _ => Err(()),
    }
}

fn misplaced_loop(tag: &Tag) -> FillError {
    FillError::new(format!("The position of the loop tags \"{}\" would produce invalid XML", tag.name))
}

/// The start and end parts of the innermost `element` around part `at`.
fn find_enclosing(parts: &[Part], at: usize, element: &str) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut start = None;
    for index in (0..at).rev() {
        let Part::Element { kind, name, .. } = &parts[index] else { continue };
        if name != element {
            continue;
        }
        match kind {
            ElementKind::Close => depth += 1,
            ElementKind::Open if depth == 0 => {
                start = Some(index);
                break;
            }
            ElementKind::Open => depth -= 1,
            _ => {}
        }
    }
    let start = start?;

    for (index, part) in parts.iter().enumerate().skip(at + 1) {
        let Part::Element { kind, name, .. } = part else { continue };
        if name != element {
            continue;
        }
        match kind {
            ElementKind::Open => depth += 1,
            ElementKind::Close if depth == 0 => return Some((start, index)),
            ElementKind::Close => depth -= 1,
            _ => {}
        }
    }
    None
}

fn not_inside(tag: &Tag, element: &str) -> FillError {
    FillError::new(format!("The tag \"{}\" is not inside a <{element}>", tag.text))
}
