//! Tree construction helper algorithms.
//!
//! These functions implement the "insert a node", "create an element",
//! and related algorithms from WHATWG §13.2.6.2. They are used by the
//! insertion mode handlers in [`super::dispatch`].

use std::cell::RefCell;
use std::rc::Rc;

use muskitty_dom::{append_child, Attribute, Node, NodeKind, NodeType};

use super::HtmlTreeConstructor;
use crate::tokenizer::TagToken;

/// Create an Element node for a start tag token.
///
/// Implements "create an element for the token" (§13.2.6.2) in a simplified
/// form: always uses the HTML namespace, no custom element definitions, no
/// attribute adjustment. Full foreign-attribute adjustment (§13.2.6.5) is
/// deferred to Phase 3.
pub fn create_element_for_token(
    parser: &HtmlTreeConstructor,
    token: &TagToken,
) -> Rc<RefCell<Node>> {
    let attrs: Vec<Attribute> = token
        .attrs
        .iter()
        .map(|(name, value)| Attribute::new(name, value))
        .collect();
    Node::new_element_html(&token.name, attrs, &parser.document)
}

/// Insert a node at the appropriate place for inserting a node.
///
/// Per §13.2.6.2, the appropriate place is the current node (top of the
/// open elements stack), unless foster parenting is active. Foster
/// parenting is deferred to Phase 4; this skeleton always inserts at the
/// current node.
pub fn insert_node(parser: &HtmlTreeConstructor, node: &Rc<RefCell<Node>>) {
    let current = parser.current_node();
    let _ = append_child(&current, node.clone());
}

/// Create an element for the token, insert it, and push it onto the open
/// elements stack.
///
/// This is the common "insert an element" sequence used by most insertion
/// modes when they encounter a start tag. Currently unused by the skeleton
/// handlers (which use `create_and_push` for attribute-less elements); the
/// InBody batch in Phase 3.2 will route start-tag handling through this.
#[allow(dead_code)]
pub fn insert_element(parser: &mut HtmlTreeConstructor, token: &TagToken) {
    let element = create_element_for_token(parser, token);
    insert_node(parser, &element);
    parser.open_elements.push(element);
}

/// Insert a character token at the current node.
///
/// Per §13.2.6.2, if the current node's last child is a Text node, the
/// character is appended to that Text node's data. Otherwise, a new Text
/// node is created and inserted.
pub fn insert_character(parser: &HtmlTreeConstructor, c: char) {
    let current = parser.current_node();
    let last_child = current.borrow().last_child();
    if let Some(child) = last_child {
        let is_text = child.borrow().node_type == NodeType::Text;
        if is_text {
            if let NodeKind::Text(ref mut t) = child.borrow_mut().kind {
                t.data.push(c);
                return;
            }
        }
    }
    let text = Node::new_text(&c.to_string(), &parser.document);
    let _ = append_child(&current, text);
}

/// Insert a comment node as a child of the current node.
///
/// Per §13.2.6.2, the exact insertion point depends on the insertion mode
/// (some modes insert comments at the Document, others at the html element).
/// This helper always inserts at the current node; insertion modes that
/// need a different target should use [`insert_comment_at`] instead.
pub fn insert_comment(parser: &HtmlTreeConstructor, data: &str) {
    let comment = Node::new_comment(data, &parser.document);
    insert_node(parser, &comment);
}

/// Insert a comment node as a child of the specified target node.
///
/// Used by insertion modes that require comments to go to a specific node
/// (e.g., Document or html element) rather than the current node.
pub fn insert_comment_at(target: &Rc<RefCell<Node>>, data: &str, document: &Rc<RefCell<Node>>) {
    let comment = Node::new_comment(data, document);
    let _ = append_child(target, comment);
}
