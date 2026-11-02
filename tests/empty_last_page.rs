//! Mirrors `apps/web/src/components/pdf/wordcraft/__tests__/emptyLastPage.test.ts` in Scope's
//! frontend.

use docx_fill::drop_empty_last_page;

const SECTION: &str = r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>"#;

fn body(content: &str) -> String {
    format!("<w:document><w:body>{content}{SECTION}</w:body></w:document>")
}

fn text(value: &str) -> String {
    format!("<w:p><w:r><w:t>{value}</w:t></w:r></w:p>")
}

fn page_break(before: &str) -> String {
    format!(r#"<w:p><w:r><w:t>{before}</w:t><w:br w:type="page"/></w:r></w:p>"#)
}

#[test]
fn drops_the_last_page_break_and_the_empty_paragraphs_after_it() {
    let dropped = drop_empty_last_page(&body(&(text("one") + &page_break("end") + "<w:p/><w:p><w:r></w:r></w:p>")));

    assert_eq!(dropped, body(&(text("one") + "<w:p><w:r><w:t>end</w:t></w:r></w:p>")));
}

#[test]
fn keeps_a_last_page_with_text_a_table_or_a_picture() {
    for content in [text("more"), "<w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl><w:p/>".to_string(), "<w:p><w:r><w:drawing/></w:r></w:p>".to_string()] {
        let xml = body(&(text("one") + &page_break("") + &content));
        assert_eq!(drop_empty_last_page(&xml), xml);
    }
}

#[test]
fn keeps_a_section_break_after_the_page_break() {
    let xml = body(&(text("one") + &page_break("") + &format!("<w:p><w:pPr>{SECTION}</w:pPr></w:p>")));

    assert_eq!(drop_empty_last_page(&xml), xml);
}

#[test]
fn leaves_a_body_with_no_page_break_alone() {
    let xml = body(&(text("one") + "<w:p/>"));

    assert_eq!(drop_empty_last_page(&xml), xml);
}
