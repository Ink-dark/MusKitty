//! SP-8 §18 matching engine — end-to-end tests against muskitty-dom.
//!
//! Builds a real DOM tree with `muskitty_dom::Node` and exercises the
//! `Element` trait impl + the matching engine. These are smoke tests;
//! per-feature coverage lives in `matching_basic.rs` and
//! `matching_pseudo.rs` using the lighter `StubElement`.

use muskitty_dom::node::{Node, NodeType};
use muskitty_selectors::matching::Element;
use std::cell::RefCell;
use std::rc::Rc;

/// Newtype wrapper around `Rc<RefCell<Node>>` so the orphan rule
/// permits us to implement the foreign trait `Element` (from
/// muskitty-selectors) on a type local to this test crate. The
/// `Element` trait methods on this wrapper simply forward to the
/// underlying DOM node.
#[derive(Clone, Debug)]
struct DomElement(Rc<RefCell<Node>>);

impl DomElement {
    fn new(node: Rc<RefCell<Node>>) -> Self {
        DomElement(node)
    }

    fn borrow_node(&self) -> std::cell::Ref<'_, Node> {
        self.0.borrow()
    }
}

/// Walk previous_element chain (skipping text / comment siblings).
fn previous_sibling_element(node: &Rc<RefCell<Node>>) -> Option<Rc<RefCell<Node>>> {
    let mut cur = node.borrow().previous_sibling();
    while let Some(sibling) = cur {
        if sibling.borrow().node_type == NodeType::Element {
            return Some(sibling);
        }
        cur = sibling.borrow().previous_sibling();
    }
    None
}

/// Walk next_element chain.
fn next_sibling_element(node: &Rc<RefCell<Node>>) -> Option<Rc<RefCell<Node>>> {
    let mut cur = node.borrow().next_sibling();
    while let Some(sibling) = cur {
        if sibling.borrow().node_type == NodeType::Element {
            return Some(sibling);
        }
        cur = sibling.borrow().next_sibling();
    }
    None
}

impl Element for DomElement {
    fn local_name(&self) -> String {
        self.borrow_node()
            .kind
            .as_element()
            .map(|e| e.local_name.clone())
            .unwrap_or_default()
    }

    fn namespace_uri(&self) -> Option<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.namespace_uri.clone())
    }

    fn id(&self) -> Option<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute("id").map(String::from))
    }

    fn classes(&self) -> Vec<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute("class").map(String::from))
            .map(|class_attr| {
                class_attr
                    .split_ascii_whitespace()
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_attribute(&self, name: &str) -> Option<String> {
        self.borrow_node()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute(name).map(String::from))
    }

    fn parent_element(&self) -> Option<Self> {
        self.borrow_node().parent_element().map(DomElement::new)
    }

    fn previous_sibling_element(&self) -> Option<Self> {
        previous_sibling_element(&self.0).map(DomElement::new)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        next_sibling_element(&self.0).map(DomElement::new)
    }

    fn child_elements(&self) -> Vec<Self> {
        self.borrow_node()
            .children
            .iter()
            .filter(|c| c.borrow().node_type == NodeType::Element)
            .cloned()
            .map(DomElement::new)
            .collect()
    }

    fn is_empty(&self) -> bool {
        // §13.3 L3837-3845: empty = no element children AND no text
        // children with non-zero length (whitespace-only text counts
        // as empty per L3858-3866).
        for child in &self.borrow_node().children {
            let child_borrow = child.borrow();
            match child_borrow.node_type {
                NodeType::Element => return false,
                NodeType::Text => {
                    if let Some(t) = child_borrow.kind.as_text() {
                        if !t.data.chars().all(|c| c.is_ascii_whitespace()) {
                            return false;
                        }
                    }
                }
                _ => {} // comments / PIs ignored
            }
        }
        true
    }

    fn index_among_siblings(&self) -> usize {
        // §13.3 L3982: 1-indexed. Walk previous_sibling_element chain.
        let mut idx = 1;
        let mut cur = self.previous_sibling_element();
        while let Some(prev) = cur {
            idx += 1;
            cur = prev.previous_sibling_element();
        }
        idx
    }

    fn count_among_siblings(&self) -> usize {
        self.index_among_siblings() + self.next_sibling_element_iter().count()
    }

    fn index_among_type(&self) -> usize {
        let my_name = self.local_name();
        let mut idx = 1;
        let mut cur = self.previous_sibling_element();
        while let Some(prev) = cur {
            if prev.local_name().eq_ignore_ascii_case(&my_name) {
                idx += 1;
            }
            cur = prev.previous_sibling_element();
        }
        idx
    }

    fn count_among_type(&self) -> usize {
        let my_name = self.local_name();
        let mut count = 1;
        let mut cur = self.previous_sibling_element();
        while let Some(prev) = cur {
            if prev.local_name().eq_ignore_ascii_case(&my_name) {
                count += 1;
            }
            cur = prev.previous_sibling_element();
        }
        let mut cur = self.next_sibling_element();
        while let Some(next) = cur {
            if next.local_name().eq_ignore_ascii_case(&my_name) {
                count += 1;
            }
            cur = next.next_sibling_element();
        }
        count
    }
}

/// Private extension so `count_among_siblings` can walk forward
/// without duplicating the next-sibling loop.
trait NextSiblingIter {
    fn next_sibling_element_iter(&self) -> std::vec::IntoIter<DomElement>;
}

impl NextSiblingIter for DomElement {
    fn next_sibling_element_iter(&self) -> std::vec::IntoIter<DomElement> {
        let mut out = Vec::new();
        let mut cur = self.next_sibling_element();
        while let Some(next) = cur {
            out.push(next.clone());
            cur = next_sibling_element(&next.0).map(DomElement::new);
        }
        out.into_iter()
    }
}

/// Build a 3-deep tree: <root><child><grandchild/></child></root>.
fn build_tree() -> DomElement {
    let doc = Node::new_document();
    let root = Node::new_element_html("root", vec![], &doc);
    let child = Node::new_element_html("child", vec![], &doc);
    let grandchild = Node::new_element_html("grandchild", vec![], &doc);
    muskitty_dom::tree::append_child(&child, grandchild).expect("append");
    muskitty_dom::tree::append_child(&root, child).expect("append");
    DomElement::new(root)
}

#[test]
fn dom_element_local_name() {
    let root = build_tree();
    assert_eq!(root.local_name(), "root");
}

#[test]
fn dom_element_parent_chain() {
    let root = build_tree();
    let child = root.child_elements().into_iter().next().expect("has child");
    let parent = child.parent_element().expect("has parent");
    assert_eq!(parent.local_name(), "root");
}

#[test]
fn dom_element_is_root() {
    let root = build_tree();
    assert!(root.is_root());
    let child = root.child_elements().into_iter().next().expect("has child");
    assert!(!child.is_root());
}
