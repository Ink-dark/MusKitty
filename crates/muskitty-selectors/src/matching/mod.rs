//! Selectors Level 4 §18 matching engine.
//!
//! Matches parsed selectors ([`crate::types::SelectorList`] /
//! [`crate::types::ComplexSelector`]) against an element tree via the
//! [`Element`] trait. The engine walks complex selectors right-to-left
//! per §18 L4902-4919.
//!
//! # Architecture
//!
//! - [`Element`] trait — abstracts the 5 aspects of an element (§3
//!   L865-874: type / namespace / id / classes / attributes) plus
//!   tree traversal required by §13 child-indexed pseudo-classes and
//!   §15 combinators.
//! - [`simple_matcher`] — type / universal / class / id / attribute.
//! - [`pseudo_matcher`] — tree-structural + An+B + logical combinations.
//! - [`dom_impl`] — reference impl of `Element` for
//!   `Rc<RefCell<muskitty_dom::Node>>` (dev-only; not compiled into
//!   the released library).

pub mod dom_impl;
pub mod pseudo_matcher;
pub mod simple_matcher;

use crate::types::{ComplexSelector, SelectorList};

/// §3 L858-873 + §18 L4879-4900: read-only view of an element in a
/// tree.
///
/// Implementors provide the 5 aspects of an element (type / namespace
/// / id / classes / attributes) plus the tree-traversal operations
/// required by §13 child-indexed pseudo-classes (parent / sibling
/// iteration) and §15 combinators (parent for Child / ancestor for
/// Descendant / siblings for NextSibling / SubsequentSibling).
///
/// `Self: Clone` is required so that trait methods can return owned
/// copies of the element handle (e.g. `parent_element()` returns
/// `Option<Self>`). For `Rc<RefCell<Node>>` this is a cheap `Rc`
/// clone.
///
/// Methods return owned `String` / `Vec<String>` (not `&str`) because
/// underlying element data is often behind a `RefCell` whose borrow
/// guard cannot escape the function returning the reference.
pub trait Element: Clone {
    /// §3 L870: element type (tag name). Lowercase for HTML.
    fn local_name(&self) -> String;

    /// §3 L871: namespace URI (`None` for no namespace).
    fn namespace_uri(&self) -> Option<String>;

    /// §3 L872: ID attribute value (`None` if absent).
    fn id(&self) -> Option<String>;

    /// §3 L873: classes (space-separated list, may be empty).
    fn classes(&self) -> Vec<String>;

    /// §3 L874: attribute lookup by name. HTML namespace: ASCII
    /// case-insensitive name comparison.
    fn get_attribute(&self, name: &str) -> Option<String>;

    /// Parent element (`None` for root / detached).
    fn parent_element(&self) -> Option<Self>;

    /// Previous sibling element (`None` if first child).
    fn previous_sibling_element(&self) -> Option<Self>;

    /// Next sibling element (`None` if last child).
    fn next_sibling_element(&self) -> Option<Self>;

    /// Iterate child elements (excluding text / comment nodes).
    fn child_elements(&self) -> Vec<Self>;

    /// §13.3 L3820: whether this is the document root (no parent
    /// element). Default impl checks `parent_element().is_none()`.
    fn is_root(&self) -> bool {
        self.parent_element().is_none()
    }

    /// §13.3 L3837-3845: whether the element has no children except
    /// optionally whitespace-only text nodes. Comments and PIs do
    /// not affect emptiness.
    fn is_empty(&self) -> bool;

    /// §13.3 L3982: 1-based index among inclusive siblings (all
    /// element siblings, including self).
    fn index_among_siblings(&self) -> usize;

    /// Total count of inclusive siblings (all element siblings,
    /// including self).
    fn count_among_siblings(&self) -> usize;

    /// §13.3: 1-based index among siblings of the same type (same
    /// `local_name`, case-insensitive).
    fn index_among_type(&self) -> usize;

    /// Total count of siblings of the same type (including self).
    fn count_among_type(&self) -> usize;
}

/// §18 L4878-4919: Match a selector list against an element.
///
/// Returns `true` if any complex selector in `list` matches `element`
/// (right-to-left walk per §18 L4902-4919).
pub fn matches<E: Element>(list: &SelectorList, element: &E) -> bool {
    list.0.iter().any(|cs| matches_complex(cs, element))
}

/// `pub(crate)` wrapper around [`matches`] for use by sibling modules
/// (e.g. `pseudo_matcher` resolving `:nth-child(An+B of S)` filters).
/// Kept separate from [`matches`] so the public API surface stays
/// flat.
pub(crate) fn matches_complex_list<E: Element>(list: &SelectorList, element: &E) -> bool {
    list.0.iter().any(|cs| matches_complex(cs, element))
}

/// §18 L4955-5026: Match a selector list against a tree, returning
/// the first matching element in tree order. Returns `None` if no
/// descendant of `root` matches.
pub fn query_selector<E: Element>(root: &E, list: &SelectorList) -> Option<E> {
    query_selector_all(root, list).into_iter().next()
}

/// §18 L4955-5026: Match a selector list against a tree, returning
/// all matching elements in tree order (depth-first, pre-order).
pub fn query_selector_all<E: Element>(root: &E, list: &SelectorList) -> Vec<E> {
    let mut out = Vec::new();
    walk_tree(root, &mut |el: &E| {
        if matches(list, el) {
            out.push(el.clone());
        }
    });
    out
}

/// Depth-first pre-order walk of `root`'s subtree (including root
/// itself).
fn walk_tree<E: Element, F: FnMut(&E)>(root: &E, f: &mut F) {
    f(root);
    for child in root.child_elements() {
        walk_tree(&child, f);
    }
}

/// §18 L4902-4919: Match a complex selector against an element,
/// processing compound selectors right-to-left.
///
/// `units[0]` is the subject (rightmost compound in source order).
/// The combinator on `units[idx]` links it to `units[idx+1]` (the
/// next leftward compound): e.g. for `a > b`, `units = [{b, Child},
/// {a, None}]` and `b`'s parent must match `a`. We match the subject
/// first, then walk leftward checking combinators against related
/// elements.
fn matches_complex<E: Element>(cs: &ComplexSelector, element: &E) -> bool {
    if cs.units.is_empty() {
        return false;
    }
    // §18 L4908: the rightmost compound (units[0], the subject)
    // must match `element`.
    let subject = &cs.units[0];
    if !simple_matcher::matches_compound(&subject.compound, element) {
        return false;
    }
    // §18 L4911-4912: if there is only one compound, success.
    if cs.units.len() == 1 {
        return true;
    }
    // §18 L4914-4919: otherwise, walk leftward using the subject's
    // combinator to find candidates for units[1..].
    let combinator = match subject.combinator {
        Some(c) => c,
        None => return true, // shouldn't happen for len > 1
    };
    walk_leftward(&cs.units[1..], element, combinator)
}

/// Walk leftward from `element` by `combinator` to find candidates
/// for `remaining[0]`. `combinator` was carried by the previous
/// (rightward) unit, so it describes how `remaining[0]` is related
/// to `element` (e.g. for `Child`, `remaining[0]` is the parent of
/// `element`).
fn walk_leftward<E: Element>(
    remaining: &[crate::types::ComplexSelectorUnit],
    element: &E,
    combinator: crate::types::Combinator,
) -> bool {
    let next_unit = &remaining[0];
    match combinator {
        crate::types::Combinator::Descendant => {
            // §15 L4369: any ancestor of `element`.
            let mut ancestor = element.parent_element();
            while let Some(parent) = ancestor {
                if simple_matcher::matches_compound(&next_unit.compound, &parent)
                    && continues_leftward(remaining, &parent)
                {
                    return true;
                }
                ancestor = parent.parent_element();
            }
            false
        }
        crate::types::Combinator::Child => {
            // §15 L4376: direct parent only.
            if let Some(parent) = element.parent_element() {
                if simple_matcher::matches_compound(&next_unit.compound, &parent)
                    && continues_leftward(remaining, &parent)
                {
                    return true;
                }
            }
            false
        }
        crate::types::Combinator::NextSibling => {
            // §15 L4383: direct previous sibling only.
            if let Some(prev) = element.previous_sibling_element() {
                if simple_matcher::matches_compound(&next_unit.compound, &prev)
                    && continues_leftward(remaining, &prev)
                {
                    return true;
                }
            }
            false
        }
        crate::types::Combinator::SubsequentSibling => {
            // §15 L4390: any previous sibling.
            let mut prev = element.previous_sibling_element();
            while let Some(sibling) = prev {
                if simple_matcher::matches_compound(&next_unit.compound, &sibling)
                    && continues_leftward(remaining, &sibling)
                {
                    return true;
                }
                prev = sibling.previous_sibling_element();
            }
            false
        }
    }
}

/// After matching `remaining[0]` against an element, continue the
/// leftward walk if there are more units. If `remaining[0]` is the
/// leftmost unit, the match is complete.
fn continues_leftward<E: Element>(
    remaining: &[crate::types::ComplexSelectorUnit],
    element: &E,
) -> bool {
    if remaining.len() == 1 {
        // remaining[0] is leftmost; we've already matched it.
        return true;
    }
    // Recurse using remaining[0].combinator to find candidates for
    // remaining[1].
    let next_combinator = match remaining[0].combinator {
        Some(c) => c,
        None => return true, // leftmost, already matched
    };
    walk_leftward(&remaining[1..], element, next_combinator)
}
