//! A scanner for a DOCX part's XML: its elements and the text between them, verbatim.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElementKind {
    Open,
    Close,
    SelfClosing,
    /// A declaration, comment, CDATA section or processing instruction.
    Other,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Token<'a> {
    Element { kind: ElementKind, name: &'a str, raw: &'a str },
    Text(&'a str),
}

/// The part's tokens in document order. Concatenating their raw text gives back `xml`.
pub(crate) fn tokenize(xml: &str) -> Result<Vec<Token<'_>>, String> {
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < xml.len() {
        if !xml[at..].starts_with('<') {
            let end = xml[at..].find('<').map_or(xml.len(), |i| at + i);
            tokens.push(Token::Text(&xml[at..end]));
            at = end;
            continue;
        }

        let end = find_element_end(xml, at).ok_or_else(|| format!("Unclosed XML element at byte {at}"))?;
        tokens.push(read_element(&xml[at..end]));
        at = end;
    }
    Ok(tokens)
}

fn find_element_end(xml: &str, at: usize) -> Option<usize> {
    let rest = &xml[at..];
    for (open, close) in [("<!--", "-->"), ("<![CDATA[", "]]>"), ("<?", "?>")] {
        if rest.starts_with(open) {
            return rest.find(close).map(|i| at + i + close.len());
        }
    }

    // A `>` inside an attribute value does not end the element.
    let mut quote = None;
    for (i, byte) in rest.bytes().enumerate() {
        match quote {
            Some(q) if byte == q => quote = None,
            Some(_) => {}
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if byte == b'>' => return Some(at + i + 1),
            None => {}
        }
    }
    None
}

fn read_element(raw: &str) -> Token<'_> {
    if raw.starts_with("<!") || raw.starts_with("<?") {
        return Token::Element { kind: ElementKind::Other, name: "", raw };
    }

    let is_close = raw.starts_with("</");
    let name_start = if is_close { 2 } else { 1 };
    let name_end = raw[name_start..]
        .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .map_or(raw.len(), |i| name_start + i);
    let kind = if is_close {
        ElementKind::Close
    } else if raw.ends_with("/>") {
        ElementKind::SelfClosing
    } else {
        ElementKind::Open
    };
    Token::Element { kind, name: &raw[name_start..name_end], raw }
}

/// `text` with the five predefined entities and character references decoded.
pub(crate) fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let Some(semi) = rest.find(';') else { break };
        let decoded = match &rest[1..semi] {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            name if name.starts_with("#x") || name.starts_with("#X") => u32::from_str_radix(&name[2..], 16).ok().and_then(char::from_u32),
            name if name.starts_with('#') => name[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
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

/// `text` escaped for element content.
pub(crate) fn escape(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
}
