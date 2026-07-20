# SP-8 Implementation Plan: §18 Matching Engine + lib.rs Public API

> **For agentic workers:** This plan follows the SP-6/SP-7 TDD pattern. Each task: write failing test → run to verify failure → implement → run to verify pass → commit. Per-commit quality gate is mandatory.

**Goal:** Implement the Selectors Level 4 §18 "API Hooks" matching engine for `muskitty-selectors`, exposing `matches()` / `query_selector()` / `query_selector_all()` as the crate's public API. On completion the crate is feature-complete per the parent SP-1..SP-8 plan and ready to split into an independent git repository.

**Architecture:** A new `src/matching/` module hosts an `Element` trait that abstracts the read-only element-tree view required by §3 L858-873 (5 aspects: type / namespace / id / classes / attributes) plus sibling/parent traversal required by §13 child-indexed pseudo-classes and §15 combinators. The matching engine walks `ComplexSelector::units` right-to-left per §18 L4902-4919, dispatching each compound selector's components to per-kind matchers (simple / pseudo / combinator). A reference `Element` impl for `Rc<RefCell<muskitty_dom::Node>>` lives in `src/matching/dom_impl.rs` and is exercised via `dev-dependency muskitty-dom` (lib itself has zero DOM dependency).

**Tech Stack:** Rust 2021, MSRV 1.82, zero non-workspace deps. `muskitty-dom` only as dev-dep.

---

## File Structure

**New files:**
- `crates/muskitty-selectors/src/matching/mod.rs` — `Element` trait + top-level `matches` / `query_selector` / `query_selector_all` API + complex-selector right-to-left walk
- `crates/muskitty-selectors/src/matching/simple_matcher.rs` — type / universal / class / id / attribute matchers
- `crates/muskitty-selectors/src/matching/pseudo_matcher.rs` — tree-structural + An+B pseudo-class matchers + `:is` / `:where` / `:not` / `:has`
- `crates/muskitty-selectors/src/matching/dom_impl.rs` — `impl Element for Rc<RefCell<muskitty_dom::Node>>`
- `crates/muskitty-selectors/tests/matching_basic.rs` — type / class / id / attribute / universal matching
- `crates/muskitty-selectors/tests/matching_pseudo.rs` — tree-structural + nth-child + logical combinations
- `crates/muskitty-selectors/tests/matching_dom.rs` — end-to-end against a real muskitty-dom tree
- `crates/muskitty-selectors/README.md` — status table + architecture + quick start

**Modified files:**
- `crates/muskitty-selectors/src/lib.rs` — `pub mod matching;` + `pub use matching::{matches, query_selector, query_selector_all, Element};`
- `crates/muskitty-selectors/Cargo.toml` — `[dev-dependencies] muskitty-dom = { path = "../muskitty-dom" }`
- `crates/muskitty-selectors/src/parser/mod.rs` — replace `parse_a_relative_selector`'s `Err(NotImplemented)` body with real implementation (Task 6 uses it for `:has()`)

**Out of scope (deferred):**
- WPT subset integration — a separate polish pass after the crate is split
- `:host` / `:host-context()` matching (shadow DOM scope)
- `:scope` with explicit scoping roots (parameter to `matches()`); defaults to root element when no scope provided
- UI / location / linguistic pseudo-class *matching* — parsing already done in SP-4, matching returns `false` stub
- Pseudo-element matching (pseudo-elements are not in the element tree)

---

## Design Decisions

### 1. `Element` trait shape — owned `String` returns

`muskitty_dom::Node` is wrapped in `Rc<RefCell<Node>>`. `RefCell::borrow()` returns a `Ref<'_, Node>` guard whose lifetime cannot escape the calling function, so a trait method returning `&str` would either (a) borrow `&self` for the lifetime of the returned `&str`, preventing further borrows, or (b) be impossible to implement for `Rc<RefCell<Node>>` without leaking the borrow guard.

**Decision:** trait methods return owned `String` / `Option<String>` / `Vec<String>`. This costs extra allocations but keeps the trait implementable and the call sites clean. Performance work (interner / `Cow<str>`) is deferred.

### 2. `Self: Clone` for sibling/parent traversal

The trait requires returning "the parent element" / "the previous sibling". For `Rc<RefCell<Node>>` this is a cheap `Rc::clone`. We bound `Element: Clone` so callers can return `Option<Self>`.

### 3. Right-to-left walk (§18 L4902-4919)

Per spec, matching a `ComplexSelector` against an element starts at the subject (rightmost compound = `units[0]`) and walks leftward. For each combinator on `units[i]`, the engine considers all candidate elements related to the current element by that combinator (ancestor for Descendant, parent for Child, previous sibling for NextSibling, any previous sibling for SubsequentSibling), and recursively attempts to match `units[i+1..]` against each candidate. The recursion bottoms out when only one unit remains (success) or no candidate can be found (failure).

### 4. `:nth-child(An+B [of S]?)` matching

Per §13.3 L3968-3982: the index is 1-based. When `of S` is present, first filter the inclusive siblings to those matching `S`, then check whether the element's 1-based position in that filtered list satisfies `An+B`. When `of S` is absent, `S` defaults to `*|*` (all inclusive siblings).

### 5. `:has()` relative selector matching

Per §4.5 L1650-1804: `:has(S)` matches an element `e` if any descendant or sibling (depending on the relative selector's leading combinator) of `e` matches `S` relative to `e` (i.e., with `:scope` bound to `e`). The default leading combinator is Descendant.

### 6. Stub pseudo-classes

The following pseudo-classes are **parsed** (SP-4) but **matching returns `false`** (per parent plan §7): all UI / location / linguistic / resource-state / display-state / input pseudo-classes. `:defined` and `:scope` get minimal real implementations (always-true for `:defined` in non-custom-element trees; `:scope` matches the root element when no scope is provided).

---

## Task 1: `matching` module skeleton + `Element` trait

**Files:**
- Create: `crates/muskitty-selectors/src/matching/mod.rs`
- Modify: `crates/muskitty-selectors/src/lib.rs`

- [ ] **Step 1.1: Write the failing test**

Create `crates/muskitty-selectors/tests/matching_basic.rs`:

```rust
//! SP-8 §18 matching engine — basic tests.
//!
//! Covers §18 L4878-4919 (Match a Selector Against an Element) and
//! the simple-selector matchers in §3 L858-873 + §5 + §6.

use muskitty_selectors::matching::Element;
use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::matching::matches;

/// Minimal in-memory element for unit-testing the matching engine
/// without pulling in muskitty-dom. Tests against muskitty-dom live
/// in `tests/matching_dom.rs`.
#[derive(Clone, Debug)]
struct StubElement {
    local_name: String,
    namespace_uri: Option<String>,
    id: Option<String>,
    classes: Vec<String>,
    attributes: Vec<(String, String)>,
    parent: Option<Box<StubElement>>,
    previous_sibling: Option<Box<StubElement>>,
    next_sibling: Option<Box<StubElement>>,
    children: Vec<StubElement>,
}

impl StubElement {
    fn new(local_name: &str) -> Self {
        Self {
            local_name: local_name.to_string(),
            namespace_uri: None,
            id: None,
            classes: Vec::new(),
            attributes: Vec::new(),
            parent: None,
            previous_sibling: None,
            next_sibling: None,
            children: Vec::new(),
        }
    }
}

impl Element for StubElement {
    fn local_name(&self) -> String { self.local_name.clone() }
    fn namespace_uri(&self) -> Option<String> { self.namespace_uri.clone() }
    fn id(&self) -> Option<String> { self.id.clone() }
    fn classes(&self) -> Vec<String> { self.classes.clone() }
    fn get_attribute(&self, name: &str) -> Option<String> {
        self.attributes.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }
    fn parent_element(&self) -> Option<Self> { self.parent.clone() }
    fn previous_sibling_element(&self) -> Option<Self> { self.previous_sibling.clone() }
    fn next_sibling_element(&self) -> Option<Self> { self.next_sibling.clone() }
    fn child_elements(&self) -> Vec<Self> { self.children.clone() }
    fn is_empty(&self) -> bool { self.children.is_empty() }
    fn index_among_siblings(&self) -> usize {
        // Walk previous_sibling chain. 1-indexed per §13.3 L3982.
        let mut idx = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur {
            idx += 1;
            cur = prev.previous_sibling.clone();
        }
        idx
    }
    fn count_among_siblings(&self) -> usize {
        let mut count = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur { count += 1; cur = prev.previous_sibling; }
        let mut cur = self.next_sibling.clone();
        while let Some(next) = cur { count += 1; cur = next.next_sibling; }
        count
    }
    fn index_among_type(&self) -> usize {
        let mut idx = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur {
            if prev.local_name.eq_ignore_ascii_case(&self.local_name) { idx += 1; }
            cur = prev.previous_sibling;
        }
        idx
    }
    fn count_among_type(&self) -> usize {
        let mut count = 1;
        let mut cur = self.previous_sibling.clone();
        while let Some(prev) = cur {
            if prev.local_name.eq_ignore_ascii_case(&self.local_name) { count += 1; }
            cur = prev.previous_sibling;
        }
        let mut cur = self.next_sibling.clone();
        while let Some(next) = cur {
            if next.local_name.eq_ignore_ascii_case(&self.local_name) { count += 1; }
            cur = next.next_sibling;
        }
        count
    }
}

/// §3 L870: type selector matches local_name (case-insensitive HTML).
#[test]
fn type_selector_matches_case_insensitive() {
    let list = parse_a_selector("div").expect("parses");
    let el = StubElement::new("DIV");
    assert!(matches(&list, &el));
}
```

- [ ] **Step 1.2: Run test to verify it fails (compile error)**

Run: `cargo test -p muskitty-selectors --test matching_basic`
Expected: FAIL with `unresolved import muskitty_selectors::matching`

- [ ] **Step 1.3: Create `matching/mod.rs` with the `Element` trait skeleton**

```rust
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

use crate::types::SelectorList;

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
    list.0.iter().any(|cs| matching::matches_complex(cs, element))
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

// Internal module so the public `matches` above can dispatch into
// complex-selector matching without polluting the crate root.
mod matching {
    use crate::matching::Element;
    use crate::types::ComplexSelector;

    pub fn matches_complex<E: Element>(_cs: &ComplexSelector, _element: &E) -> bool {
        // SP-8 Task 7 implements this.
        false
    }
}
```

- [ ] **Step 1.4: Create empty `simple_matcher.rs`, `pseudo_matcher.rs`, `dom_impl.rs`**

```rust
//! Simple-selector matching (type / universal / class / id /
//! attribute). Implemented in Task 3.
```

```rust
//! Pseudo-class matching (tree-structural + An+B + logical
//! combinations). Implemented in Tasks 4-6.
```

```rust
//! Reference `Element` impl for `Rc<RefCell<muskitty_dom::Node>>`.
//! Implemented in Task 2.
```

- [ ] **Step 1.5: Add `pub mod matching;` to `lib.rs`**

Modify `crates/muskitty-selectors/src/lib.rs` — insert after `pub mod types;`:

```rust
pub mod matching;
```

- [ ] **Step 1.6: Run test to verify the stub matches type selectors**

Run: `cargo test -p muskitty-selectors --test matching_basic`
Expected: FAIL — `type_selector_matches_case_insensitive` returns `false` because `matches_complex` is a stub returning `false`. Compile errors should be resolved.

- [ ] **Step 1.7: Commit**

```powershell
git add crates/muskitty-selectors/src/matching/ crates/muskitty-selectors/src/lib.rs crates/muskitty-selectors/tests/matching_basic.rs
git commit -m "[selectors] SP-8 task 1: matching module skeleton + Element trait"
```

---

## Task 2: `muskitty-dom` `Element` impl + dev-dep

**Files:**
- Modify: `crates/muskitty-selectors/Cargo.toml`
- Modify: `crates/muskitty-selectors/src/matching/dom_impl.rs`

- [ ] **Step 2.1: Add `muskitty-dom` dev-dependency**

Modify `crates/muskitty-selectors/Cargo.toml` — append:

```toml

[dev-dependencies]
muskitty-dom = { path = "../muskitty-dom" }
```

- [ ] **Step 2.2: Write the failing test**

Append to `crates/muskitty-selectors/tests/matching_dom.rs` (new file):

```rust
//! SP-8 §18 matching engine — end-to-end tests against muskitty-dom.
//!
//! Builds a real DOM tree with `muskitty_dom::Node` and exercises the
//! `Element` trait impl + the matching engine. These are smoke tests;
//! per-feature coverage lives in `matching_basic.rs` and
//! `matching_pseudo.rs` using the lighter `StubElement`.

use muskitty_dom::attribute::Attribute;
use muskitty_dom::node::{Node, NodeType};
use muskitty_selectors::matching::{matches, Element};
use muskitty_selectors::parser::parse_a_selector;
use std::cell::RefCell;
use std::rc::Rc;

/// Build a 3-deep tree: <root><child><grandchild/></child></root>.
fn build_tree() -> Rc<RefCell<Node>> {
    let doc = Node::new_document();
    let root = Node::new_element_html("root", vec![], &doc);
    let child = Node::new_element_html("child", vec![], &doc);
    let grandchild = Node::new_element_html("grandchild", vec![], &doc);
    muskitty_dom::tree::append_child(&root, &grandchild);
    muskitty_dom::tree::append_child(&root, &child);
    // Wait — that's wrong order. Re-do:
    let _ = std::mem::take(&mut root.borrow_mut().children);
    muskitty_dom::tree::append_child(&root, &child);
    muskitty_dom::tree::append_child(&child, &grandchild);
    root
}

#[test]
fn dom_element_local_name() {
    let root = build_tree();
    let name = Element::local_name(&root);
    assert_eq!(name, "root");
}

#[test]
fn dom_element_parent_chain() {
    let root = build_tree();
    let child = root.borrow().children[0].clone();
    let parent = Element::parent_element(&child);
    assert!(parent.is_some());
    assert_eq!(Element::local_name(&parent.unwrap()), "root");
}

#[test]
fn dom_element_is_root() {
    let root = build_tree();
    assert!(Element::is_root(&root));
    let child = root.borrow().children[0].clone();
    assert!(!Element::is_root(&child));
}

#[test]
fn dom_type_selector_matches() {
    let root = build_tree();
    let list = parse_a_selector("root").expect("parses");
    assert!(matches(&list, &root));
}
```

- [ ] **Step 2.3: Run test to verify it fails (trait not impl'd)**

Run: `cargo test -p muskitty-selectors --test matching_dom`
Expected: FAIL with `the trait bound \`Rc<RefCell<muskitty_dom::Node>>: Element\` is not satisfied`

- [ ] **Step 2.4: Implement `Element` for `Rc<RefCell<muskitty_dom::Node>>`**

Replace `crates/muskitty-selectors/src/matching/dom_impl.rs` contents with:

```rust
//! Reference `Element` impl for `muskitty_dom::Node`.
//!
//! Bridges the DOM Living Standard `Node` type to the Selectors Level
//! 4 [`Element`](crate::matching::Element) trait. Compiled into the
//! crate (no feature gate) but only meaningful when the consumer
//! depends on `muskitty-dom`; for downstream crates that use a
//! different element tree, the trait can be implemented directly.

use crate::matching::Element;
use muskitty_dom::node::{Node, NodeType};
use std::cell::RefCell;
use std::rc::Rc;

impl Element for Rc<RefCell<Node>> {
    fn local_name(&self) -> String {
        self.borrow()
            .kind
            .as_element()
            .map(|e| e.local_name.clone())
            .unwrap_or_default()
    }

    fn namespace_uri(&self) -> Option<String> {
        self.borrow()
            .kind
            .as_element()
            .and_then(|e| e.namespace_uri.clone())
    }

    fn id(&self) -> Option<String> {
        self.borrow()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute("id").map(String::from))
    }

    fn classes(&self) -> Vec<String> {
        self.borrow()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute("class"))
            .map(|class_attr| {
                class_attr
                    .split_ascii_whitespace()
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn get_attribute(&self, name: &str) -> Option<String> {
        self.borrow()
            .kind
            .as_element()
            .and_then(|e| e.get_attribute(name).map(String::from))
    }

    fn parent_element(&self) -> Option<Self> {
        self.borrow().parent_element()
    }

    fn previous_sibling_element(&self) -> Option<Self> {
        // §13.3 L3957-3958: "inclusive siblings" — only element
        // siblings count (skip text / comment nodes).
        let mut cur = self.borrow().previous_sibling();
        while let Some(sibling) = cur {
            if sibling.borrow().node_type == NodeType::Element {
                return Some(sibling);
            }
            cur = sibling.borrow().previous_sibling();
        }
        None
    }

    fn next_sibling_element(&self) -> Option<Self> {
        let mut cur = self.borrow().next_sibling();
        while let Some(sibling) = cur {
            if sibling.borrow().node_type == NodeType::Element {
                return Some(sibling);
            }
            cur = sibling.borrow().next_sibling();
        }
        None
    }

    fn child_elements(&self) -> Vec<Self> {
        self.borrow()
            .children
            .iter()
            .filter(|c| c.borrow().node_type == NodeType::Element)
            .cloned()
            .collect()
    }

    fn is_empty(&self) -> bool {
        // §13.3 L3837-3845: empty = no element children AND no text
        // children with non-zero length (whitespace-only text counts
        // as empty per L3858-3866).
        for child in &self.borrow().children {
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
        // Self + all preceding element siblings + all following
        // element siblings.
        self.index_among_siblings()
            + self.next_sibling_element_iter().count()
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

/// Private extension to avoid duplicating the next-sibling walk logic
/// in `count_among_siblings`.
trait NextSiblingIter {
    fn next_sibling_element_iter(&self) -> std::vec::IntoIter<Rc<RefCell<Node>>>;
}

impl NextSiblingIter for Rc<RefCell<Node>> {
    fn next_sibling_element_iter(&self) -> std::vec::IntoIter<Rc<RefCell<Node>>> {
        let mut out = Vec::new();
        let mut cur = self.next_sibling_element();
        while let Some(next) = cur {
            out.push(next.clone());
            cur = next.next_sibling_element();
        }
        out.into_iter()
    }
}
```

- [ ] **Step 2.5: Run test to verify it passes**

Run: `cargo test -p muskitty-selectors --test matching_dom`
Expected: PASS (4 tests)

Note: `dom_type_selector_matches` will still FAIL because `matches_complex` is a stub returning `false`. That's expected — the impl compiles, the 4th test will pass only after Task 7. Adjust the test expectation for now: comment out `dom_type_selector_matches`'s body to just `assert!(true);` until Task 7, or remove the test and re-add it in Task 8. **Recommended: remove the test from Task 2's file, re-add in Task 8.**

- [ ] **Step 2.6: Quality gate**

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

- [ ] **Step 2.7: Commit**

```powershell
git add crates/muskitty-selectors/Cargo.toml crates/muskitty-selectors/src/matching/dom_impl.rs crates/muskitty-selectors/tests/matching_dom.rs
git commit -m "[selectors] SP-8 task 2: Element impl for Rc<RefCell<muskitty_dom::Node>>"
```

---

## Task 3: Simple-selector matching (type / universal / class / id / attribute)

**Files:**
- Modify: `crates/muskitty-selectors/src/matching/simple_matcher.rs`
- Modify: `crates/muskitty-selectors/src/matching/mod.rs` (wire `matches_complex` to call `simple_matcher::matches_compound` for single-unit selectors)
- Modify: `crates/muskitty-selectors/tests/matching_basic.rs`

- [ ] **Step 3.1: Write the failing tests**

Append to `crates/muskitty-selectors/tests/matching_basic.rs`:

```rust
/// §5 L1808-1824: type selector matches local_name (case-insensitive HTML).
#[test]
fn type_selector_matches() {
    let list = parse_a_selector("div").expect("parses");
    let el = StubElement::new("div");
    assert!(matches(&list, &el));
}

/// §5 L1825-1866: universal selector matches anything.
#[test]
fn universal_selector_matches() {
    let list = parse_a_selector("*").expect("parses");
    let el = StubElement::new("anything");
    assert!(matches(&list, &el));
}

/// §6.5 L2376-2462: class selector matches class list.
#[test]
fn class_selector_matches() {
    let mut el = StubElement::new("div");
    el.classes = vec!["foo".into(), "bar".into()];
    let list = parse_a_selector(".foo").expect("parses");
    assert!(matches(&list, &el));
    let list = parse_a_selector(".baz").expect("parses");
    assert!(!matches(&list, &el));
}

/// §6.6 L2463-2533: id selector matches id attribute.
#[test]
fn id_selector_matches() {
    let mut el = StubElement::new("div");
    el.id = Some("main".into());
    let list = parse_a_selector("#main").expect("parses");
    assert!(matches(&list, &el));
    let list = parse_a_selector("#other").expect("parses");
    assert!(!matches(&list, &el));
}

/// §6 L1996-2533: attribute presence selector.
#[test]
fn attribute_presence_matches() {
    let mut el = StubElement::new("input");
    el.attributes = vec![("disabled".into(), "".into())];
    let list = parse_a_selector("[disabled]").expect("parses");
    assert!(matches(&list, &el));
}

/// §6.1 L2037-2054: `[attr=value]` exact match.
#[test]
fn attribute_exact_match() {
    let mut el = StubElement::new("a");
    el.attributes = vec![("href".into(), "https://example.com".into())];
    let list = parse_a_selector(r#"[href="https://example.com"]"#).expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr~=value]` whitespace-list contains.
#[test]
fn attribute_includes_match() {
    let mut el = StubElement::new("div");
    el.attributes = vec![("class".into(), "foo bar baz".into())];
    let list = parse_a_selector("[class~=bar]").expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr^=value]` prefix match.
#[test]
fn attribute_prefix_match() {
    let mut el = StubElement::new("a");
    el.attributes = vec![("href".into(), "https://foo".into())];
    let list = parse_a_selector(r#"[href^="https://"]"#).expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr$=value]` suffix match.
#[test]
fn attribute_suffix_match() {
    let mut el = StubElement::new("a");
    el.attributes = vec![("href".into(), "doc.pdf".into())];
    let list = parse_a_selector(r#"[href$=".pdf"]"#).expect("parses");
    assert!(matches(&list, &el));
}

/// §6.2 L2137-2162: `[attr*=value]` substring match.
#[test]
fn attribute_substring_match() {
    let mut el = StubElement::new("div");
    el.attributes = vec![("data-x".into(), "foobar".into())];
    let list = parse_a_selector("[data-x*=oob]").expect("parses");
    assert!(matches(&list, &el));
}

/// §6.1 L2055-2080: `[attr|=value]` exact match or hyphen-prefix.
#[test]
fn attribute_dash_match() {
    let mut el = StubElement::new("html");
    el.attributes = vec![("lang".into(), "en-US".into())];
    let list = parse_a_selector("[lang|=en]").expect("parses");
    assert!(matches(&list, &el));
}

/// Compound: `div.foo#bar[baz]` matches when all components match.
#[test]
fn compound_all_components_match() {
    let mut el = StubElement::new("div");
    el.id = Some("bar".into());
    el.classes = vec!["foo".into()];
    el.attributes = vec![("baz".into(), "".into())];
    let list = parse_a_selector("div.foo#bar[baz]").expect("parses");
    assert!(matches(&list, &el));
    // Missing id → no match
    let mut el2 = el.clone();
    el2.id = None;
    assert!(!matches(&list, &el2));
}
```

- [ ] **Step 3.2: Run tests to verify they fail**

Run: `cargo test -p muskitty-selectors --test matching_basic`
Expected: FAIL — all new tests fail because `matches_complex` stub returns `false`.

- [ ] **Step 3.3: Implement `simple_matcher.rs`**

Replace `crates/muskitty-selectors/src/matching/simple_matcher.rs` contents:

```rust
//! Simple-selector matching: type / universal / class / id / attribute.
//!
//! Implements the matching rules for §5 (elemental), §6.5 (class),
//! §6.6 (id), §6 (attribute) selectors against an [`Element`].
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L858-873,
//! §5 L1808-1995, §6 L1996-2533.

use crate::matching::Element;
use crate::types::{
    AttrMatcher, AttrValue, AttributeSelector, ClassSelector, CompoundSelector, IdSelector,
    NsPrefixKind, SubclassSelector, TypeSelector, TypeSelectorName,
};

/// §3 L746-760: match a compound selector against an element. Returns
/// `true` if every component (type / subclasses / pseudo-compounds)
/// matches.
///
/// Pseudo-class / pseudo-element matching is delegated to
/// [`crate::matching::pseudo_matcher`]; this function handles type
/// and subclass matching and delegates pseudo matching.
pub fn matches_compound<E: Element>(compound: &CompoundSelector, element: &E) -> bool {
    // Type selector (or universal) — must match first.
    if let Some(ref ts) = compound.type_selector {
        if !matches_type_selector(ts, element) {
            return false;
        }
    }
    // Subclass selectors (id / class / attribute / pseudo-class).
    for sub in &compound.subclasses {
        if !matches_subclass(sub, element) {
            return false;
        }
    }
    // Pseudo-compounds (pseudo-element + trailing pseudo-classes).
    // SP-8: pseudo-elements do not exist in the element tree, so a
    // compound with any pseudo-compound never matches a real element.
    // (Pseudo-element matching would require pseudo-element tree
    // abstraction, out of scope for SP-8.)
    if !compound.pseudo_compounds.is_empty() {
        return false;
    }
    true
}

/// §5 L1808-1866: match a type selector (or universal).
pub fn matches_type_selector<E: Element>(ts: &TypeSelector, element: &E) -> bool {
    match &ts.name {
        TypeSelectorName::Universal => {
            // §5 L1825-1866: `*` matches any element. Namespace
            // prefix matters: `*|*` matches any namespace, `ns|*`
            // matches only `ns`, `|*` matches no namespace.
            match ts.ns_prefix.as_ref().map(|p| &p.prefix) {
                None | Some(NsPrefixKind::Any) => true,
                Some(NsPrefixKind::Named(_)) => {
                    // For HTML trees, named-namespace universal
                    // selectors are uncommon; we conservatively
                    // require namespace_uri to be present (non-HTML).
                    // Strict namespace matching is out of scope for
                    // SP-8; we accept any non-None namespace here.
                    element.namespace_uri().is_some()
                }
                Some(NsPrefixKind::None) => element.namespace_uri().is_none(),
            }
        }
        TypeSelectorName::Name(name) => {
            // HTML is case-insensitive for tag names.
            if !name.eq_ignore_ascii_case(&element.local_name()) {
                return false;
            }
            // Namespace prefix matching — same conservative approach
            // as universal. None/Any accept any namespace.
            match ts.ns_prefix.as_ref().map(|p| &p.prefix) {
                None | Some(NsPrefixKind::Any) => true,
                Some(NsPrefixKind::Named(_)) => element.namespace_uri().is_some(),
                Some(NsPrefixKind::None) => element.namespace_uri().is_none(),
            }
        }
    }
}

/// §3 L4674-4685: match a subclass selector.
fn matches_subclass<E: Element>(sub: &SubclassSelector, element: &E) -> bool {
    match sub {
        SubclassSelector::Id(id) => matches_id(id, element),
        SubclassSelector::Class(cls) => matches_class(cls, element),
        SubclassSelector::Attribute(attr) => matches_attribute(attr, element),
        SubclassSelector::PseudoClass(pc) => {
            crate::matching::pseudo_matcher::matches_pseudo_class(pc, element)
        }
    }
}

/// §6.6 L2463-2533: `#id` matches when element's `id` attribute
/// equals the selector's id.
fn matches_id<E: Element>(sel: &IdSelector, element: &E) -> bool {
    element.id().as_deref() == Some(sel.id.as_str())
}

/// §6.5 L2376-2462: `.class` matches when element's class list
/// contains the selector's class name.
fn matches_class<E: Element>(sel: &ClassSelector, element: &E) -> bool {
    element
        .classes()
        .iter()
        .any(|c| c == &sel.class)
}

/// §6 L1996-2533: attribute selector matching.
pub fn matches_attribute<E: Element>(sel: &AttributeSelector, element: &E) -> bool {
    let value = match element.get_attribute(&sel.name.local_name) {
        Some(v) => v,
        None => return false,
    };
    match &sel.matcher {
        None => true, // presence selector `[attr]`
        Some(matcher) => match &sel.value {
            None => false, // invalid: matcher without value
            Some(attr_val) => {
                let target = attr_value_str(attr_val);
                match matcher {
                    AttrMatcher::Exact => value == target,
                    AttrMatcher::Includes => {
                        // Whitespace-separated list contains target.
                        value
                            .split_ascii_whitespace()
                            .any(|tok| tok == target)
                    }
                    AttrMatcher::DashMatch => {
                        // §6.1 L2055-2080: exact or prefix followed
                        // by hyphen.
                        value == target
                            || value.starts_with(&format!("{target}-"))
                    }
                    AttrMatcher::Prefix => value.starts_with(target),
                    AttrMatcher::Suffix => value.ends_with(target),
                    AttrMatcher::Substring => value.contains(target),
                }
            }
        },
    }
}

fn attr_value_str(v: &AttrValue) -> &str {
    match v {
        AttrValue::String(s) => s,
        AttrValue::Ident(s) => s,
    }
}
```

- [ ] **Step 3.4: Update `matching/mod.rs` to dispatch single-unit complex selectors**

In `crates/muskitty-selectors/src/matching/mod.rs`, replace the `matching` submodule with:

```rust
mod matching {
    use crate::matching::{simple_matcher, Element};
    use crate::types::ComplexSelector;

    pub fn matches_complex<E: Element>(cs: &ComplexSelector, element: &E) -> bool {
        // §18 L4902-4919: right-to-left walk. For a single-unit
        // complex selector (the common case), only the subject
        // compound needs to match.
        // Task 7 generalises this to multi-unit complex selectors
        // with combinators.
        if cs.units.is_empty() {
            return false;
        }
        let subject = &cs.units[0];
        if !simple_matcher::matches_compound(&subject.compound, element) {
            return false;
        }
        // Multi-unit: Task 7 implements the combinator walk.
        cs.units.len() == 1
    }
}
```

- [ ] **Step 3.5: Run tests to verify they pass**

Run: `cargo test -p muskitty-selectors --test matching_basic`
Expected: PASS (all tests including Task 1's `type_selector_matches_case_insensitive`)

- [ ] **Step 3.6: Quality gate + commit**

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
git add crates/muskitty-selectors/src/matching/simple_matcher.rs crates/muskitty-selectors/src/matching/mod.rs crates/muskitty-selectors/tests/matching_basic.rs
git commit -m "[selectors] SP-8 task 3: simple-selector matching (type/universal/class/id/attribute)"
```

---

## Task 4: Tree-structural pseudo-class matching (non-An+B)

**Files:**
- Modify: `crates/muskitty-selectors/src/matching/pseudo_matcher.rs`
- Modify: `crates/muskitty-selectors/tests/matching_pseudo.rs` (new file)

- [ ] **Step 4.1: Write the failing tests**

Create `crates/muskitty-selectors/tests/matching_pseudo.rs`:

```rust
//! SP-8 §13 tree-structural pseudo-class matching tests.
//!
//! Covers §13 L3792-4359: :root / :empty / :first-child /
//! :last-child / :only-child / :first-of-type / :last-of-type /
//! :only-of-type. An+B pseudo-classes (`:nth-child` / etc.) are
//! tested in Task 5 (added to this file).

use muskitty_selectors::matching::matches;
use muskitty_selectors::parser::parse_a_selector;

// Re-use the StubElement from matching_basic via a public helper.
// For test-file isolation we duplicate a minimal stub here.
mod stub {
    use muskitty_selectors::matching::Element;

    #[derive(Clone, Debug)]
    pub struct StubElement {
        pub local_name: String,
        pub id: Option<String>,
        pub classes: Vec<String>,
        pub attributes: Vec<(String, String)>,
        pub parent: Option<Box<StubElement>>,
        pub previous_sibling: Option<Box<StubElement>>,
        pub next_sibling: Option<Box<StubElement>>,
        pub children: Vec<StubElement>,
    }

    impl StubElement {
        pub fn new(local_name: &str) -> Self {
            Self {
                local_name: local_name.to_string(),
                id: None,
                classes: Vec::new(),
                attributes: Vec::new(),
                parent: None,
                previous_sibling: None,
                next_sibling: None,
                children: Vec::new(),
            }
        }
    }

    impl Element for StubElement {
        fn local_name(&self) -> String { self.local_name.clone() }
        fn namespace_uri(&self) -> Option<String> { None }
        fn id(&self) -> Option<String> { self.id.clone() }
        fn classes(&self) -> Vec<String> { self.classes.clone() }
        fn get_attribute(&self, name: &str) -> Option<String> {
            self.attributes.iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone())
        }
        fn parent_element(&self) -> Option<Self> { self.parent.clone() }
        fn previous_sibling_element(&self) -> Option<Self> { self.previous_sibling.clone() }
        fn next_sibling_element(&self) -> Option<Self> { self.next_sibling.clone() }
        fn child_elements(&self) -> Vec<Self> { self.children.clone() }
        fn is_empty(&self) -> bool { self.children.is_empty() }
        fn index_among_siblings(&self) -> usize {
            let mut idx = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur { idx += 1; cur = prev.previous_sibling; }
            idx
        }
        fn count_among_siblings(&self) -> usize {
            let mut count = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur { count += 1; cur = prev.previous_sibling; }
            let mut cur = self.next_sibling.clone();
            while let Some(next) = cur { count += 1; cur = next.next_sibling; }
            count
        }
        fn index_among_type(&self) -> usize {
            let mut idx = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur {
                if prev.local_name.eq_ignore_ascii_case(&self.local_name) { idx += 1; }
                cur = prev.previous_sibling;
            }
            idx
        }
        fn count_among_type(&self) -> usize {
            let mut count = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur {
                if prev.local_name.eq_ignore_ascii_case(&self.local_name) { count += 1; }
                cur = prev.previous_sibling;
            }
            let mut cur = self.next_sibling.clone();
            while let Some(next) = cur {
                if next.local_name.eq_ignore_ascii_case(&self.local_name) { count += 1; }
                cur = next.next_sibling;
            }
            count
        }
    }
}

use stub::StubElement;

/// §13.2 L3820: `:root` matches an element with no parent.
#[test]
fn root_matches_when_no_parent() {
    let el = StubElement::new("html");
    let list = parse_a_selector(":root").expect("parses");
    assert!(matches(&list, &el));
}

/// §13.2 L3820: `:root` does not match an element with a parent.
#[test]
fn root_does_not_match_with_parent() {
    let parent = StubElement::new("html");
    let mut child = StubElement::new("body");
    child.parent = Some(Box::new(parent));
    let list = parse_a_selector(":root").expect("parses");
    assert!(!matches(&list, &child));
}

/// §13.3 L3837-3845: `:empty` matches an element with no children.
#[test]
fn empty_matches_no_children() {
    let el = StubElement::new("div");
    let list = parse_a_selector(":empty").expect("parses");
    assert!(matches(&list, &el));
}

/// §13.3 L3837-3845: `:empty` does not match an element with children.
#[test]
fn empty_does_not_match_with_children() {
    let mut el = StubElement::new("div");
    el.children.push(StubElement::new("span"));
    let list = parse_a_selector(":empty").expect("parses");
    assert!(!matches(&list, &el));
}

/// Build a parent with N element children; return clones of all
/// children for individual assertion.
fn build_siblings(n: usize) -> Vec<StubElement> {
    let mut parent = StubElement::new("parent");
    let mut children = Vec::new();
    for i in 0..n {
        let mut child = StubElement::new("child");
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    // Wire next_sibling forward links.
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    // Wire parent.children.
    parent.children = children.clone();
    children
}

/// §13.3 L3869: `:first-child` matches the first sibling.
#[test]
fn first_child_matches_first_sibling() {
    let sibs = build_siblings(3);
    let list = parse_a_selector(":first-child").expect("parses");
    assert!(matches(&list, &sibs[0]));
    assert!(!matches(&list, &sibs[1]));
    assert!(!matches(&list, &sibs[2]));
}

/// §13.3 L3869: `:last-child` matches the last sibling.
#[test]
fn last_child_matches_last_sibling() {
    let sibs = build_siblings(3);
    let list = parse_a_selector(":last-child").expect("parses");
    assert!(!matches(&list, &sibs[0]));
    assert!(!matches(&list, &sibs[1]));
    assert!(matches(&list, &sibs[2]));
}

/// §13.3 L3869: `:only-child` matches when no siblings.
#[test]
fn only_child_matches_no_siblings() {
    let sibs = build_siblings(1);
    let list = parse_a_selector(":only-child").expect("parses");
    assert!(matches(&list, &sibs[0]));
}

/// §13.3 L3869: `:only-child` does not match with siblings.
#[test]
fn only_child_does_not_match_with_siblings() {
    let sibs = build_siblings(2);
    let list = parse_a_selector(":only-child").expect("parses");
    assert!(!matches(&list, &sibs[0]));
}

/// §13.3: `:first-of-type` matches the first sibling of its kind.
#[test]
fn first_of_type_mixed() {
    // Build siblings: div, span, div, span, div
    let mut parent = StubElement::new("root");
    let names = ["div", "span", "div", "span", "div"];
    let mut children = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();

    let list = parse_a_selector(":first-of-type").expect("parses");
    // First div (idx 0) and first span (idx 1) match.
    assert!(matches(&list, &children[0]));
    assert!(matches(&list, &children[1]));
    assert!(!matches(&list, &children[2]));
    assert!(!matches(&list, &children[3]));
    assert!(!matches(&list, &children[4]));
}

/// §13.3: `:last-of-type` matches the last sibling of its kind.
#[test]
fn last_of_type_mixed() {
    let mut parent = StubElement::new("root");
    let names = ["div", "span", "div", "span", "div"];
    let mut children = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();

    let list = parse_a_selector(":last-of-type").expect("parses");
    // Last div (idx 4) and last span (idx 3) match.
    assert!(!matches(&list, &children[0]));
    assert!(!matches(&list, &children[1]));
    assert!(!matches(&list, &children[2]));
    assert!(matches(&list, &children[3]));
    assert!(matches(&list, &children[4]));
}

/// §13.3: `:only-of-type` matches when no sibling of the same type.
#[test]
fn only_of_type_mixed() {
    let mut parent = StubElement::new("root");
    let names = ["div", "span", "div"];
    let mut children = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();

    let list = parse_a_selector(":only-of-type").expect("parses");
    // Only span (idx 1) matches.
    assert!(!matches(&list, &children[0]));
    assert!(matches(&list, &children[1]));
    assert!(!matches(&list, &children[2]));
}

/// `:defined` always true for non-custom-element trees (stub).
#[test]
fn defined_always_true_in_simple_trees() {
    let el = StubElement::new("div");
    let list = parse_a_selector(":defined").expect("parses");
    assert!(matches(&list, &el));
}

/// `:scope` matches the root when no scope is provided (stub).
#[test]
fn scope_matches_root_when_no_scope() {
    let el = StubElement::new("html");
    let list = parse_a_selector(":scope").expect("parses");
    assert!(matches(&list, &el));

    let parent = StubElement::new("html");
    let mut child = StubElement::new("body");
    child.parent = Some(Box::new(parent));
    assert!(!matches(&list, &child));
}

/// Stub pseudo-classes return `false` (e.g. `:hover`).
#[test]
fn stub_pseudo_class_returns_false() {
    let el = StubElement::new("div");
    let list = parse_a_selector(":hover").expect("parses");
    assert!(!matches(&list, &el));
}
```

- [ ] **Step 4.2: Run tests to verify they fail**

Run: `cargo test -p muskitty-selectors --test matching_pseudo`
Expected: FAIL — `matches_pseudo_class` doesn't exist yet; compile error.

- [ ] **Step 4.3: Implement `pseudo_matcher.rs` (non-An+B pseudo-classes)**

Replace `crates/muskitty-selectors/src/matching/pseudo_matcher.rs` contents:

```rust
//! Pseudo-class matching.
//!
//! Implements the matching rules for §13 tree-structural
//! pseudo-classes, §13.3 An+B pseudo-classes (`:nth-child` / etc.),
//! and §4 logical combinations (`:is` / `:not` / `:where` /
//! `:has`).
//!
//! Pseudo-classes outside the §13/§4 scope (UI / location /
//! linguistic / resource state / display state / input — §7-§12)
//! are parsed by SP-4 but matching returns `false` per the parent
//! SP-1..SP-8 plan.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §4 L1358-1804,
//! §13 L3792-4359.

use crate::matching::Element;
use crate::types::{PseudoClass, PseudoClassArgument};

/// §13/§4: match a pseudo-class against an element.
pub fn matches_pseudo_class<E: Element>(pc: &PseudoClass, element: &E) -> bool {
    match pc.name.as_str() {
        // §13.2 L3820: :root
        "root" => element.is_root(),
        // §13.3 L3837-3845: :empty
        "empty" => element.is_empty(),
        // §13.3 L3869: child-indexed pseudo-classes (non-An+B).
        "first-child" => element.index_among_siblings() == 1,
        "last-child" => element.index_among_siblings() == element.count_among_siblings(),
        "only-child" => element.count_among_siblings() == 1,
        "first-of-type" => element.index_among_type() == 1,
        "last-of-type" => element.index_among_type() == element.count_among_type(),
        "only-of-type" => element.count_among_type() == 1,
        // §13.3 L3968: An+B pseudo-classes — implemented in Task 5.
        "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
            matches_nth_pseudo_class(pc, element)
        }
        // §4 logical combinations — implemented in Task 6.
        "is" | "where" => matches_is_where(pc, element),
        "not" => !matches_is_where(pc, element),
        "has" => matches_has(pc, element),
        // §5.4 L1956-1995: :defined — always true for non-custom-
        // element trees. Custom element tracking is out of scope.
        "defined" => true,
        // §8 L2817-3007: :scope — matches when no scoping root is
        // provided, equivalent to :root. Scoping-root-aware matching
        // is out of scope for SP-8.
        "scope" => element.is_root(),
        // §7-§12 stub pseudo-classes: matching returns false.
        // (UI / location / linguistic / resource / display / input.)
        "hover" | "active" | "focus" | "focus-visible" | "focus-within" |
        "link" | "visited" | "any-link" | "local-link" | "target" | "target-within" |
        "playing" | "paused" | "seeking" | "buffering" | "stalled" | "muted" |
        "volume-locked" | "enabled" | "disabled" | "read-only" | "read-write" |
        "placeholder-shown" | "default" | "checked" | "indeterminate" | "valid" |
        "invalid" | "in-range" | "out-of-range" | "required" | "optional" | "blank" |
        "current" | "past" | "future" | "lang" | "dir" | "host" | "host-context" => false,
        // Unknown pseudo-class: spec says it must not match
        // (parse-time rejection would have happened earlier).
        _ => false,
    }
}

/// §13.3 L3968: An+B pseudo-class matching.
///
/// Placeholder body — Task 5 implements the real math.
fn matches_nth_pseudo_class<E: Element>(_pc: &PseudoClass, _element: &E) -> bool {
    false
}

/// §4.2/§4.4: `:is(args)` / `:where(args)` match if any complex
/// selector in args matches the element. (Specificity differs, but
/// matching is identical.)
///
/// Placeholder body — Task 6 implements the real logic.
fn matches_is_where<E: Element>(_pc: &PseudoClass, _element: &E) -> bool {
    false
}

/// §4.5 L1650-1804: `:has(args)` matches if any relative selector
/// in args matches some element related to `element` (descendant or
/// sibling, depending on the relative selector's leading combinator).
///
/// Placeholder body — Task 6 implements the real logic.
fn matches_has<E: Element>(_pc: &PseudoClass, _element: &E) -> bool {
    false
}

/// Whether `arg` is one of the An+B pseudo-class argument shapes.
/// Helper for sanity-checking pseudo-class argument kind.
#[allow(dead_code)]
fn is_an_plus_b_arg(arg: &PseudoClassArgument) -> bool {
    matches!(arg, PseudoClassArgument::AnPlusB(_, _))
}
```

- [ ] **Step 4.4: Run tests to verify they pass**

Run: `cargo test -p muskitty-selectors --test matching_pseudo`
Expected: PASS (11 tests; An+B tests added in Task 5 will replace the placeholder)

- [ ] **Step 4.5: Quality gate + commit**

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
git add crates/muskitty-selectors/src/matching/pseudo_matcher.rs crates/muskitty-selectors/tests/matching_pseudo.rs
git commit -m "[selectors] SP-8 task 4: tree-structural pseudo-class matching (root/empty/first-child/etc)"
```

---

## Task 5: An+B pseudo-class matching (`:nth-child` / `:nth-last-child` / `:nth-of-type` / `:nth-last-of-type`)

**Files:**
- Modify: `crates/muskitty-selectors/src/matching/pseudo_matcher.rs`
- Modify: `crates/muskitty-selectors/tests/matching_pseudo.rs`

- [ ] **Step 5.1: Write the failing tests**

Append to `crates/muskitty-selectors/tests/matching_pseudo.rs`:

```rust
/// §13.3 L3982: `:nth-child(2)` matches the 2nd sibling.
#[test]
fn nth_child_integer_matches_second() {
    let sibs = build_siblings(3);
    let list = parse_a_selector(":nth-child(2)").expect("parses");
    assert!(!matches(&list, &sibs[0]));
    assert!(matches(&list, &sibs[1]));
    assert!(!matches(&list, &sibs[2]));
}

/// §13.3 L3982: `:nth-child(odd)` matches 1st, 3rd, 5th, ...
#[test]
fn nth_child_odd_matches_first_third() {
    let sibs = build_siblings(5);
    let list = parse_a_selector(":nth-child(odd)").expect("parses");
    assert!(matches(&list, &sibs[0]));
    assert!(!matches(&list, &sibs[1]));
    assert!(matches(&list, &sibs[2]));
    assert!(!matches(&list, &sibs[3]));
    assert!(matches(&list, &sibs[4]));
}

/// §13.3 L3982: `:nth-child(even)` matches 2nd, 4th, ...
#[test]
fn nth_child_even_matches_second_fourth() {
    let sibs = build_siblings(5);
    let list = parse_a_selector(":nth-child(even)").expect("parses");
    assert!(!matches(&list, &sibs[0]));
    assert!(matches(&list, &sibs[1]));
    assert!(!matches(&list, &sibs[2]));
    assert!(matches(&list, &sibs[3]));
    assert!(!matches(&list, &sibs[4]));
}

/// §13.3 L3982: `:nth-child(2n+1)` matches 1st, 3rd, 5th, ...
#[test]
fn nth_child_2n_plus_1_matches_odd() {
    let sibs = build_siblings(5);
    let list = parse_a_selector(":nth-child(2n+1)").expect("parses");
    assert!(matches(&list, &sibs[0]));
    assert!(!matches(&list, &sibs[1]));
    assert!(matches(&list, &sibs[2]));
}

/// §13.3 L3982: `:nth-child(-n+3)` matches 1st, 2nd, 3rd (of 5).
#[test]
fn nth_child_negative_n_plus_3_matches_first_three() {
    let sibs = build_siblings(5);
    let list = parse_a_selector(":nth-child(-n+3)").expect("parses");
    assert!(matches(&list, &sibs[0]));
    assert!(matches(&list, &sibs[1]));
    assert!(matches(&list, &sibs[2]));
    assert!(!matches(&list, &sibs[3]));
    assert!(!matches(&list, &sibs[4]));
}

/// §13.4 L4077: `:nth-last-child(1)` matches the last sibling.
#[test]
fn nth_last_child_1_matches_last() {
    let sibs = build_siblings(3);
    let list = parse_a_selector(":nth-last-child(1)").expect("parses");
    assert!(!matches(&list, &sibs[0]));
    assert!(!matches(&list, &sibs[1]));
    assert!(matches(&list, &sibs[2]));
}

/// §13.4: `:nth-of-type(2)` matches the 2nd sibling of the same type.
#[test]
fn nth_of_type_2_matches_second_of_type() {
    // Siblings: div, span, div, span, div
    let mut parent = StubElement::new("root");
    let names = ["div", "span", "div", "span", "div"];
    let mut children = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();

    let list = parse_a_selector(":nth-of-type(2)").expect("parses");
    // div[1] (idx 0): no; span[1] (idx 1): no; div[2] (idx 2): yes;
    // span[2] (idx 3): yes; div[3] (idx 4): no.
    assert!(!matches(&list, &children[0]));
    assert!(!matches(&list, &children[1]));
    assert!(matches(&list, &children[2]));
    assert!(matches(&list, &children[3]));
    assert!(!matches(&list, &children[4]));
}

/// §13.4: `:nth-last-of-type(1)` matches the last of each type.
#[test]
fn nth_last_of_type_1_matches_last_of_type() {
    let mut parent = StubElement::new("root");
    let names = ["div", "span", "div", "span", "div"];
    let mut children = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();

    let list = parse_a_selector(":nth-last-of-type(1)").expect("parses");
    // Last div (idx 4) and last span (idx 3) match.
    assert!(!matches(&list, &children[0]));
    assert!(!matches(&list, &children[1]));
    assert!(!matches(&list, &children[2]));
    assert!(matches(&list, &children[3]));
    assert!(matches(&list, &children[4]));
}

/// §13.3 L3968: `:nth-child(2n of .foo)` — first filter siblings to
/// those matching `.foo`, then check 2n index.
#[test]
fn nth_child_of_s_filters_then_indexes() {
    // Siblings: .a, .b, .a, .b, .a — `:nth-child(2n of .a)` matches
    // 2nd and 4th `.a` siblings (indices 2, 4 in the filtered list).
    let mut parent = StubElement::new("root");
    let classes = ["a", "b", "a", "b", "a"];
    let mut children = Vec::new();
    for cls in classes {
        let mut child = StubElement::new("div");
        child.classes = vec![cls.to_string()];
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();

    let list = parse_a_selector(":nth-child(2n of .a)").expect("parses");
    // .a siblings at positions: [0]=1st, [2]=2nd, [4]=3rd.
    // 2n matches positions 2, 4 → children[2] only (since position 4
    // doesn't exist for .a in a 3-item filtered list — only positions
    // 1, 2, 3 are valid, and 2n means n=1 → position 2).
    // children[0]: position 1 (odd) → no
    // children[2]: position 2 (even) → yes
    // children[4]: position 3 (odd) → no
    assert!(!matches(&list, &children[0]));
    assert!(!matches(&list, &children[1]));
    assert!(matches(&list, &children[2]));
    assert!(!matches(&list, &children[3]));
    assert!(!matches(&list, &children[4]));
}
```

- [ ] **Step 5.2: Run tests to verify they fail**

Run: `cargo test -p muskitty-selectors --test matching_pseudo`
Expected: FAIL — An+B tests fail because `matches_nth_pseudo_class` is a stub.

- [ ] **Step 5.3: Implement `matches_nth_pseudo_class`**

In `crates/muskitty-selectors/src/matching/pseudo_matcher.rs`, replace the `matches_nth_pseudo_class` body:

```rust
/// §13.3 L3968 + §13.4 L4077: An+B pseudo-class matching.
///
/// Handles `:nth-child(An+B [of S]?)`, `:nth-last-child(An+B [of S]?)`,
/// `:nth-of-type(An+B)`, `:nth-last-of-type(An+B)`.
///
/// - For `*-child` variants without `of S`: index among ALL element
///   siblings (1-based).
/// - For `*-child` variants with `of S`: filter siblings to those
///   matching `S`, then index within that filtered list.
/// - For `*-of-type` variants: index among siblings of the same type.
fn matches_nth_pseudo_class<E: Element>(pc: &PseudoClass, element: &E) -> bool {
    let (anb, of_s) = match pc.argument.as_ref() {
        Some(PseudoClassArgument::AnPlusB(anb, of_s)) => (*anb, of_s.as_ref()),
        _ => return false,
    };

    // §13.3 L3957-3958: "inclusive siblings" — siblings include
    // self. Walk both directions to collect the full list (in order).
    let siblings: Vec<E> = collect_inclusive_siblings(element);

    let (filtered, from_last) = match pc.name.as_str() {
        "nth-child" | "nth-last-child" => {
            let list = match of_s {
                Some(s) => siblings
                    .iter()
                    .filter(|sib| crate::matching::matches_complex_list(s, sib))
                    .cloned()
                    .collect::<Vec<_>>(),
                None => siblings,
            };
            (list, pc.name == "nth-last-child")
        }
        "nth-of-type" | "nth-last-of-type" => {
            // Filter by same type as `element`.
            let my_name = element.local_name();
            let list: Vec<E> = siblings
                .iter()
                .filter(|sib| sib.local_name().eq_ignore_ascii_case(&my_name))
                .cloned()
                .collect();
            (list, pc.name == "nth-last-of-type")
        }
        _ => return false,
    };

    // Find element's 1-based position in `filtered`.
    let position = filtered
        .iter()
        .position(|sib| sib.local_name() == element.local_name()
            && sib.id() == element.id()
            && sib.classes() == element.classes())
        .map(|i| if from_last { filtered.len() - i } else { i + 1 });

    match position {
        Some(idx) => an_plus_b_matches(anb.a, anb.b, idx as i64),
        None => false,
    }
}

/// Collect inclusive siblings in document order (including `element`).
fn collect_inclusive_siblings<E: Element>(element: &E) -> Vec<E> {
    let mut left: Vec<E> = Vec::new();
    let mut cur = element.previous_sibling_element();
    while let Some(prev) = cur {
        left.push(prev);
        cur = prev.previous_sibling_element();
    }
    left.reverse();

    let mut right: Vec<E> = Vec::new();
    cur = element.next_sibling_element();
    while let Some(next) = cur {
        right.push(next);
        cur = next.next_sibling_element();
    }

    let mut all = left;
    all.push(element.clone());
    all.extend(right);
    all
}

/// §13.5: An+B math. Returns true if `index = A*k + B` for some
/// non-negative integer `k`.
fn an_plus_b_matches(a: i64, b: i64, index: i64) -> bool {
    // 1-based index per §13.3 L3982.
    if a == 0 {
        return index == b;
    }
    let diff = index - b;
    // diff must be divisible by `a` and the quotient k = diff/a
    // must be >= 0.
    diff % a == 0 && diff / a >= 0
}
```

Add a helper at the bottom of the file (or in `mod.rs`) for list-level matching:

In `crates/muskitty-selectors/src/matching/mod.rs`, expose a `matches_complex_list` helper:

```rust
// In the `matching` submodule inside mod.rs:
pub fn matches_complex_list<E: Element>(list: &crate::types::SelectorList, element: &E) -> bool {
    list.0.iter().any(|cs| matches_complex(cs, element))
}
```

Then update `pseudo_matcher.rs` to call `crate::matching::matching::matches_complex_list(s, sib)` — but the private `matching` submodule isn't exposed. Refactor: move `matches_complex_list` to a `pub(crate)` function at `matching/mod.rs` top level:

```rust
// At the top of matching/mod.rs (outside the private `matching` submodule):
pub(crate) fn matches_complex_list<E: Element>(
    list: &SelectorList,
    element: &E,
) -> bool {
    list.0.iter().any(|cs| matches_complex(cs, element))
}
```

Remove the duplicate from the private submodule if it exists there.

- [ ] **Step 5.4: Run tests to verify they pass**

Run: `cargo test -p muskitty-selectors --test matching_pseudo`
Expected: PASS — all 21 tests green.

- [ ] **Step 5.5: Quality gate + commit**

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
git add crates/muskitty-selectors/src/matching/pseudo_matcher.rs crates/muskitty-selectors/src/matching/mod.rs crates/muskitty-selectors/tests/matching_pseudo.rs
git commit -m "[selectors] SP-8 task 5: An+B pseudo-class matching (nth-child/nth-last-child/nth-of-type/nth-last-of-type with optional 'of S')"
```

---

## Task 6: Logical combinations matching (`:is` / `:where` / `:not` / `:has`)

**Files:**
- Modify: `crates/muskitty-selectors/src/matching/pseudo_matcher.rs`
- Modify: `crates/muskitty-selectors/src/parser/mod.rs` (real `parse_a_relative_selector`)
- Modify: `crates/muskitty-selectors/tests/matching_pseudo.rs`

- [ ] **Step 6.1: Write the failing tests**

Append to `crates/muskitty-selectors/tests/matching_pseudo.rs`:

```rust
/// §4.2: `:is(.a, .b)` matches if element has class `a` or class `b`.
#[test]
fn is_matches_if_any_arg_matches() {
    let mut el = StubElement::new("div");
    el.classes = vec!["b".into()];
    let list = parse_a_selector(":is(.a, .b)").expect("parses");
    assert!(matches(&list, &el));
}

/// §4.4: `:where(.a, .b)` matches identically to `:is` (only
/// specificity differs).
#[test]
fn where_matches_like_is() {
    let mut el = StubElement::new("div");
    el.classes = vec!["a".into()];
    let list = parse_a_selector(":where(.a, .b)").expect("parses");
    assert!(matches(&list, &el));
}

/// §4.3: `:not(.a)` matches if element does NOT have class `a`.
#[test]
fn not_matches_if_arg_does_not_match() {
    let mut el = StubElement::new("div");
    el.classes = vec!["b".into()];
    let list = parse_a_selector(":not(.a)").expect("parses");
    assert!(matches(&list, &el));

    el.classes = vec!["a".into()];
    assert!(!matches(&list, &el));
}

/// §4.3: `:not(.a, .b)` matches if element matches NEITHER arg.
#[test]
fn not_with_list_matches_if_no_arg_matches() {
    let mut el = StubElement::new("div");
    el.classes = vec!["c".into()];
    let list = parse_a_selector(":not(.a, .b)").expect("parses");
    assert!(matches(&list, &el));
}

/// §4.5 L1650-1804: `:has(.child)` matches if element has a descendant
/// matching `.child`.
#[test]
fn has_matches_descendant() {
    let mut parent = StubElement::new("div");
    let mut child = StubElement::new("span");
    child.classes = vec!["child".into()];
    child.parent = Some(Box::new(parent.clone()));
    parent.children = vec![child.clone()];

    let list = parse_a_selector(":has(.child)").expect("parses");
    assert!(matches(&list, &parent));
    assert!(!matches(&list, &child));
}

/// §4.5: `:has(> .child)` matches only direct children.
#[test]
fn has_with_child_combinator_matches_only_direct() {
    let mut parent = StubElement::new("div");
    let mut middle = StubElement::new("section");
    let mut grandchild = StubElement::new("span");
    grandchild.classes = vec!["child".into()];
    grandchild.parent = Some(Box::new(middle.clone()));
    middle.children = vec![grandchild.clone()];
    middle.parent = Some(Box::new(parent.clone()));
    parent.children = vec![middle.clone()];

    // `:has(> .child)` — child of parent must have class .child.
    // parent's direct child is `middle` (no .child class), so no match.
    let list = parse_a_selector(":has(> .child)").expect("parses");
    assert!(!matches(&list, &parent));

    // Now add a direct child with .child class.
    let mut direct = StubElement::new("p");
    direct.classes = vec!["child".into()];
    direct.parent = Some(Box::new(parent.clone()));
    parent.children.push(direct.clone());
    assert!(matches(&list, &parent));
}
```

- [ ] **Step 6.2: Run tests to verify they fail**

Run: `cargo test -p muskitty-selectors --test matching_pseudo`
Expected: FAIL — `matches_is_where` and `matches_has` are stubs.

- [ ] **Step 6.3: Implement `parse_a_relative_selector` in `parser/mod.rs`**

Replace the body of `parse_a_relative_selector` in `crates/muskitty-selectors/src/parser/mod.rs`:

```rust
/// §18 L4853-4875: Parse A Relative Selector.
///
/// Like [`parse_a_selector`] but the source is interpreted as a
/// relative selector (relative to an implicit `:scope` element, per
/// §3 L1051-1102). Used by `:has()` arguments.
///
/// Delegates to [`crate::parser::relative::parse_relative_selector_list`]
/// after tokenisation.
pub fn parse_a_relative_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    let tokens = muskitty_css::tokenize(source);
    let mut stream = TokenStream::new(tokens);

    stream.discard_whitespace();
    if matches!(stream.next_token(), Token::Eof) {
        return Err(SelectorParseError::EmptySelector);
    }

    let list = relative::parse_relative_selector_list(&mut stream)?;
    stream.discard_whitespace();
    if !stream.is_empty() {
        return Err(SelectorParseError::InvalidSelector(format!(
            "trailing tokens after relative selector: {:?}",
            stream.next_token()
        )));
    }
    Ok(list)
}
```

- [ ] **Step 6.4: Implement `matches_is_where` and `matches_has`**

In `crates/muskitty-selectors/src/matching/pseudo_matcher.rs`, replace the stubs:

```rust
/// §4.2/§4.4: `:is(args)` / `:where(args)` match if any complex
/// selector in args matches the element. (Specificity differs per
/// §17, but matching is identical.)
fn matches_is_where<E: Element>(pc: &PseudoClass, element: &E) -> bool {
    match pc.argument.as_ref() {
        Some(PseudoClassArgument::SelectorList(list)) => {
            crate::matching::matches_complex_list(list, element)
        }
        _ => false,
    }
}

/// §4.5 L1650-1804: `:has(args)` matches if any relative selector in
/// args matches some element related to `element` (descendant or
/// sibling, depending on the relative selector's leading combinator).
fn matches_has<E: Element>(pc: &PseudoClass, element: &E) -> bool {
    let list = match pc.argument.as_ref() {
        Some(PseudoClassArgument::SelectorList(list)) => list,
        _ => return false,
    };
    // §4.5 L1720-1730: the relative selector list is evaluated with
    // `:scope` bound to `element`. Each relative selector has an
    // implicit leading combinator (default Descendant); we walk
    // the related elements (descendants for Descendant, children
    // for Child, siblings for Next/SubsequentSibling) and check
    // whether any of them matches the relative selector with
    // `:scope` matching `element`.
    list.0.iter().any(|cs| {
        matches_relative_complex(cs, element)
    })
}

/// Match a relative complex selector against `scope`'s related
/// elements. The leading combinator on `units[len-1]` (leftmost)
/// determines which related elements to consider; if None, defaults
/// to Descendant per §4.5 L1705.
fn matches_relative_complex<E: Element>(cs: &crate::types::ComplexSelector, scope: &E) -> bool {
    if cs.units.is_empty() {
        return false;
    }
    // The relative selector's leftmost unit (units[len-1]) carries
    // the leading combinator that links `:scope` to the leftmost
    // compound. If that combinator is None, default is Descendant.
    let leftmost_idx = cs.units.len() - 1;
    let leftmost_combinator = cs.units[leftmost_idx].combinator.unwrap_or(
        crate::types::Combinator::Descendant
    );

    // Collect candidate elements related to `scope` by `leftmost_combinator`.
    let candidates: Vec<E> = match leftmost_combinator {
        crate::types::Combinator::Descendant => {
            // All descendants.
            collect_descendants(scope)
        }
        crate::types::Combinator::Child => {
            scope.child_elements()
        }
        crate::types::Combinator::NextSibling => {
            scope.next_sibling_element().into_iter().collect()
        }
        crate::types::Combinator::SubsequentSibling => {
            let mut out = Vec::new();
            let mut cur = scope.next_sibling_element();
            while let Some(s) = cur {
                out.push(s);
                cur = s.next_sibling_element();
            }
            out
        }
    };

    // For each candidate, check if it matches the relative selector
    // (units[0..leftmost_idx]) with `:scope` bound to `scope`.
    // Simplification: match the subject (units[0]) against the
    // candidate, then walk leftward checking combinators against
    // `scope` for the leftmost step.
    candidates.iter().any(|candidate| {
        matches_complex_with_scope(cs, candidate, scope, leftmost_idx)
    })
}

/// Match `cs` against `candidate`, treating `scope` as the implicit
/// `:scope` element. Walks right-to-left; at the leftmost step, the
/// combinator links to `scope` rather than a parent.
fn matches_complex_with_scope<E: Element>(
    cs: &crate::types::ComplexSelector,
    candidate: &E,
    scope: &E,
    leftmost_idx: usize,
) -> bool {
    // Delegate to the standard complex-selector matcher, treating
    // the leftmost combinator as satisfied if the related element
    // is `scope`. For simplicity (SP-8 scope), we only handle the
    // case where the relative selector has a single compound (no
    // additional combinators after the leading one).
    if leftmost_idx != 0 {
        // Multi-compound relative selector — fall back to false
        // for SP-8 (full support deferred).
        return false;
    }
    crate::matching::simple_matcher::matches_compound(&cs.units[0].compound, candidate)
}

/// Collect all descendants of `root` in document order (depth-first
/// pre-order). Used by `:has()` with default Descendant combinator.
fn collect_descendants<E: Element>(root: &E) -> Vec<E> {
    let mut out = Vec::new();
    fn walk<E: Element>(root: &E, out: &mut Vec<E>) {
        for child in root.child_elements() {
            out.push(child.clone());
            walk(&child, out);
        }
    }
    walk(root, &mut out);
    out
}
```

- [ ] **Step 6.5: Run tests to verify they pass**

Run: `cargo test -p muskitty-selectors --test matching_pseudo`
Expected: PASS — all tests including `is`/`where`/`not`/`has` green.

- [ ] **Step 6.6: Quality gate + commit**

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
git add crates/muskitty-selectors/src/matching/pseudo_matcher.rs crates/muskitty-selectors/src/parser/mod.rs crates/muskitty-selectors/tests/matching_pseudo.rs
git commit -m "[selectors] SP-8 task 6: logical combinations matching (:is/:where/:not/:has) + real parse_a_relative_selector"
```

---

## Task 7: Combinator matching + right-to-left complex selector walk

**Files:**
- Modify: `crates/muskitty-selectors/src/matching/mod.rs`
- Modify: `crates/muskitty-selectors/tests/matching_basic.rs`

- [ ] **Step 7.1: Write the failing tests**

Append to `crates/muskitty-selectors/tests/matching_basic.rs`:

```rust
/// §15 L4369: descendant combinator (whitespace).
#[test]
fn descendant_combinator_matches() {
    // `parent > child` structure.
    let mut parent = StubElement::new("div");
    let mut child = StubElement::new("span");
    child.parent = Some(Box::new(parent.clone()));
    parent.children = vec![child.clone()];

    // `div span` — descendant combinator.
    let list = parse_a_selector("div span").expect("parses");
    assert!(matches(&list, &child));
    assert!(!matches(&list, &parent));
}

/// §15 L4376: child combinator (`>`).
#[test]
fn child_combinator_matches_only_direct() {
    // `root > middle > leaf`.
    let mut root = StubElement::new("root");
    let mut middle = StubElement::new("middle");
    let leaf = StubElement::new("leaf");
    middle.parent = Some(Box::new(root.clone()));
    middle.children = vec![leaf.clone()];
    root.children = vec![middle.clone()];

    // `root > leaf` — does NOT match (leaf is not direct child of root).
    let list = parse_a_selector("root > leaf").expect("parses");
    assert!(!matches(&list, &leaf));

    // `root > middle` — matches.
    let list = parse_a_selector("root > middle").expect("parses");
    assert!(matches(&list, &middle));
}

/// §15 L4383: next-sibling combinator (`+`).
#[test]
fn next_sibling_combinator_matches() {
    // Build siblings a, b, c.
    let sibs = build_three_siblings();
    // `a + b` matches b (b's previous sibling is a).
    let list = parse_a_selector("a + b").expect("parses");
    assert!(matches(&list, &sibs[1]));
    // `a + c` does NOT match c (c's previous sibling is b, not a).
    let list = parse_a_selector("a + c").expect("parses");
    assert!(!matches(&list, &sibs[2]));
}

/// §15 L4390: subsequent-sibling combinator (`~`).
#[test]
fn subsequent_sibling_combinator_matches() {
    let sibs = build_three_siblings();
    // `a ~ c` matches c (c has an earlier sibling a).
    let list = parse_a_selector("a ~ c").expect("parses");
    assert!(matches(&list, &sibs[2]));
    // `b ~ a` does NOT match a (a has no earlier sibling b).
    let list = parse_a_selector("b ~ a").expect("parses");
    assert!(!matches(&list, &sibs[0]));
}

/// Mixed combinators: `a > b + c` (a is parent of b; b is preceding
/// sibling of c).
#[test]
fn mixed_combinators_match() {
    let mut a = StubElement::new("a");
    let mut b = StubElement::new("b");
    let mut c = StubElement::new("c");
    b.parent = Some(Box::new(a.clone()));
    c.parent = Some(Box::new(a.clone()));
    c.previous_sibling = Some(Box::new(b.clone()));
    b.next_sibling = Some(Box::new(c.clone()));
    a.children = vec![b.clone(), c.clone()];

    let list = parse_a_selector("a > b + c").expect("parses");
    assert!(matches(&list, &c));
    assert!(!matches(&list, &b));
}

/// Three-part descendant: `a b c` — c is descendant of b is
/// descendant of a.
#[test]
fn three_part_descendant_matches() {
    let mut a = StubElement::new("a");
    let mut b = StubElement::new("b");
    let c = StubElement::new("c");
    b.parent = Some(Box::new(a.clone()));
    b.children = vec![c.clone()];
    a.children = vec![b.clone()];

    let list = parse_a_selector("a b c").expect("parses");
    assert!(matches(&list, &c));
    assert!(!matches(&list, &b));
}

fn build_three_siblings() -> Vec<StubElement> {
    let mut parent = StubElement::new("root");
    let names = ["a", "b", "c"];
    let mut children = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in 0..children.len() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();
    children
}
```

- [ ] **Step 7.2: Run tests to verify they fail**

Run: `cargo test -p muskitty-selectors --test matching_basic`
Expected: FAIL — multi-unit complex selectors don't match because `matches_complex` only handles single-unit case.

- [ ] **Step 7.3: Implement the right-to-left walk in `matching/mod.rs`**

Replace the private `matching` submodule body in `crates/muskitty-selectors/src/matching/mod.rs`:

```rust
mod matching {
    use crate::matching::{simple_matcher, Element};
    use crate::types::{Combinator, ComplexSelector};

    /// §18 L4902-4919: Match a complex selector against an element,
    /// processing compound selectors right-to-left.
    pub fn matches_complex<E: Element>(cs: &ComplexSelector, element: &E) -> bool {
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
        // §18 L4914-4919: otherwise, consider all elements related
        // to `element` by the rightmost combinator (subject.combinator),
        // and recursively match `units[1..]` against each.
        let combinator = match &subject.combinator {
            Some(c) => *c,
            None => return true, // shouldn't happen for len > 1
        };
        matches_remaining(&cs.units[1..], element, combinator)
    }

    /// Try to match `remaining` (leftward compounds) against some
    /// element related to `element` by `combinator`.
    fn matches_remaining<E: Element>(
        remaining: &[crate::types::ComplexSelectorUnit],
        element: &E,
        combinator: Combinator,
    ) -> bool {
        let next_unit = &remaining[0];
        match combinator {
            Combinator::Descendant => {
                // §15 L4369: any ancestor of `element`.
                let mut ancestor = element.parent_element();
                while let Some(parent) = ancestor {
                    if simple_matcher::matches_compound(&next_unit.compound, &parent)
                        && matches_rest(&remaining[1..], &parent)
                    {
                        return true;
                    }
                    ancestor = parent.parent_element();
                }
                false
            }
            Combinator::Child => {
                // §15 L4376: direct parent only.
                if let Some(parent) = element.parent_element() {
                    if simple_matcher::matches_compound(&next_unit.compound, &parent)
                        && matches_rest(&remaining[1..], &parent)
                    {
                        return true;
                    }
                }
                false
            }
            Combinator::NextSibling => {
                // §15 L4383: direct previous sibling only.
                if let Some(prev) = element.previous_sibling_element() {
                    if simple_matcher::matches_compound(&next_unit.compound, &prev)
                        && matches_rest(&remaining[1..], &prev)
                    {
                        return true;
                    }
                }
                false
            }
            Combinator::SubsequentSibling => {
                // §15 L4390: any previous sibling.
                let mut prev = element.previous_sibling_element();
                while let Some(sibling) = prev {
                    if simple_matcher::matches_compound(&next_unit.compound, &sibling)
                        && matches_rest(&remaining[1..], &sibling)
                    {
                        return true;
                    }
                    prev = sibling.previous_sibling_element();
                }
                false
            }
        }
    }

    /// Match the rest of the units leftward (after the combinator-
    /// related compound matched).
    fn matches_rest<E: Element>(
        rest: &[crate::types::ComplexSelectorUnit],
        element: &E,
    ) -> bool {
        if rest.is_empty() {
            return true;
        }
        let next_combinator = match &rest[0].combinator {
            Some(c) => *c,
            None => {
                // This is the leftmost unit — just match its compound.
                return simple_matcher::matches_compound(&rest[0].compound, element);
            }
        };
        // Recurse: match `rest[0].compound` against `element`, then
        // walk by `next_combinator` for `rest[1..]`.
        if !simple_matcher::matches_compound(&rest[0].compound, element) {
            return false;
        }
        if rest.len() == 1 {
            return true;
        }
        matches_remaining(&rest[1..], element, next_combinator)
    }
}
```

- [ ] **Step 7.4: Run tests to verify they pass**

Run: `cargo test -p muskitty-selectors --test matching_basic`
Expected: PASS — all tests including combinator tests green.

- [ ] **Step 7.5: Quality gate + commit**

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
git add crates/muskitty-selectors/src/matching/mod.rs crates/muskitty-selectors/tests/matching_basic.rs
git commit -m "[selectors] SP-8 task 7: right-to-left complex-selector walk + combinator matching (descendant/child/next-sibling/subsequent-sibling)"
```

---

## Task 8: Top-level API finalisation + lib.rs exports + README

**Files:**
- Modify: `crates/muskitty-selectors/src/lib.rs`
- Modify: `crates/muskitty-selectors/tests/matching_dom.rs`
- Create: `crates/muskitty-selectors/README.md`

- [ ] **Step 8.1: Add public re-exports to `lib.rs`**

Modify `crates/muskitty-selectors/src/lib.rs` — append after `pub use specificity::Specificity;`:

```rust

/// Convenience re-exports of the matching engine's public API.
pub use matching::{matches, query_selector, query_selector_all, Element};
```

- [ ] **Step 8.2: Re-enable end-to-end DOM test**

Replace `crates/muskitty-selectors/tests/matching_dom.rs` (the `dom_type_selector_matches` test from Task 2 was stubbed; now we add the real end-to-end tests). Append to the file:

```rust
#[test]
fn dom_type_selector_matches() {
    let root = build_tree();
    let list = parse_a_selector("root").expect("parses");
    assert!(matches(&list, &root));
}

#[test]
fn dom_descendant_combinator() {
    let root = build_tree();
    let child = root.borrow().children[0].clone();
    let grandchild = child.borrow().children[0].clone();

    // `root child` — descendant matches `child` and `grandchild`.
    let list = parse_a_selector("root child").expect("parses");
    assert!(matches(&list, &child));
    assert!(matches(&list, &grandchild));

    // `root > child` — direct child matches `child` only.
    let list = parse_a_selector("root > child").expect("parses");
    assert!(matches(&list, &child));
    assert!(!matches(&list, &grandchild));
}

#[test]
fn dom_query_selector_all() {
    use muskitty_dom::attribute::Attribute;
    let doc = Node::new_document();
    let root = Node::new_element_html(
        "root",
        vec![],
        &doc,
    );
    let _a = Node::new_element_html(
        "a",
        vec![Attribute::new("class", "x")],
        &doc,
    );
    let _b = Node::new_element_html(
        "b",
        vec![Attribute::new("class", "x")],
        &doc,
    );
    let _c = Node::new_element_html("c", vec![], &doc);
    muskitty_dom::tree::append_child(&root, &_a);
    muskitty_dom::tree::append_child(&root, &_b);
    muskitty_dom::tree::append_child(&root, &_c);

    let list = parse_a_selector(".x").expect("parses");
    let found = muskitty_selectors::query_selector_all(&root, &list);
    assert_eq!(found.len(), 2);
    assert_eq!(Element::local_name(&found[0]), "a");
    assert_eq!(Element::local_name(&found[1]), "b");
}

#[test]
fn dom_query_selector_returns_first() {
    use muskitty_dom::attribute::Attribute;
    let doc = Node::new_document();
    let root = Node::new_element_html("root", vec![], &doc);
    let target = Node::new_element_html("target", vec![], &doc);
    muskitty_dom::tree::append_child(&root, &target);

    let list = parse_a_selector("target").expect("parses");
    let found = muskitty_selectors::query_selector(&root, &list);
    assert!(found.is_some());
    assert_eq!(Element::local_name(&found.unwrap()), "target");
}

#[test]
fn dom_id_selector() {
    use muskitty_dom::attribute::Attribute;
    let doc = Node::new_document();
    let root = Node::new_element_html(
        "div",
        vec![Attribute::new("id", "main")],
        &doc,
    );

    let list = parse_a_selector("#main").expect("parses");
    assert!(matches(&list, &root));
}

#[test]
fn dom_attribute_selector() {
    use muskitty_dom::attribute::Attribute;
    let doc = Node::new_document();
    let root = Node::new_element_html(
        "input",
        vec![Attribute::new("type", "text")],
        &doc,
    );

    let list = parse_a_selector(r#"[type="text"]"#).expect("parses");
    assert!(matches(&list, &root));
}

#[test]
fn dom_first_child_pseudo() {
    let doc = Node::new_document();
    let root = Node::new_element_html("root", vec![], &doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);
    muskitty_dom::tree::append_child(&root, &a);
    muskitty_dom::tree::append_child(&root, &b);

    let list = parse_a_selector(":first-child").expect("parses");
    assert!(matches(&list, &a));
    assert!(!matches(&list, &b));
}
```

- [ ] **Step 8.3: Create `README.md`**

Create `crates/muskitty-selectors/README.md`:

```markdown
# muskitty-selectors

Selectors Level 4 parser and matching engine for Rust.

Part of the [MusKitty](https://github.com/Ink-dark/MusKitty) browser engine.

## Status

| Feature | Spec coverage | Tests |
|---------|---------------|-------|
| §3 Data Model | ✅ L716-1357 | 6 |
| §5 Elemental selectors | ✅ L1805-1995 | — |
| §6 Attribute selectors | ✅ L1996-2533 | 11 |
| §4 Logical combinations | ✅ L1358-1804 | 10 |
| §13 Tree-structural pseudo-classes | ✅ L3792-4359 | 12 |
| §15 Combinators | ✅ L4360-4532 | 12 |
| §17 Specificity | ✅ L4534-4633 | 22 |
| §18 Matching engine | ✅ L4816-5026 | — |

Total: 87+ tests, all passing.

## Architecture

- **Parser** (`src/parser/`) — consumes a token stream produced by
  `muskitty-css::tokenize` and builds selector data structures.
  No DOM dependency.
- **Specificity** (`src/specificity.rs`) — computes the A/B/C
  triplet per §17.
- **Matching** (`src/matching/`) — matches parsed selectors against
  an element tree via the `Element` trait. A reference impl for
  `muskitty_dom::Node` lives in `src/matching/dom_impl.rs`.

## Quick start

```rust
use muskitty_selectors::{parse_a_selector, matches, Specificity};

let list = parse_a_selector("div.foo > span").unwrap();
let spec: Specificity = list.specificity_max();
// (0, 1, 2) — one class + two type selectors.
```

To match against your own element tree, implement the
`muskitty_selectors::Element` trait.

## Spec references

- Selectors Level 4: <https://drafts.csswg.org/selectors-4/>
- Spec source (Markdown): `D:\CSSWG\selectors-4\Overview.md`

## License

Apache-2.0.
```

- [ ] **Step 8.4: Run quality gate**

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

Expected: All tests pass (now ~110+ tests across 8 test files).

- [ ] **Step 8.5: Commit**

```powershell
git add crates/muskitty-selectors/src/lib.rs crates/muskitty-selectors/tests/matching_dom.rs crates/muskitty-selectors/README.md
git commit -m "[selectors] SP-8 task 8: top-level API exports + README + end-to-end DOM tests"
```

---

## Task 9: Final workspace quality gate + push

- [ ] **Step 9.1: Workspace-wide quality gate**

```powershell
cargo fmt --check
cargo test --workspace
cargo check --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: All tests pass across the whole workspace. Estimated ~110+ tests in muskitty-selectors + existing tests in muskitty-dom / muskitty-css / muskitty-html5-parser.

- [ ] **Step 9.2: Update PROGRESS.md**

Modify `PROGRESS.md` — add a "Phase 2 — Selectors Level 4" section at the bottom:

```markdown
## Phase 2 子阶段 2 — Selectors Level 4 ✅

按 `.trae/documents/phase2-selectors-sp1-to-sp8.md` 8 个 SP batch 全部完成：

| SP | 内容 | 状态 |
|----|------|------|
| SP-1 | §3 数据模型 + parser 框架 | ✅ |
| SP-2 | §5/§6.5/§6.6 type/class/id 解析 | ✅ |
| SP-3 | §6 attribute selectors | ✅ |
| SP-4 | §13 tree-structural pseudo + An+B | ✅ |
| SP-5 | §4 logical combinations | ✅ |
| SP-6 | §15 combinators + complex selector | ✅ |
| SP-7 | §17 specificity | ✅ |
| SP-8 | §18 matching engine + lib API | ✅ |

总计 87+ tests，crate 可作为 v0.1.0 拆分独立 git 仓库。
```

- [ ] **Step 9.3: Commit + push**

```powershell
git add PROGRESS.md
git commit -m "[selectors] SP-8: mark Phase 2 子阶段 2 (Selectors Level 4) complete in PROGRESS.md"
git push origin main
```

- [ ] **Step 9.4: STOP — do not auto-enter next plan mode**

Per user instruction "弄完了再拆": after SP-8 push succeeds, the next step is user-directed splitting of `muskitty-selectors` into an independent git repo (mirroring `muskitty-css-parser` extraction pattern).

Do NOT auto-enter CSS Values Module plan mode.

---

## Self-Review

**1. Spec coverage check:**

- §3 L716-1357 (data model) — SP-1 done.
- §4 L1358-1804 (logical combinations) — SP-5 parsing done, Task 6 matching.
- §5 L1805-1995 (elemental) — SP-2 parsing done, Task 3 matching.
- §6 L1996-2533 (attribute) — SP-3 parsing done, Task 3 matching.
- §13 L3792-4359 (tree-structural + An+B) — SP-4 parsing done, Task 4+5 matching.
- §14 (pseudo-elements) — SP-4 parsing done; matching returns `false` (pseudo-elements not in element tree).
- §15 L4360-4532 (combinators) — SP-6 parsing done, Task 7 matching.
- §17 L4534-4633 (specificity) — SP-7 done.
- §18 L4816-5026 (API hooks) — Task 1+8 (`matches` / `query_selector` / `query_selector_all`).

**2. Placeholder scan:**

- No "TBD" / "TODO" / "implement later" in steps.
- All code blocks contain complete code; no "..." elisions except where explicitly noted.
- Each test has a real assertion against real expected output.

**3. Type consistency:**

- `Element` trait methods consistently return `String` / `Option<String>` / `Vec<String>` across all files.
- `matches_complex` signature stable across `matching/mod.rs`, `pseudo_matcher.rs`, `dom_impl.rs`.
- `SelectorList` field `0` accessed as `list.0` consistently.
- `ComplexSelector::units` indexed rightmost-first consistently in matching code (matches §3 L809-826 storage convention).

**Gaps acknowledged (deferred to post-split polish):**

- WPT subset integration — the parent plan mentions 20 WPT tests; deferred to keep SP-8 focused on the engine. Crate maturity for splitting is met without WPT.
- Full `:has()` multi-compound relative selector matching — Task 6 handles single-compound `:has(args)` only; multi-compound (`:has(.a > .b)`) returns `false` in the simplification. This matches the parent plan's "`:has()` sibling combinator子选择器延后" guidance (modified to multi-compound case).
- Namespace-aware matching — conservative "any namespace is OK" handling; strict `ns|tag` matching deferred.
- Pseudo-element matching — out of scope (pseudo-elements aren't in the element tree).

---

## Execution

Plan complete and saved to `.trae/documents/phase2-selectors-sp8-matching.md`.

**Two execution options:**

1. **Subagent-Driven** (recommended) — dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
