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

// ── Open elements stack helpers (§13.2.6.4.2) ─────────────────

/// The default scope set per §13.2.6.4.2. An element is "in scope" if it
/// appears on the open elements stack before any of these boundary names.
const DEFAULT_SCOPE: &[&str] = &[
    "applet",
    "caption",
    "html",
    "table",
    "td",
    "th",
    "marquee",
    "object",
    "template",
    // MathML / SVG foreign elements omitted — Phase 4 will add foreign content.
];

/// The list scope set: default scope + `ol` + `ul` (§13.2.6.4.2).
const LIST_SCOPE_EXTRA: &[&str] = &["ol", "ul"];

/// Return the local name (lowercase tag name) of an open element, or `None`
/// if the node is not an HTML-namespace element.
fn html_local_name(node: &Rc<RefCell<Node>>) -> Option<String> {
    let n = node.borrow();
    if let NodeKind::Element(ref e) = n.kind {
        if e.namespace == muskitty_dom::Namespace::Html {
            return Some(e.local_name.clone());
        }
    }
    None
}

/// Check whether an element with the given tag name is in scope (§13.2.6.4.2
/// "default scope").
pub fn has_element_in_scope(parser: &HtmlTreeConstructor, name: &str) -> bool {
    has_element_in_scope_with(parser, name, DEFAULT_SCOPE, &[])
}

/// Check whether an element with the given tag name is in *button scope*
/// (default scope + `button`).
pub fn has_element_in_button_scope(parser: &HtmlTreeConstructor, name: &str) -> bool {
    has_element_in_scope_with(parser, name, DEFAULT_SCOPE, &["button"])
}

/// Check whether an element with the given tag name is in *list scope*
/// (default scope + `ol` + `ul`).
pub fn has_element_in_list_scope(parser: &HtmlTreeConstructor, name: &str) -> bool {
    has_element_in_scope_with(parser, name, DEFAULT_SCOPE, LIST_SCOPE_EXTRA)
}

fn has_element_in_scope_with(
    parser: &HtmlTreeConstructor,
    name: &str,
    base_scope: &[&str],
    extra: &[&str],
) -> bool {
    for node in parser.open_elements.iter().rev() {
        let local = match html_local_name(node) {
            Some(l) => l,
            None => continue,
        };
        if local == name {
            return true;
        }
        // Boundary element encountered: target is not in scope.
        if base_scope.contains(&local.as_str()) || extra.contains(&local.as_str()) {
            return false;
        }
    }
    false
}

/// Generate implied end tags (§13.2.6.4.1).
///
/// Pop nodes from the open elements stack while the current node's name is
/// one of the implied-end-tag names. If `except` is `Some(name)`, that name
/// is not treated as an implied end tag (used by `</p>`/`</li>`/`</dd>`/`</dt>`
/// handling to avoid popping the target element prematurely).
pub fn generate_implied_end_tags(parser: &mut HtmlTreeConstructor, except: Option<&str>) {
    const IMPLIED_END: &[&str] = &[
        "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc", "td", "th", "tr",
    ];
    loop {
        let top_name = parser
            .open_elements
            .last()
            .and_then(html_local_name);
        match top_name.as_deref() {
            Some(n) if IMPLIED_END.contains(&n) && Some(n) != except => {
                parser.open_elements.pop();
            }
            _ => break,
        }
    }
}

/// "Close a p element" (§13.2.6.4.7).
///
/// Generate implied end tags for `p`; if the current node is not `p`, it is
/// a parse error. Pop nodes from the open elements stack until a `p` has
/// been popped. Stops at the `<html>` element as a safety net so a missing
/// `p` never empties the stack.
pub fn close_p_element(parser: &mut HtmlTreeConstructor) {
    generate_implied_end_tags(parser, Some("p"));
    // Per spec, current node should be p here; if not, parse error (ignored
    // in the skeleton — we still pop until p is gone).
    while let Some(top) = parser.open_elements.last() {
        let local = html_local_name(top);
        // Safety net: never pop past <html>.
        if local.as_deref() == Some("html") {
            break;
        }
        let is_p = local.as_deref() == Some("p");
        parser.open_elements.pop();
        if is_p {
            break;
        }
    }
}

/// "Reconstruct the active formatting elements" (§13.2.6.4.2).
///
/// This algorithm re-opens formatting elements (`<b>`, `<i>`, etc.) that
/// were closed implicitly when block elements were inserted, so that
/// subsequent text inherits the formatting. The full algorithm is complex;
/// Phase 3.3 will implement it. For Phase 3.2 (block-level elements, no
/// formatting elements), the stub is a no-op.
pub fn reconstruct_active_formatting_elements(_parser: &mut HtmlTreeConstructor) {
    // Phase 3.3: implement per §13.2.6.4.2.
}
