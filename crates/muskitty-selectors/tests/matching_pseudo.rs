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
        #[allow(dead_code)]
        pub namespace_uri: Option<String>,
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
        fn local_name(&self) -> String {
            self.local_name.clone()
        }
        fn namespace_uri(&self) -> Option<String> {
            None
        }
        fn id(&self) -> Option<String> {
            self.id.clone()
        }
        fn classes(&self) -> Vec<String> {
            self.classes.clone()
        }
        fn get_attribute(&self, name: &str) -> Option<String> {
            self.attributes
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone())
        }
        fn parent_element(&self) -> Option<Self> {
            self.parent.clone().map(|b| *b)
        }
        fn previous_sibling_element(&self) -> Option<Self> {
            self.previous_sibling.clone().map(|b| *b)
        }
        fn next_sibling_element(&self) -> Option<Self> {
            self.next_sibling.clone().map(|b| *b)
        }
        fn child_elements(&self) -> Vec<Self> {
            self.children.clone()
        }
        fn is_empty(&self) -> bool {
            self.children.is_empty()
        }
        fn index_among_siblings(&self) -> usize {
            let mut idx = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur {
                idx += 1;
                cur = prev.previous_sibling;
            }
            idx
        }
        fn count_among_siblings(&self) -> usize {
            // Prefer parent.children (single source of truth) to avoid
            // stale next_sibling clones in Box<StubElement> chains.
            if let Some(parent) = &self.parent {
                return parent.children.len();
            }
            let mut count = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur {
                count += 1;
                cur = prev.previous_sibling;
            }
            let mut cur = self.next_sibling.clone();
            while let Some(next) = cur {
                count += 1;
                cur = next.next_sibling;
            }
            count
        }
        fn index_among_type(&self) -> usize {
            let mut idx = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur {
                if prev.local_name.eq_ignore_ascii_case(&self.local_name) {
                    idx += 1;
                }
                cur = prev.previous_sibling;
            }
            idx
        }
        fn count_among_type(&self) -> usize {
            if let Some(parent) = &self.parent {
                return parent
                    .children
                    .iter()
                    .filter(|c| c.local_name.eq_ignore_ascii_case(&self.local_name))
                    .count();
            }
            let mut count = 1;
            let mut cur = self.previous_sibling.clone();
            while let Some(prev) = cur {
                if prev.local_name.eq_ignore_ascii_case(&self.local_name) {
                    count += 1;
                }
                cur = prev.previous_sibling;
            }
            let mut cur = self.next_sibling.clone();
            while let Some(next) = cur {
                if next.local_name.eq_ignore_ascii_case(&self.local_name) {
                    count += 1;
                }
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
    let mut children: Vec<StubElement> = Vec::new();
    for _ in 0..n {
        let mut child = StubElement::new("child");
        // parent snapshot here has empty children; will be re-snapshotted below.
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    // Wire next_sibling forward links in REVERSE so each snapshot
    // captures the already-populated forward chain (required by
    // `nth-last-*` backward walks).
    for i in (0..children.len()).rev() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    // Wire parent.children and re-snapshot parent into each child so
    // count_among_siblings / count_among_type (which read parent.children)
    // observe the populated children list.
    parent.children = children.clone();
    for child in &mut children {
        child.parent = Some(Box::new(parent.clone()));
    }
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

/// Build siblings with mixed types for first/last-of-type tests.
fn build_mixed_type_siblings() -> Vec<StubElement> {
    let mut parent = StubElement::new("root");
    let names = ["div", "span", "div", "span", "div"];
    let mut children: Vec<StubElement> = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in (0..children.len()).rev() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();
    for child in &mut children {
        child.parent = Some(Box::new(parent.clone()));
    }
    children
}

/// §13.3: `:first-of-type` matches the first sibling of its kind.
#[test]
fn first_of_type_mixed() {
    let children = build_mixed_type_siblings();
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
    let children = build_mixed_type_siblings();
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
    let mut children: Vec<StubElement> = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in (0..children.len()).rev() {
        if i + 1 < children.len() {
            children[i].next_sibling = Some(Box::new(children[i + 1].clone()));
        }
    }
    parent.children = children.clone();
    for child in &mut children {
        child.parent = Some(Box::new(parent.clone()));
    }

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

// ---------------------------------------------------------------------------
// Task 5: An+B pseudo-class tests
// ---------------------------------------------------------------------------

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
    let mut children: Vec<StubElement> = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in (0..children.len()).rev() {
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
    let mut children: Vec<StubElement> = Vec::new();
    for name in names {
        let mut child = StubElement::new(name);
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in (0..children.len()).rev() {
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

/// §13.3 L3968: `:nth-child(2n of .a)` — first filter siblings to
/// those matching `.a`, then check 2n index.
#[test]
fn nth_child_of_s_filters_then_indexes() {
    // Siblings: .a, .b, .a, .b, .a — `:nth-child(2n of .a)` matches
    // 2nd and 4th `.a` siblings (indices 2, 4 in the filtered list).
    let mut parent = StubElement::new("root");
    let classes = ["a", "b", "a", "b", "a"];
    let mut children: Vec<StubElement> = Vec::new();
    for cls in classes {
        let mut child = StubElement::new("div");
        child.classes = vec![cls.to_string()];
        child.parent = Some(Box::new(parent.clone()));
        if let Some(prev) = children.last() {
            child.previous_sibling = Some(Box::new(prev.clone()));
        }
        children.push(child);
    }
    for i in (0..children.len()).rev() {
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

// ---------------------------------------------------------------------------
// Task 6: Logical combinations tests (:is / :where / :not / :has)
// ---------------------------------------------------------------------------

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
