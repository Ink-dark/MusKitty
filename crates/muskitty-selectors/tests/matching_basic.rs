//! SP-8 §18 matching engine — basic tests.
//!
//! Covers §18 L4878-4919 (Match a Selector Against an Element) and
//! the simple-selector matchers in §3 L858-873 + §5 + §6.

use muskitty_selectors::matching::matches;
use muskitty_selectors::matching::Element;
use muskitty_selectors::parser::parse_a_selector;

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
    #[allow(dead_code)]
    parent: Option<Box<StubElement>>,
    #[allow(dead_code)]
    previous_sibling: Option<Box<StubElement>>,
    #[allow(dead_code)]
    next_sibling: Option<Box<StubElement>>,
    #[allow(dead_code)]
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
    fn local_name(&self) -> String {
        self.local_name.clone()
    }
    fn namespace_uri(&self) -> Option<String> {
        self.namespace_uri.clone()
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

/// §3 L870: type selector matches local_name (case-insensitive HTML).
#[test]
fn type_selector_matches_case_insensitive() {
    let list = parse_a_selector("div").expect("parses");
    let el = StubElement::new("DIV");
    assert!(matches(&list, &el));
}
