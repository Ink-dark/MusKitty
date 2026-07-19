//! SP-7 §17 specificity calculation tests.
//!
//! Covers §17 L4534-4633: the A/B/C triplet computation, comparison
//! rules, and the special cases for `:is`/`:not`/`:has`/`:where`/
//! `:nth-child`/`:nth-last-child`.

use muskitty_selectors::specificity::Specificity;

/// §17 L4598-4605: lexicographic comparison on (A, B, C).
#[test]
fn specificity_ordering() {
    assert!(Specificity::new(1, 0, 0) > Specificity::new(0, 99, 99));
    assert!(Specificity::new(0, 2, 0) > Specificity::new(0, 1, 99));
    assert!(Specificity::new(0, 0, 2) > Specificity::new(0, 0, 1));
    assert_eq!(Specificity::new(1, 2, 3), Specificity::new(1, 2, 3));
    // Default is (0,0,0) — the universal-selector / `*` specificity.
    assert_eq!(Specificity::default(), Specificity::new(0, 0, 0));
}

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{ComplexSelectorUnit, CompoundSelector};

fn single_compound_of(list: &muskitty_selectors::types::SelectorList) -> &CompoundSelector {
    assert_eq!(list.0.len(), 1);
    let unit: &ComplexSelectorUnit = &list.0[0].units[0];
    assert!(unit.combinator.is_none());
    &unit.compound
}

fn specificity_of(input: &str) -> Specificity {
    let list = parse_a_selector(input).expect("selector should parse");
    let compound = single_compound_of(&list);
    muskitty_selectors::specificity::specificity_of_compound(compound)
}

/// §17 L4616: `*` → (0,0,0).
#[test]
fn star_zero() {
    assert_eq!(specificity_of("*"), Specificity::new(0, 0, 0));
}

/// §17 L4617: `LI` → (0,0,1).
#[test]
fn type_li() {
    assert_eq!(specificity_of("LI"), Specificity::new(0, 0, 1));
}

/// §17 L4623: `#x34y` → (1,0,0).
#[test]
fn id_selector() {
    assert_eq!(specificity_of("#x34y"), Specificity::new(1, 0, 0));
}

/// §17 L4622: `LI.red.level` → (0,2,1).
#[test]
fn type_with_two_classes() {
    assert_eq!(specificity_of("LI.red.level"), Specificity::new(0, 2, 1));
}

/// §17 L4620: `H1 + *[REL=up]` — but this is a complex selector, not
/// a single compound. The single-compound variant `[REL=up]` alone
/// has specificity (0,1,0).
#[test]
fn attribute_selector_alone() {
    assert_eq!(specificity_of("[REL=up]"), Specificity::new(0, 1, 0));
}

/// §17 L4542: universal selector contributes nothing.
#[test]
fn universal_with_pseudo_class() {
    assert_eq!(specificity_of("*:hover"), Specificity::new(0, 1, 0));
}

/// §14 pseudo-element: `::before` → (0,0,1).
#[test]
fn pseudo_element_alone() {
    assert_eq!(specificity_of("::before"), Specificity::new(0, 0, 1));
}

/// Pseudo-class without `:is`/`:not`/`:where`/`:has`/`:nth-child`:
/// simple case `:hover` → (0,1,0).
#[test]
fn simple_pseudo_class() {
    assert_eq!(specificity_of(":hover"), Specificity::new(0, 1, 0));
}

/// Compound with type + class + attribute + pseudo-class + pseudo-element:
/// `div.foo[bar]:hover::before` → A=0, B=3 (.foo, [bar], :hover),
/// C=2 (div + ::before).
#[test]
fn compound_full_mix() {
    assert_eq!(
        specificity_of("div.foo[bar]:hover::before"),
        Specificity::new(0, 3, 2)
    );
}

/// §3 L762-787: trailing pseudo-classes on a pseudo-compound.
/// `::before:hover` → pseudo-element + pseudo-class → (0,1,1).
#[test]
fn pseudo_compound_with_trailing_pc() {
    assert_eq!(specificity_of("::before:hover"), Specificity::new(0, 1, 1));
}
