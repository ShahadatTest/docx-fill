use docx_fill::{CommentDefaults, FilledDocument, TextPosition, TextRange};

fn document(body: &str) -> FilledDocument {
    let document = format!(r#"<w:document xmlns:w="w"><w:body>{body}</w:body></w:document>"#);
    FilledDocument::new([
        ("word/document.xml".to_string(), document.into_bytes()),
        ("word/_rels/document.xml.rels".to_string(), br#"<Relationships xmlns="r"></Relationships>"#.to_vec()),
        ("[Content_Types].xml".to_string(), br#"<Types xmlns="t"></Types>"#.to_vec()),
    ])
}

fn part(document: FilledDocument, name: &str) -> String {
    let parts = document.into_parts();
    String::from_utf8(parts.into_iter().find(|(each, _)| each == name).unwrap_or_else(|| panic!("no {name}")).1).unwrap()
}

fn body(document: FilledDocument) -> String {
    let xml = part(document, "word/document.xml");
    xml["<w:document xmlns:w=\"w\"><w:body>".len()..xml.len() - "</w:body></w:document>".len()].to_string()
}

const SPLIT: &str = r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Lorem ip</w:t></w:r><w:r><w:t>sum &amp; Lorem</w:t><w:tab/><w:t>end</w:t></w:r></w:p>"#;

#[test]
fn finds_text_across_runs_with_tabs_and_breaks() {
    let document = document(&format!(r#"{SPLIT}<w:p><w:pPr><w:tabs><w:tab w:val="left"/></w:tabs></w:pPr><w:r><w:t>a</w:t><w:br/><w:t>Lorem</w:t></w:r></w:p>"#));

    let found = document.search("Lorem");
    let at = |paragraph, offset| TextPosition { paragraph, offset };
    assert_eq!(found.iter().map(|range| (range.start, range.end)).collect::<Vec<_>>(), [(at(0, 0), at(0, 5)), (at(0, 14), at(0, 19)), (at(1, 2), at(1, 7))]);
    assert_eq!(document.search("ipsum & Lorem\tend").len(), 1);
    assert_eq!(document.search("a\nL").len(), 1);
    assert!(document.search("").is_empty());
}

#[test]
fn replaces_a_match_in_its_first_run() {
    let mut document = document(SPLIT);
    let found = document.search("ipsum &");
    document.replace(&found[0], "<x>").unwrap();

    assert_eq!(body(document), r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">Lorem &lt;x&gt;</w:t></w:r><w:r><w:t xml:space="preserve"> Lorem</w:t><w:tab/><w:t>end</w:t></w:r></w:p>"#);
}

#[test]
fn replaces_tabs_and_refuses_a_stale_range() {
    let mut document = document(SPLIT);
    let found = document.search("Lorem\te");
    document.replace(&found[0], "X").unwrap();
    assert!(document.replace(&found[0], "Y").is_err());

    let spanning = TextRange { start: TextPosition { paragraph: 0, offset: 0 }, end: TextPosition { paragraph: 1, offset: 0 }, text: String::new() };
    assert!(document.replace(&spanning, "Y").is_err());
    assert!(body(document).contains(r#"<w:t xml:space="preserve">sum &amp; X</w:t><w:t xml:space="preserve">nd</w:t>"#));
}

#[test]
fn replaces_every_match() {
    let mut document = document(&format!("{SPLIT}{SPLIT}"));
    assert_eq!(document.replace_all("Lorem", "Dolor"), 4);
    assert_eq!(document.search("Dolor").len(), 4);
    assert!(document.search("Lorem").is_empty());
}

#[test]
fn adds_a_comment_around_a_match_and_lists_it() {
    let mut document = document(SPLIT);
    let found = document.search("ipsum");
    let by = CommentDefaults { author: "Jane".to_string(), initials: Some("J".to_string()), date: Some("2026-11-02T09:00:00Z".to_string()) };
    assert_eq!(document.add_comment(&found[0], "Latin\nword", &by).unwrap(), 0);
    let found = document.search("end");
    assert_eq!(document.add_comment(&found[0], "Last", &CommentDefaults::default()).unwrap(), 1);

    let comments = document.comments();
    assert_eq!(comments.len(), 2);
    let first = &comments[0];
    assert_eq!((first.id, first.comment.as_str(), first.author.as_deref(), first.initials.as_deref()), (0, "Latin\nword", Some("Jane"), Some("J")));
    assert_eq!(first.range.text, "ipsum");
    assert_eq!((first.range.start.offset, first.range.end.offset), (6, 11));
    assert_eq!(first.xml, r#"<w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">ip</w:t></w:r><w:r><w:t xml:space="preserve">sum</w:t></w:r>"#);
    assert_eq!(comments[1].range.text, "end");

    let xml = body(document);
    assert!(xml.contains(r#"<w:t xml:space="preserve">Lorem </w:t></w:r><w:commentRangeStart w:id="0"/><w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">ip</w:t>"#), "{xml}");
    assert!(xml.contains(r#"sum</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:rPr><w:rStyle w:val="CommentReference"/></w:rPr><w:commentReference w:id="0"/></w:r><w:r><w:t xml:space="preserve"> &amp; Lorem</w:t>"#));
}

#[test]
fn writes_the_comments_part_and_replaces_commented_text() {
    let mut document = document(SPLIT);
    let found = document.search("Lorem");
    document.add_comment(&found[0], "First", &CommentDefaults::default()).unwrap();
    let comment = document.comments().remove(0);
    document.replace(&comment.range, "Dolor").unwrap();

    let comments = document.comments();
    assert_eq!(comments[0].range.text, "Dolor");
    let parts = document.into_parts();
    let read = |name: &str| String::from_utf8(parts.iter().find(|(each, _)| each == name).unwrap().1.clone()).unwrap();
    assert!(read("word/comments.xml").contains(r#"<w:comment w:id="0" w:author="Author">"#));
    assert!(read("word/_rels/document.xml.rels").contains("comments.xml"));
    assert!(read("[Content_Types].xml").contains("/word/comments.xml"));
}
