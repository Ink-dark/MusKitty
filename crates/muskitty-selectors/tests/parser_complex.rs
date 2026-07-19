//! SP-6 unit tests for §15 combinators and complete complex-selector
//! parsing.
//!
//! Covers:
//! - §15 L4369: descendant (whitespace) combinator
//! - §15 L4376: child (`>`) combinator
//! - §15 L4383: next-sibling (`+`) combinator
//! - §15 L4390: subsequent-sibling (`~`) combinator
//! - §3 L4664-4665: complex-selector grammar (multiple compounds joined
//!   by combinators)
//! - §3 L1317-1347: invalid selector error handling (trailing combinator,
//!   trailing comma, empty input)
//!
//! Storage convention: `units[0]` is the subject (rightmost compound in
//! source), `units[len-1]` is the leftmost. The combinator on
//! `units[idx]` links it to `units[idx+1]` (the next leftward unit).
//! The leftmost unit has `combinator == None`.

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    Combinator, ComplexSelector, ComplexSelectorUnit, SelectorList, SubclassSelector,
    TypeSelectorName,
};

/// Helper: extract the single complex selector from a list that should
/// contain exactly one.
fn single_complex(list: &SelectorList) -> &ComplexSelector {
    assert_eq!(
        list.0.len(),
        1,
        "expected 1 complex selector, got {}",
        list.0.len()
    );
    &list.0[0]
}

/// Helper: assert the unit's compound has a type selector with the
/// given tag name.
fn assert_subject_tag(unit: &ComplexSelectorUnit, expected: &str) {
    let ts = unit
        .compound
        .type_selector
        .as_ref()
        .expect("expected type selector");
    match &ts.name {
        TypeSelectorName::Name(n) => assert_eq!(n, expected),
        TypeSelectorName::Universal => panic!("expected Name({}), got Universal", expected),
    }
}

/// §3 L4664 + §15 L4360: `"div.foo"` → 1 unit, no combinator.
#[test]
fn single_compound() {
    let list = parse_a_selector("div.foo").expect("div.foo should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 1);
    assert!(cs.units[0].combinator.is_none());
    assert_subject_tag(&cs.units[0], "div");
    // subject also has a .foo class subclass
    let class_count = cs.units[0]
        .compound
        .subclasses
        .iter()
        .filter(|s| matches!(s, SubclassSelector::Class(_)))
        .count();
    assert_eq!(class_count, 1);
}

/// §15 L4369: whitespace → Descendant combinator.
/// `"a b"` → units = [{ b, Some(Descendant) }, { a, None }]
#[test]
fn descendant_whitespace() {
    let list = parse_a_selector("a b").expect("a b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(
        cs.units[0].combinator,
        Some(Combinator::Descendant),
        "subject carries Descendant combinator linking to leftmost"
    );
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None, "leftmost has no combinator");
}

/// §15 L4376: `>` → Child combinator. `"a > b"` → 2 units with Child.
#[test]
fn child_explicit() {
    let list = parse_a_selector("a > b").expect("a > b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::Child));
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §15 L4383: `+` → NextSibling combinator.
#[test]
fn next_sibling() {
    let list = parse_a_selector("a + b").expect("a + b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::NextSibling));
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §15 L4390: `~` → SubsequentSibling combinator.
#[test]
fn subsequent_sibling() {
    let list = parse_a_selector("a ~ b").expect("a ~ b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::SubsequentSibling));
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §3 L4664: three-part complex selector.
/// `"a b c"` → 3 units: [c (Desc), b (Desc), a (None)]
#[test]
fn three_part_descendant() {
    let list = parse_a_selector("a b c").expect("a b c should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 3);
    assert_subject_tag(&cs.units[0], "c");
    assert_eq!(cs.units[0].combinator, Some(Combinator::Descendant));
    assert_subject_tag(&cs.units[1], "b");
    assert_eq!(cs.units[1].combinator, Some(Combinator::Descendant));
    assert_subject_tag(&cs.units[2], "a");
    assert_eq!(cs.units[2].combinator, None);
}

/// §15 mixed combinators: `"a > b + c"` → 3 units.
/// Rightmost-first: units[0]=c (subject, NextSibling to b),
/// units[1]=b (Child to a), units[2]=a (None).
#[test]
fn mixed_combinators() {
    let list = parse_a_selector("a > b + c").expect("a > b + c should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 3);
    assert_subject_tag(&cs.units[0], "c");
    assert_eq!(cs.units[0].combinator, Some(Combinator::NextSibling));
    assert_subject_tag(&cs.units[1], "b");
    assert_eq!(cs.units[1].combinator, Some(Combinator::Child));
    assert_subject_tag(&cs.units[2], "a");
    assert_eq!(cs.units[2].combinator, None);
}

/// §15 + §13: pseudo-class on rightmost compound.
/// `"a > b:hover"` → 2 units, subject `b` has :hover subclass.
#[test]
fn combinator_with_pseudo_class() {
    let list = parse_a_selector("a > b:hover").expect("a > b:hover should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::Child));
    // subject has :hover pseudo-class as a subclass
    let pseudo = cs.units[0]
        .compound
        .subclasses
        .iter()
        .find_map(|s| match s {
            SubclassSelector::PseudoClass(pc) => Some(pc),
            _ => None,
        })
        .expect("expected :hover pseudo-class");
    assert_eq!(pseudo.name, "hover");
    assert!(pseudo.argument.is_none());
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §3 L1317-1347: trailing combinator is invalid.
/// `"a >"` → Err (any SelectorParseError variant).
#[test]
fn trailing_combinator_fails() {
    let result = parse_a_selector("a >");
    assert!(result.is_err(), "trailing combinator should fail");
}

/// §3 L4651-4653: selector list with 3 items.
/// `"a, b, c"` → SelectorList with 3 complex selectors.
#[test]
fn selector_list_three_items() {
    let list = parse_a_selector("a, b, c").expect("a, b, c should parse");
    assert_eq!(list.0.len(), 3);
    for (i, cs) in list.0.iter().enumerate() {
        assert_eq!(cs.units.len(), 1);
        let expected = match i {
            0 => "a",
            1 => "b",
            2 => "c",
            _ => unreachable!(),
        };
        assert_subject_tag(&cs.units[0], expected);
    }
}

/// §3 L4651-4653: trailing comma is invalid.
/// `"a,"` → Err.
#[test]
fn trailing_comma_fails() {
    let result = parse_a_selector("a,");
    assert!(result.is_err(), "trailing comma should fail");
}

/// §3 L1317-1347 + error::EmptySelector: empty input → Err(EmptySelector).
#[test]
fn empty_string_fails() {
    use muskitty_selectors::error::SelectorParseError;
    let result = parse_a_selector("");
    assert!(
        result.is_err(),
        "empty input should fail with EmptySelector"
    );
    assert!(
        matches!(result, Err(SelectorParseError::EmptySelector)),
        "expected EmptySelector, got {:?}",
        result
    );
}
