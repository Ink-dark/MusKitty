//! Tree construction tests for the prelude insertion modes.
//!
//! Covers Initial / BeforeHtml / BeforeHead / InHead / AfterHead and the
//! minimal Text mode, per WHATWG §13.2.6.4.1–§13.2.6.5.
//!
//! These verify the DOM structure produced by `parse()` for inputs that
//! exercise the prelude chain. Full html5lib tree construction coverage
//! comes in Phase 5.

use std::cell::RefCell;
use std::rc::Rc;

use muskitty_dom::{Node, NodeKind, NodeType};
use muskitty_html_parser::parse;

/// Find the first descendant element with the given node_name (uppercase,
/// per DOM §6.1 HTML-namespace convention).
fn find_element_by_name(root: &Rc<RefCell<Node>>, name: &str) -> Option<Rc<RefCell<Node>>> {
    for desc in Node::descendants(root) {
        if desc.borrow().node_type == NodeType::Element && desc.borrow().node_name == name {
            return Some(desc);
        }
    }
    None
}

/// Collect the node_names of all direct children of a node (uppercase).
fn child_names(node: &Rc<RefCell<Node>>) -> Vec<String> {
    node.borrow()
        .children
        .iter()
        .map(|c| c.borrow().node_name.clone())
        .collect()
}

// ── Initial / BeforeHtml / BeforeHead ──────────────────────────

#[test]
fn doctype_html_creates_document_type_and_html() {
    let doc = parse("<!DOCTYPE html>");
    let names = child_names(&doc);
    assert_eq!(names, vec!["html", "HTML"]);
    let dt = doc.borrow().first_child().unwrap();
    assert_eq!(dt.borrow().node_type, NodeType::DocumentType);
    assert_eq!(dt.borrow().node_name, "html");
}

#[test]
fn empty_input_auto_creates_html_head_body() {
    // Per §13.2.6.4.2, EOF triggers BeforeHtml's anything-else → create
    // <html>. Then BeforeHead creates <head>, AfterHead creates <body>.
    let doc = parse("");
    let html = find_element_by_name(&doc, "HTML").expect("missing <HTML>");
    let html_children = child_names(&html);
    assert!(
        html_children.contains(&"HEAD".to_string()),
        "expected <HEAD> under <HTML>, got {:?}",
        html_children
    );
    assert!(
        html_children.contains(&"BODY".to_string()),
        "expected <BODY> under <HTML>, got {:?}",
        html_children
    );
}

#[test]
fn comment_before_doctype_goes_to_document() {
    let doc = parse("<!-- hi --><!DOCTYPE html>");
    // First child should be the comment, attached to Document per
    // §13.2.6.4.1 Initial mode.
    let first = doc.borrow().first_child().unwrap();
    assert_eq!(first.borrow().node_type, NodeType::Comment);
}

// ── InHead: void head elements ─────────────────────────────────

#[test]
fn meta_element_is_created_and_not_open() {
    // <meta> should be created in <head> but not remain on the open
    // elements stack — subsequent text should still be in head context.
    let doc = parse("<!DOCTYPE html><meta charset=utf-8>");
    let meta = find_element_by_name(&doc, "META").expect("missing <META>");
    assert_eq!(meta.borrow().node_name, "META");
    // The meta element's parent should be HEAD.
    let parent = meta.borrow().parent_element().unwrap();
    assert_eq!(parent.borrow().node_name, "HEAD");
}

#[test]
fn link_element_is_created_in_head() {
    let doc = parse("<!DOCTYPE html><link rel=stylesheet href=x.css>");
    let link = find_element_by_name(&doc, "LINK").expect("missing <LINK>");
    let parent = link.borrow().parent_element().unwrap();
    assert_eq!(parent.borrow().node_name, "HEAD");
}

#[test]
fn base_element_is_created_in_head() {
    let doc = parse("<!DOCTYPE html><base href=https://example.com/>");
    let base = find_element_by_name(&doc, "BASE").expect("missing <BASE>");
    let parent = base.borrow().parent_element().unwrap();
    assert_eq!(parent.borrow().node_name, "HEAD");
}

// ── InHead: title (RCDATA + Text mode) ─────────────────────────

#[test]
fn title_element_contains_text_content() {
    let doc = parse("<!DOCTYPE html><title>Hello</title>");
    let title = find_element_by_name(&doc, "TITLE").expect("missing <TITLE>");
    assert_eq!(title.borrow().child_count(), 1);
    let text = title.borrow().first_child().unwrap();
    assert_eq!(text.borrow().node_type, NodeType::Text);
    assert_eq!(text.borrow().text_content().unwrap(), "Hello");
}

#[test]
fn title_with_entities_decodes_them() {
    // RCDATA mode decodes character references. &amp; → &.
    let doc = parse("<!DOCTYPE html><title>A&amp;B</title>");
    let title = find_element_by_name(&doc, "TITLE").expect("missing <TITLE>");
    let text = title.borrow().first_child().unwrap();
    assert_eq!(text.borrow().text_content().unwrap(), "A&B");
}

#[test]
fn title_end_tag_restores_previous_mode() {
    // After </title>, parsing should continue in the head/body context.
    // The auto-created <body> should exist.
    let doc = parse("<!DOCTYPE html><title>X</title>");
    let _body = find_element_by_name(&doc, "BODY").expect("missing <BODY>");
}

// ── InHead: style / script (RAWTEXT / ScriptData + Text mode) ─

#[test]
fn style_element_preserves_raw_text() {
    let doc = parse("<!DOCTYPE html><style>.a > b { color: red; }</style>");
    let style = find_element_by_name(&doc, "STYLE").expect("missing <STYLE>");
    let text = style.borrow().first_child().unwrap();
    assert_eq!(text.borrow().node_type, NodeType::Text);
    assert_eq!(
        text.borrow().text_content().unwrap(),
        ".a > b { color: red; }"
    );
}

#[test]
fn script_element_preserves_raw_text() {
    let doc = parse("<!DOCTYPE html><script>if (a < b) { alert('hi'); }</script>");
    let script = find_element_by_name(&doc, "SCRIPT").expect("missing <SCRIPT>");
    let text = script.borrow().first_child().unwrap();
    assert_eq!(
        text.borrow().text_content().unwrap(),
        "if (a < b) { alert('hi'); }"
    );
}

// ── InHead: head end tag ───────────────────────────────────────

#[test]
fn explicit_head_end_tag_switches_to_after_head() {
    let doc = parse("<!DOCTYPE html><head></head><body></body>");
    let head = find_element_by_name(&doc, "HEAD").expect("missing <HEAD>");
    let body = find_element_by_name(&doc, "BODY").expect("missing <BODY>");
    // Both should be children of <html>.
    let html = find_element_by_name(&doc, "HTML").unwrap();
    let html_children = child_names(&html);
    assert!(html_children.contains(&"HEAD".to_string()));
    assert!(html_children.contains(&"BODY".to_string()));
    // head and body should be siblings under html.
    assert_eq!(
        head.borrow().parent_element().unwrap().borrow().node_name,
        "HTML"
    );
    assert_eq!(
        body.borrow().parent_element().unwrap().borrow().node_name,
        "HTML"
    );
}

// ── AfterHead ──────────────────────────────────────────────────

#[test]
fn body_start_tag_creates_body_and_switches_to_in_body() {
    let doc = parse("<!DOCTYPE html><head></head><body>hi</body>");
    let body = find_element_by_name(&doc, "BODY").expect("missing <BODY>");
    assert_eq!(body.borrow().child_count(), 1);
    let text = body.borrow().first_child().unwrap();
    assert_eq!(text.borrow().text_content().unwrap(), "hi");
}

#[test]
fn content_without_body_auto_creates_body() {
    // Per §13.2.6.4.6 anything-else, text after </head> auto-creates <body>.
    let doc = parse("<!DOCTYPE html><head></head>hello");
    let body = find_element_by_name(&doc, "BODY").expect("missing <BODY>");
    assert_eq!(body.borrow().text_content().unwrap(), "hello");
}

#[test]
fn meta_after_head_is_processed_in_head_context() {
    // Per §13.2.6.4.6, <meta> after </head> is a parse error but still
    // processed using in-head rules. The <meta> should end up under <head>.
    let doc = parse("<!DOCTYPE html><head></head><meta charset=utf-8>");
    let meta = find_element_by_name(&doc, "META").expect("missing <META>");
    let parent = meta.borrow().parent_element().unwrap();
    assert_eq!(parent.borrow().node_name, "HEAD");
}

// ── Full prelude chain ─────────────────────────────────────────

#[test]
fn full_document_structure() {
    let doc = parse("<!DOCTYPE html><html><head><title>T</title></head><body><p>hi</p></body></html>");
    // Document children: DocumentType + html.
    assert_eq!(doc.borrow().child_count(), 2);
    // html children: head + body.
    let html = find_element_by_name(&doc, "HTML").unwrap();
    let html_children = child_names(&html);
    assert_eq!(html_children, vec!["HEAD", "BODY"]);
    // head has a title child.
    let head = find_element_by_name(&doc, "HEAD").unwrap();
    let title = head
        .borrow()
        .children
        .iter()
        .find(|c| c.borrow().node_name == "TITLE")
        .expect("missing <TITLE> under <HEAD>")
        .clone();
    assert_eq!(title.borrow().text_content().unwrap(), "T");
}

#[test]
fn attributes_are_preserved_on_html_element() {
    let doc = parse("<!DOCTYPE html><html lang=en><head></head></html>");
    let html = find_element_by_name(&doc, "HTML").unwrap();
    // Check the lang attribute via ElementData.
    let html_ref = html.borrow();
    if let NodeKind::Element(ref e) = html_ref.kind {
        let lang = e.get_attribute("lang").expect("missing lang attribute");
        assert_eq!(lang, "en");
    } else {
        panic!("expected Element node");
    }
}
