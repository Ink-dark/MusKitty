//! Tree construction helper algorithms.
//!
//! These functions implement the "insert a node", "create an element",
//! and related algorithms from WHATWG §13.2.6.2. They are used by the
//! insertion mode handlers in [`super::dispatch`].

use std::cell::RefCell;
use std::rc::Rc;

use muskitty_dom::{append_child, Attribute, Node, NodeKind, NodeType};

use super::ActiveFormattingEntry;
use super::HtmlTreeConstructor;
use crate::error::ParseError;
use crate::tokenizer::TagToken;

/// Create an Element node for a start tag token.
///
/// Implements "create an element for the token" (§13.2.6.2) in a simplified
/// form: always uses the HTML namespace, no custom element definitions, no
/// attribute adjustment. Full foreign-attribute adjustment (§13.2.6.5) is
/// deferred to Phase 3.
///
/// Per §13.2.6.2, when creating a `<template>` element a DocumentFragment is
/// also created and attached as the template's content. All nodes inserted
/// while the template is the current node go into this content fragment.
pub fn create_element_for_token(
    parser: &HtmlTreeConstructor,
    token: &TagToken,
) -> Rc<RefCell<Node>> {
    let attrs: Vec<Attribute> = token
        .attrs
        .iter()
        .map(|(name, value)| Attribute::new(name, value))
        .collect();
    let element = Node::new_element_html(&token.name, attrs, &parser.document);
    if token.name == "template" {
        let content = Node::new_document_fragment(&parser.document);
        if let NodeKind::Element(ref mut e) = element.borrow_mut().kind {
            e.set_template_content(content);
        }
    }
    element
}

/// Determine the target node for insertion per §13.2.6.2.
///
/// If the current node is a `<template>` element, returns the template's
/// content DocumentFragment; otherwise returns the current node itself.
/// Foster parenting is handled separately by the caller via `foster_parent`.
fn insertion_target(parser: &HtmlTreeConstructor) -> Rc<RefCell<Node>> {
    let current = parser.current_node();
    let target = {
        let current_ref = current.borrow();
        match &current_ref.kind {
            NodeKind::Element(e) if e.local_name == "template" => e.template_content.clone(),
            _ => None,
        }
    };
    target.unwrap_or_else(|| current.clone())
}

/// Check whether foster parenting should actually apply for the current
/// insertion. Per §13.2.6.3, foster parenting only takes effect when the
/// foster-parenting flag is enabled **and** the target (current node) is a
/// `table`, `tbody`, `tfoot`, `thead`, or `tr` element. If the current
/// node is some other element (e.g. a `<plaintext>` that was foster-parented
/// onto the stack before a table), normal insertion applies.
fn foster_parenting_active(parser: &HtmlTreeConstructor) -> bool {
    if !parser.foster_parenting {
        return false;
    }
    parser
        .open_elements
        .last()
        .and_then(|n| {
            n.borrow().kind.as_element().map(|e| {
                matches!(
                    e.local_name.as_str(),
                    "table" | "tbody" | "tfoot" | "thead" | "tr"
                )
            })
        })
        .unwrap_or(false)
}

/// Insert a node at the appropriate place for inserting a node (§13.2.6.2).
///
/// When foster parenting is active (§13.2.6.3), the node is inserted at
/// the foster parenting position: before the last table element in its
/// parent's children list. Otherwise, the node is appended to the current
/// node (or template content if current node is a `<template>`).
pub fn insert_node(parser: &HtmlTreeConstructor, node: &Rc<RefCell<Node>>) {
    if foster_parenting_active(parser) {
        let (parent, table) = foster_parent_position(parser);
        if let Some(table_node) = table {
            let _ = muskitty_dom::insert_before(&parent, node.clone(), Some(&table_node));
        } else {
            let _ = append_child(&parent, node.clone());
        }
    } else {
        let target = insertion_target(parser);
        let _ = append_child(&target, node.clone());
    }
}

/// Find the foster parent and the table element to insert before
/// (§13.2.6.3).
///
/// Returns (parent, Some(table_node)) where `parent` is the element below
/// the last table on the stack, and `table_node` is the table element to
/// insert before. If no table is on the stack, returns (current_node, None).
fn foster_parent_position(
    parser: &HtmlTreeConstructor,
) -> (Rc<RefCell<Node>>, Option<Rc<RefCell<Node>>>) {
    let table_index = parser
        .open_elements
        .iter()
        .rev()
        .position(|n| {
            n.borrow()
                .kind
                .as_element()
                .map(|e| e.local_name == "table")
                .unwrap_or(false)
        })
        .map(|i| parser.open_elements.len() - 1 - i);

    if let Some(idx) = table_index {
        let table_node = parser.open_elements[idx].clone();
        let parent = if idx > 0 {
            parser.open_elements[idx - 1].clone()
        } else {
            parser.document.clone()
        };
        (parent, Some(table_node))
    } else {
        (parser.current_node(), None)
    }
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
///
/// If the current node is a `<template>`, the character is inserted into
/// the template's content DocumentFragment (via `insertion_target`).
///
/// When foster parenting is active (§13.2.6.3), the character is inserted
/// at the foster parenting position (before the last table element on the
/// stack). If the node before the table is a Text node, the character is
/// appended to it.
pub fn insert_character(parser: &HtmlTreeConstructor, c: char) {
    if foster_parenting_active(parser) {
        // Find foster parent position and check if the previous sibling
        // of the table is a Text node we can append to.
        let (parent, table) = foster_parent_position(parser);
        if let Some(table_node) = table {
            // Find the previous sibling of the table node.
            let prev_sibling = {
                let children = parent.borrow();
                let idx = children
                    .children
                    .iter()
                    .position(|n| Rc::ptr_eq(n, &table_node));
                idx.and_then(|i| {
                    if i > 0 {
                        Some(children.children[i - 1].clone())
                    } else {
                        None
                    }
                })
            };
            if let Some(prev) = prev_sibling {
                if prev.borrow().node_type == NodeType::Text {
                    if let NodeKind::Text(ref mut t) = prev.borrow_mut().kind {
                        t.data.push(c);
                        return;
                    }
                }
            }
            // Create a new Text node before the table.
            let text = Node::new_text(&c.to_string(), &parser.document);
            let _ = muskitty_dom::insert_before(&parent, text, Some(&table_node));
        } else {
            // No table on stack: fall back to appending to foster parent.
            let last_child = parent.borrow().last_child();
            if let Some(child) = last_child {
                if child.borrow().node_type == NodeType::Text {
                    if let NodeKind::Text(ref mut t) = child.borrow_mut().kind {
                        t.data.push(c);
                        return;
                    }
                }
            }
            let text = Node::new_text(&c.to_string(), &parser.document);
            let _ = append_child(&parent, text);
        }
        return;
    }
    let target = insertion_target(parser);
    let last_child = target.borrow().last_child();
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
    let _ = append_child(&target, text);
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

/// Insert a ProcessingInstruction node at the adjusted insertion location
/// (§13.2.6.2 "insert a processing instruction").
///
/// Per the spec, a ProcessingInstruction node with the token's target and
/// data is created and inserted at the same location a comment would go.
/// Foster parenting applies as normal for `insert_node`.
pub fn insert_processing_instruction(parser: &HtmlTreeConstructor, target: &str, data: &str) {
    let pi = Node::new_processing_instruction(target, data, &parser.document);
    insert_node(parser, &pi);
}

/// Insert a ProcessingInstruction node as a child of the specified target
/// node (variant of `insert_comment_at` for PI tokens emitted before the
/// html element exists, e.g. in Initial/BeforeHtml modes).
pub fn insert_processing_instruction_at(
    target: &Rc<RefCell<Node>>,
    target_name: &str,
    data: &str,
    document: &Rc<RefCell<Node>>,
) {
    let pi = Node::new_processing_instruction(target_name, data, document);
    let _ = append_child(target, pi);
}

// ── Quirks mode detection (§13.2.6.4.1) ────────────────────────

/// ASCII case-insensitive "starts with" check (§13.2.6.4.1 line 3165
/// requires ASCII case-insensitive comparison of public/system IDs).
fn ascii_starts_with(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

/// Public identifiers whose exact value (ASCII case-insensitive) triggers
/// quirks mode (§13.2.6.4.1, the first list, exact-match entries).
const QUIRKS_PUBLIC_ID_EXACT: &[&str] = &[
    "-//W3O//DTD W3 HTML Strict 3.0//EN//",
    "-/W3C/DTD HTML 4.0 Transitional/EN",
    "HTML",
];

/// The single system identifier whose exact value triggers quirks mode
/// (§13.2.6.4.1, the first list).
const QUIRKS_SYSTEM_ID_EXACT: &str = "http://www.ibm.com/data/dtd/v11/ibmxhtml1-transitional.dtd";

/// Public identifier prefixes that trigger quirks mode
/// (§13.2.6.4.1, the first list, "starts with" entries).
const QUIRKS_PUBLIC_ID_PREFIX: &[&str] = &[
    "+//Silmaril//dtd html Pro v0r11 19970101//",
    "-//AS//DTD HTML 3.0 asWedit + extensions//",
    "-//AdvaSoft Ltd//DTD HTML 3.0 asWedit + extensions//",
    "-//IETF//DTD HTML 2.0 Level 1//",
    "-//IETF//DTD HTML 2.0 Level 2//",
    "-//IETF//DTD HTML 2.0 Strict Level 1//",
    "-//IETF//DTD HTML 2.0 Strict Level 2//",
    "-//IETF//DTD HTML 2.0 Strict//",
    "-//IETF//DTD HTML 2.0//",
    "-//IETF//DTD HTML 2.1E//",
    "-//IETF//DTD HTML 3.0//",
    "-//IETF//DTD HTML 3.2 Final//",
    "-//IETF//DTD HTML 3.2//",
    "-//IETF//DTD HTML 3//",
    "-//IETF//DTD HTML Level 0//",
    "-//IETF//DTD HTML Level 1//",
    "-//IETF//DTD HTML Level 2//",
    "-//IETF//DTD HTML Level 3//",
    "-//IETF//DTD HTML Strict Level 0//",
    "-//IETF//DTD HTML Strict Level 1//",
    "-//IETF//DTD HTML Strict Level 2//",
    "-//IETF//DTD HTML Strict Level 3//",
    "-//IETF//DTD HTML Strict//",
    "-//IETF//DTD HTML//",
    "-//Metrius//DTD Metrius Presentational//",
    "-//Microsoft//DTD Internet Explorer 2.0 HTML Strict//",
    "-//Microsoft//DTD Internet Explorer 2.0 HTML//",
    "-//Microsoft//DTD Internet Explorer 2.0 Tables//",
    "-//Microsoft//DTD Internet Explorer 3.0 HTML Strict//",
    "-//Microsoft//DTD Internet Explorer 3.0 HTML//",
    "-//Microsoft//DTD Internet Explorer 3.0 Tables//",
    "-//Netscape Comm. Corp.//DTD HTML//",
    "-//Netscape Comm. Corp.//DTD Strict HTML//",
    "-//O'Reilly and Associates//DTD HTML 2.0//",
    "-//O'Reilly and Associates//DTD HTML Extended 1.0//",
    "-//O'Reilly and Associates//DTD HTML Extended Relaxed 1.0//",
    "-//SQ//DTD HTML 2.0 HoTMetaL + extensions//",
    "-//SoftQuad Software//DTD HoTMetaL PRO 6.0::19990601::extensions to HTML 4.0//",
    "-//SoftQuad//DTD HoTMetaL PRO 4.0::19971010::extensions to HTML 4.0//",
    "-//Spyglass//DTD HTML 2.0 Extended//",
    "-//Sun Microsystems Corp.//DTD HotJava HTML//",
    "-//Sun Microsystems Corp.//DTD HotJava Strict HTML//",
    "-//W3C//DTD HTML 3 1995-03-24//",
    "-//W3C//DTD HTML 3.2 Draft//",
    "-//W3C//DTD HTML 3.2 Final//",
    "-//W3C//DTD HTML 3.2//",
    "-//W3C//DTD HTML 3.2S Draft//",
    "-//W3C//DTD HTML 4.0 Frameset//",
    "-//W3C//DTD HTML 4.0 Transitional//",
    "-//W3C//DTD HTML Experimental 19960712//",
    "-//W3C//DTD HTML Experimental 970421//",
    "-//W3C//DTD W3 HTML//",
    "-//W3O//DTD W3 HTML 3.0//",
    "-//WebTechs//DTD Mozilla HTML 2.0//",
    "-//WebTechs//DTD Mozilla HTML//",
];

/// Public identifier prefixes that trigger quirks mode only when the system
/// identifier is missing or the empty string (§13.2.6.4.1, first list).
const QUIRKS_PUBLIC_ID_PREFIX_WHEN_SYS_MISSING: &[&str] = &[
    "-//W3C//DTD HTML 4.01 Frameset//",
    "-//W3C//DTD HTML 4.01 Transitional//",
];

/// Detect whether a DOCTYPE token puts the Document into quirks mode,
/// per §13.2.6.4.1 (the "force-quirks" / public-id / system-id checks).
///
/// Returns `true` for full quirks mode. Limited-quirks mode is not tracked
/// separately because no implemented insertion-mode rule distinguishes it
/// from no-quirks (the only current consumer — InBody `<table>` closing `<p>`
/// — treats limited-quirks as "not quirks").
pub fn detect_quirks_mode(
    force_quirks: bool,
    name: Option<&str>,
    public_id: Option<&str>,
    system_id: Option<&str>,
) -> bool {
    // force-quirks flag, or name is not "html".
    if force_quirks || name != Some("html") {
        return true;
    }
    let pid = public_id.unwrap_or("");
    let sid = system_id.unwrap_or("");
    let sid_missing_or_empty = system_id.map(|s| s.is_empty()).unwrap_or(true);

    // Exact public identifier matches.
    if QUIRKS_PUBLIC_ID_EXACT
        .iter()
        .any(|p| pid.eq_ignore_ascii_case(p))
    {
        return true;
    }
    // Exact system identifier match.
    if sid.eq_ignore_ascii_case(QUIRKS_SYSTEM_ID_EXACT) {
        return true;
    }
    // Public identifier prefix matches.
    if QUIRKS_PUBLIC_ID_PREFIX
        .iter()
        .any(|p| ascii_starts_with(pid, p))
    {
        return true;
    }
    // System identifier missing/empty + public identifier prefix.
    if sid_missing_or_empty
        && QUIRKS_PUBLIC_ID_PREFIX_WHEN_SYS_MISSING
            .iter()
            .any(|p| ascii_starts_with(pid, p))
    {
        return true;
    }
    false
}

// ── Open elements stack helpers (§13.2.6.4.2) ─────────────────

/// The default scope set per §13.2.6.4.2. An element is "in scope" if it
/// appears on the open elements stack before any of these boundary names.
const DEFAULT_SCOPE: &[&str] = &[
    "applet", "caption", "html", "table", "td", "th", "marquee", "object",
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

/// The table scope set (§13.2.6.4.2): only `html`, `table`, `template`.
const TABLE_SCOPE: &[&str] = &["html", "table", "template"];

/// Check whether an element with the given tag name is in *table scope*
/// (§13.2.6.4.2: boundary set = `html` + `table` + `template`).
pub fn has_element_in_table_scope(parser: &HtmlTreeConstructor, name: &str) -> bool {
    for node in parser.open_elements.iter().rev() {
        let local = match html_local_name(node) {
            Some(l) => l,
            None => continue,
        };
        if local == name {
            return true;
        }
        if TABLE_SCOPE.contains(&local.as_str()) {
            return false;
        }
    }
    false
}

/// Check whether an element with the given tag name is on the stack of
/// open elements (no scope boundaries — just a plain stack search).
/// Used by `</template>` handling per §13.2.6.4.5.
pub fn has_element_in_stack(parser: &HtmlTreeConstructor, name: &str) -> bool {
    parser
        .open_elements
        .iter()
        .any(|n| html_local_name(n).as_deref() == Some(name))
}

/// Close a list item (`<li>`) or definition term (`<dd>`/`<dt>`) per the
/// loop algorithm in §13.2.6.4.7.
///
/// Walks the stack of open elements from top to bottom. If any of the
/// `targets` is found, generates implied end tags except for that element,
/// then pops until that element is popped. The walk stops early if a
/// special element that is not `address`, `div`, or `p` is encountered.
///
/// For `<li>`, pass `&["li"]`. For `<dd>`/`<dt>`, pass `&["dd", "dt"]`
/// (the spec checks for both regardless of which start tag triggered it).
///
/// Returns `true` if a target was found and popped; `false` otherwise.
pub fn close_list_item(parser: &mut HtmlTreeConstructor, targets: &[&str]) -> bool {
    // Walk from top of stack downward.
    for i in (0..parser.open_elements.len()).rev() {
        let local = match html_local_name(&parser.open_elements[i]) {
            Some(l) => l,
            None => continue,
        };
        let target = targets.iter().find(|t| local == **t);
        if let Some(target) = target {
            let target = target.to_string();
            // Found the target. Generate implied end tags except for target.
            generate_implied_end_tags(parser, Some(&target));
            // If current node is not target, parse error.
            let current_is_target = parser
                .open_elements
                .last()
                .and_then(|n| html_local_name(n))
                .map(|l| l == target)
                .unwrap_or(false);
            if !current_is_target {
                parser
                    .errors
                    .push(ParseError::Generic("list item not at current node"));
            }
            // Pop until target is popped.
            while let Some(top) = parser.open_elements.last() {
                let top_name = html_local_name(top);
                parser.open_elements.pop();
                if top_name.as_deref() == Some(target.as_str()) {
                    break;
                }
            }
            return true;
        }
        // If node is special but not address/div/p, stop.
        if SPECIAL_ELEMENTS.contains(&local.as_str())
            && !matches!(local.as_str(), "address" | "div" | "p")
        {
            return false;
        }
        // Otherwise: continue to previous entry.
    }
    false
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
        "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc",
    ];
    loop {
        let top_name = parser.open_elements.last().and_then(html_local_name);
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

/// "Push onto the list of active formatting elements" (§13.2.6.2).
///
/// Adds `element` to the list of active formatting elements, applying the
/// Noah's Ark clause: if there are already three entries in the list (after
/// the last marker) that are elements with the same tag name, namespace, and
/// attributes as `element`, the earliest such entry is dropped before pushing.
pub fn push_formatting_element(parser: &mut HtmlTreeConstructor, element: Rc<RefCell<Node>>) {
    // Noah's Ark clause (§13.2.6.2): count matching entries after the last
    // marker. If there are already 3, drop the first one before pushing.
    let mut count = 0;
    let mut first_match_index: Option<usize> = None;
    for (i, entry) in parser.active_formatting_elements.iter().enumerate().rev() {
        match entry {
            ActiveFormattingEntry::Marker => break,
            ActiveFormattingEntry::Element(e) => {
                if elements_match_for_noahs_ark(e, &element) {
                    count += 1;
                    if count == 1 {
                        first_match_index = Some(i);
                    }
                    if count == 3 {
                        // Drop the earliest (first_match_index is the lowest
                        // index found while iterating in reverse, so it is
                        // actually the earliest in list order).
                        if let Some(idx) = first_match_index {
                            parser.active_formatting_elements.remove(idx);
                        }
                        break;
                    }
                }
            }
        }
    }
    parser
        .active_formatting_elements
        .push(ActiveFormattingEntry::Element(element));
}

/// Add a marker to the list of active formatting elements (§13.2.6.2).
///
/// Markers delimit sections of the list; they are pushed when entering
/// table contexts, template content, etc. (Phase 3.4/3.5).
#[allow(dead_code)]
pub fn add_formatting_marker(parser: &mut HtmlTreeConstructor) {
    parser
        .active_formatting_elements
        .push(ActiveFormattingEntry::Marker);
}

/// "Clear the list of active formatting elements up to the last marker"
/// (§13.2.6.2).
///
/// Remove entries from the end of the list until a marker has been removed
/// (or the list is empty). Used by table/template modes (Phase 3.4/3.5).
#[allow(dead_code)]
pub fn clear_active_formatting_to_last_marker(parser: &mut HtmlTreeConstructor) {
    while let Some(entry) = parser.active_formatting_elements.pop() {
        if matches!(entry, ActiveFormattingEntry::Marker) {
            break;
        }
    }
}

/// Compare two elements for the Noah's Ark clause (§13.2.6.2).
///
/// Two elements match if they have the same tag name, namespace, and
/// attributes (count and values). This is used to limit the list to at
/// most three consecutive equivalent formatting elements.
fn elements_match_for_noahs_ark(a: &Rc<RefCell<Node>>, b: &Rc<RefCell<Node>>) -> bool {
    let a = a.borrow();
    let b = b.borrow();
    let (ea, eb) = match (a.kind.as_element(), b.kind.as_element()) {
        (Some(ea), Some(eb)) => (ea, eb),
        _ => return false,
    };
    if ea.namespace != eb.namespace {
        return false;
    }
    if ea.local_name != eb.local_name {
        return false;
    }
    if ea.attributes.len() != eb.attributes.len() {
        return false;
    }
    // Attributes are compared in order; HTML tree construction preserves
    // attribute order so this is sufficient.
    ea.attributes
        .iter()
        .zip(eb.attributes.iter())
        .all(|(x, y)| x.local_name == y.local_name && x.value == y.value)
}

/// "Reconstruct the active formatting elements" (§13.2.6.4.2).
///
/// Re-opens formatting elements (`<b>`, `<i>`, etc.) that were closed
/// implicitly when block elements were inserted, so that subsequent text or
/// inline elements inherit the formatting. The algorithm walks the list
/// from the end, finds the first entry that is either a marker or already
/// on the open elements stack, then re-inserts each formatting element
/// after that point (cloning the element with no attributes change).
pub fn reconstruct_active_formatting_elements(parser: &mut HtmlTreeConstructor) {
    // If the list is empty, nothing to do.
    if parser.active_formatting_elements.is_empty() {
        return;
    }
    // If the last entry is a marker, nothing to do.
    if matches!(
        parser.active_formatting_elements.last(),
        Some(ActiveFormattingEntry::Marker)
    ) {
        return;
    }
    // If the last entry's element is the current node (top of open elements
    // stack), nothing to do.
    if let Some(ActiveFormattingEntry::Element(el)) = parser.active_formatting_elements.last() {
        if let Some(top) = parser.open_elements.last() {
            if Rc::ptr_eq(el, top) {
                return;
            }
        }
    }

    // Walk backward from the end to find the "first" entry (in spec terms,
    // the furthest from the end) that is a marker or on the open elements
    // stack. We then re-insert everything after it.
    let mut index = parser.active_formatting_elements.len();
    loop {
        if index == 0 {
            // Reached the start of the list; all entries need reconstruction.
            break;
        }
        index -= 1;
        let on_stack_or_marker = match &parser.active_formatting_elements[index] {
            ActiveFormattingEntry::Marker => true,
            ActiveFormattingEntry::Element(el) => {
                parser.open_elements.iter().any(|o| Rc::ptr_eq(o, el))
            }
        };
        if on_stack_or_marker {
            // Step past this entry; reconstruction starts at index+1.
            index += 1;
            break;
        }
    }

    // Re-insert each entry from `index` to the end. New elements are cloned
    // (same tag, same attributes) and pushed onto both the open elements
    // stack and the active formatting list (replacing the old entry).
    while index < parser.active_formatting_elements.len() {
        let entry = parser.active_formatting_elements[index].clone();
        let old_element = match entry {
            ActiveFormattingEntry::Element(e) => e,
            ActiveFormattingEntry::Marker => {
                index += 1;
                continue;
            }
        };
        // Clone the element: same tag name and attributes, fresh node.
        let (local_name, attrs) = {
            let o = old_element.borrow();
            let e = o
                .kind
                .as_element()
                .expect("formatting entry not an element");
            (
                e.local_name.clone(),
                e.attributes
                    .iter()
                    .map(|a| Attribute::new(&a.local_name, &a.value))
                    .collect::<Vec<_>>(),
            )
        };
        let new_element = Node::new_element_html(&local_name, attrs, &parser.document);
        let current = parser.current_node();
        let _ = append_child(&current, new_element.clone());
        parser.open_elements.push(new_element.clone());
        // Replace the entry with the new element.
        parser.active_formatting_elements[index] = ActiveFormattingEntry::Element(new_element);
        index += 1;
    }
}

// ── Adoption agency algorithm (§13.2.6.4.7) ───────────────────

/// The "special" elements per §13.2.6.3 (HTML-namespace subset). Used by
/// the adoption agency algorithm to find the furthest block, and by the
/// "any other end tag" branch to decide when to ignore an end tag.
pub const SPECIAL_ELEMENTS: &[&str] = &[
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "basefont",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "keygen",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "search",
    "script",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
    "wbr",
    "xmp",
];

/// Find the last entry in the active formatting list (scanning from the end
/// to the last marker) whose element has the given tag name. Returns its
/// index in the list, or `None`.
fn find_formatting_element(parser: &HtmlTreeConstructor, name: &str) -> Option<usize> {
    for (i, entry) in parser.active_formatting_elements.iter().enumerate().rev() {
        match entry {
            ActiveFormattingEntry::Marker => return None,
            ActiveFormattingEntry::Element(el) => {
                let is_match = el
                    .borrow()
                    .kind
                    .as_element()
                    .map(|e| e.local_name == name)
                    .unwrap_or(false);
                if is_match {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// Find the index of a node in the open elements stack, comparing by `Rc`
/// pointer identity.
fn stack_index_of(parser: &HtmlTreeConstructor, node: &Rc<RefCell<Node>>) -> Option<usize> {
    parser
        .open_elements
        .iter()
        .position(|n| Rc::ptr_eq(n, node))
}

/// Find the index of an active-formatting entry's element in the open
/// elements stack, comparing by `Rc` pointer identity.
fn stack_index_of_afe(parser: &HtmlTreeConstructor, afe_index: usize) -> Option<usize> {
    match &parser.active_formatting_elements[afe_index] {
        ActiveFormattingEntry::Element(el) => stack_index_of(parser, el),
        ActiveFormattingEntry::Marker => None,
    }
}

/// Clone an element's tag name and attributes into a fresh node (used by the
/// adoption agency algorithm to recreate formatting elements).
fn clone_element(parser: &HtmlTreeConstructor, src: &Rc<RefCell<Node>>) -> Rc<RefCell<Node>> {
    let (local_name, attrs) = {
        let s = src.borrow();
        let e = s
            .kind
            .as_element()
            .expect("formatting entry not an element");
        (
            e.local_name.clone(),
            e.attributes
                .iter()
                .map(|a| Attribute::new(&a.local_name, &a.value))
                .collect::<Vec<_>>(),
        )
    };
    Node::new_element_html(&local_name, attrs, &parser.document)
}

/// The adoption agency algorithm (§13.2.6.4.7).
///
/// Handles end tags for formatting elements (`</b>`, `</i>`, `</a>`, etc.)
/// and the special misnested-tag cases that arise when block elements
/// interleave with formatting elements. Implements the full 8-iteration
/// outer loop with the inner "reconstruct" loop.
pub fn adoption_agency(parser: &mut HtmlTreeConstructor, subject: &str) {
    // Step 2: If the current node is an HTML element whose tag name is
    // subject, and the current node is not in the list of active formatting
    // elements, then pop the current node off the stack of return.
    if let Some(top) = parser.open_elements.last() {
        let is_subject = top
            .borrow()
            .kind
            .as_element()
            .map(|e| e.local_name == subject)
            .unwrap_or(false);
        if is_subject {
            let in_afe = parser
                .active_formatting_elements
                .iter()
                .any(|e| matches!(e, ActiveFormattingEntry::Element(el) if Rc::ptr_eq(el, top)));
            if !in_afe {
                parser.open_elements.pop();
                return;
            }
        }
    }

    // Outer loop: at most 8 iterations.
    for _ in 0..8 {
        // 4.1: Find the formatting element (last AFE entry with tag=subject,
        //      between end and last marker).
        let mut formatting_afe_index = match find_formatting_element(parser, subject) {
            Some(i) => i,
            None => {
                // No formatting element: run "any other end tag" behavior.
                run_any_other_end_tag(parser, subject);
                return;
            }
        };

        // 4.2: If formatting element is not on the open elements stack,
        //      parse error, remove from list, return.
        let mut fmt_stack_index = match stack_index_of_afe(parser, formatting_afe_index) {
            Some(i) => i,
            None => {
                parser.errors.push(ParseError::Generic(
                    "adoption agency: formatting element not on stack",
                ));
                parser
                    .active_formatting_elements
                    .remove(formatting_afe_index);
                return;
            }
        };

        // 4.3: If formatting element is in the stack but not in scope,
        //      parse error, return.
        if !has_element_in_scope(parser, subject) {
            parser.errors.push(ParseError::Generic(
                "adoption agency: formatting element not in scope",
            ));
            return;
        }

        // 4.4: If formatting element is not the current node, parse error
        //      (not fatal).
        if let Some(top) = parser.open_elements.last() {
            let is_fmt = match &parser.active_formatting_elements[formatting_afe_index] {
                ActiveFormattingEntry::Element(el) => Rc::ptr_eq(el, top),
                _ => false,
            };
            if !is_fmt {
                parser.errors.push(ParseError::Generic(
                    "adoption agency: formatting element not current",
                ));
            }
        }

        // 4.5: Find the furthest block — per §13.2.6.4.7 step 5, "the topmost
        //      node in the stack of open elements that is lower in the stack
        //      than formatting element, and is an element in the special
        //      category." The stack grows downwards (root at index 0, current
        //      node at the end), so "lower than formatting element" means a
        //      HIGHER index. "Topmost" among those is the smallest such index,
        //      i.e. the first special element found scanning forward from
        //      fmt_stack_index+1.
        let furthest_block_index = parser.open_elements[fmt_stack_index + 1..]
            .iter()
            .enumerate()
            .find(|(_, n)| {
                n.borrow()
                    .kind
                    .as_element()
                    .map(|e| SPECIAL_ELEMENTS.contains(&e.local_name.as_str()))
                    .unwrap_or(false)
            })
            .map(|(i, _)| fmt_stack_index + 1 + i);

        let mut furthest_block_index = match furthest_block_index {
            Some(i) => i,
            None => {
                // No furthest block: pop elements off the stack until the
                // formatting element is popped, remove it from the AFE list,
                // and return.
                while let Some(top) = parser.open_elements.pop() {
                    let is_fmt = match &parser.active_formatting_elements[formatting_afe_index] {
                        ActiveFormattingEntry::Element(el) => Rc::ptr_eq(el, &top),
                        _ => false,
                    };
                    if is_fmt {
                        break;
                    }
                }
                parser
                    .active_formatting_elements
                    .remove(formatting_afe_index);
                return;
            }
        };

        // 4.6: Common ancestor — the element immediately below the
        //      formatting element in the stack.
        let common_ancestor = parser
            .open_elements
            .get(fmt_stack_index.wrapping_sub(1))
            .cloned();

        // 4.7: Bookmark — note the position of the formatting element in
        //      the AFE list. We track it as an index, adjusting for removals.
        let mut bookmark = formatting_afe_index;

        // 4.8: Inner loop.
        let mut node_index = furthest_block_index;
        let furthest_block_node = parser.open_elements[furthest_block_index].clone();
        let mut last_node = furthest_block_node.clone();

        let mut inner_iterations = 0;
        loop {
            inner_iterations += 1;
            if inner_iterations > 64 {
                // Safety valve against infinite loops on malformed state.
                break;
            }
            // node = element immediately above the current node in the stack.
            if node_index == 0 {
                break;
            }
            node_index -= 1;
            let node = parser.open_elements[node_index].clone();

            // If node is the formatting element, end inner loop.
            let is_fmt = match &parser.active_formatting_elements[formatting_afe_index] {
                ActiveFormattingEntry::Element(el) => Rc::ptr_eq(el, &node),
                _ => false,
            };
            if is_fmt {
                break;
            }

            // Is node in the AFE list?
            let node_afe_index = parser.active_formatting_elements.iter().position(
                |e| matches!(e, ActiveFormattingEntry::Element(el) if Rc::ptr_eq(el, &node)),
            );

            match node_afe_index {
                None => {
                    // Remove node from the stack.
                    parser.open_elements.remove(node_index);
                    // Adjust furthest_block_index / fmt_stack_index if needed.
                    if node_index < fmt_stack_index {
                        fmt_stack_index_will_decrement(&mut fmt_stack_index);
                    }
                    if node_index < furthest_block_index {
                        furthest_block_index -= 1;
                    }
                }
                Some(afe_idx) => {
                    // §13.2.6.4.7 step 13.4: If innerLoopCounter > 3 and
                    // node is in the AFE list, remove it from the AFE list
                    // (and then from the stack via the "not in AFE" path).
                    // This prevents excessive cloning of formatting elements
                    // in deeply nested misnested content.
                    if inner_iterations > 3 {
                        parser.active_formatting_elements.remove(afe_idx);
                        // Adjust bookmark if the removed entry was before it.
                        if afe_idx < bookmark {
                            bookmark -= 1;
                        }
                        // Adjust formatting_afe_index if the removed entry
                        // was before the formatting element.
                        if afe_idx < formatting_afe_index {
                            formatting_afe_index -= 1;
                        }
                        // Now treat as "not in AFE": remove from stack.
                        parser.open_elements.remove(node_index);
                        if node_index < fmt_stack_index {
                            fmt_stack_index_will_decrement(&mut fmt_stack_index);
                        }
                        if node_index < furthest_block_index {
                            furthest_block_index -= 1;
                        }
                        continue;
                    }
                    // Step 13.6: Create a new element with the same tag/attrs
                    // as node, append last_node to it, replace node in both
                    // the stack and the AFE list, set last_node = new element.
                    let new_element = clone_element(parser, &node);
                    let _ = append_child(&new_element, last_node.clone());
                    // Replace node in the open elements stack.
                    parser.open_elements[node_index] = new_element.clone();
                    // Replace node in the AFE list.
                    parser.active_formatting_elements[afe_idx] =
                        ActiveFormattingEntry::Element(new_element.clone());
                    // Step 13.7: If lastNode is furthestBlock, move the
                    // bookmark to be immediately after the new node in the
                    // AFE list.
                    if Rc::ptr_eq(&last_node, &furthest_block_node) {
                        bookmark = afe_idx + 1;
                    }
                    last_node = new_element;
                }
            }
        }

        // 4.9: Insert last_node into common ancestor, fostering if needed.
        //      For the non-table case, append last_node to common ancestor.
        if let Some(ancestor) = common_ancestor {
            // If last_node is already a child of ancestor (shouldn't be, but
            // guard against double-append), skip. Otherwise append.
            let _ = append_child(&ancestor, last_node.clone());
        }

        // 4.10: Create a new element with the same tag/attrs as the
        //       formatting element.
        let formatting_element = match &parser.active_formatting_elements[formatting_afe_index] {
            ActiveFormattingEntry::Element(el) => el.clone(),
            _ => unreachable!("formatting element entry is always Element here"),
        };
        let new_formatting = clone_element(parser, &formatting_element);

        // 4.11: Move all children of furthest block to new_formatting.
        let furthest_block = parser.open_elements[furthest_block_index].clone();
        let children: Vec<Rc<RefCell<Node>>> = furthest_block.borrow().children.clone();
        for child in &children {
            let _ = muskitty_dom::remove_child(&furthest_block, child);
            let _ = append_child(&new_formatting, child.clone());
        }

        // 4.12: Append new_formatting to furthest block.
        let _ = append_child(&furthest_block, new_formatting.clone());

        // 4.13: Remove the formatting element from the AFE list, and insert
        //       new_formatting at the bookmark position.
        parser
            .active_formatting_elements
            .remove(formatting_afe_index);
        // Adjust bookmark if it was after the removed entry.
        if bookmark > formatting_afe_index {
            bookmark -= 1;
        }
        // Insert at bookmark (clamped to list length).
        let insert_at = bookmark.min(parser.active_formatting_elements.len());
        parser.active_formatting_elements.insert(
            insert_at,
            ActiveFormattingEntry::Element(new_formatting.clone()),
        );

        // 4.14: Remove the formatting element from the stack, and insert
        //       new_formatting immediately after furthest block.
        let fmt_stack_idx = stack_index_of(parser, &formatting_element);
        if let Some(i) = fmt_stack_idx {
            parser.open_elements.remove(i);
        }
        let fb_idx = stack_index_of(parser, &furthest_block);
        if let Some(i) = fb_idx {
            parser.open_elements.insert(i + 1, new_formatting);
        }
        // Continue the outer loop.
    }
}

/// Helper: decrement `fmt_stack_index` by one (used when a node below the
/// formatting element is removed from the stack, shifting indices down).
fn fmt_stack_index_will_decrement(idx: &mut usize) {
    *idx = idx.saturating_sub(1);
}

/// The "any other end tag" algorithm (§13.2.6.4.7), used as a fallback by
/// the adoption agency algorithm when there is no matching formatting
/// element in the active formatting list.
fn run_any_other_end_tag(parser: &mut HtmlTreeConstructor, name: &str) {
    // Walk the stack from top to bottom. If a matching element is found,
    // generate implied end tags except `name`, then pop until it is popped.
    // If a special element (that is not the target) is encountered first,
    // parse error, return (ignore the end tag).
    for (i, node) in parser.open_elements.iter().enumerate().rev() {
        let node_name = node
            .borrow()
            .kind
            .as_element()
            .map(|e| e.local_name.clone());
        if node_name.as_deref() == Some(name) {
            generate_implied_end_tags(parser, Some(name));
            // Pop until the node at index i is popped.
            while parser.open_elements.len() > i {
                parser.open_elements.pop();
            }
            return;
        }
        if let Some(n) = node_name.as_deref() {
            if SPECIAL_ELEMENTS.contains(&n) {
                parser
                    .errors
                    .push(ParseError::UnexpectedEndTag(name.to_string()));
                return;
            }
        }
    }
}
