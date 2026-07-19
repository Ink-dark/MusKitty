//! SP-7 §17 specificity calculation tests.
//!
//! Covers §17 L4534-4633: the A/B/C triplet computation, comparison
//! rules, and the special cases for `:is`/`:not`/`:has`/`:where`/
//! `:nth-child`/`:nth-last-child`.

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::specificity::Specificity;
use muskitty_selectors::types::{
    ComplexSelector, ComplexSelectorUnit, CompoundSelector, SelectorList,
};

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

fn single_compound_of(list: &SelectorList) -> &CompoundSelector {
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

/// §17 L4573-4577: `:is(em, #foo)` → (1,0,0). The `:is()` argument
/// list has `em` = (0,0,1) and `#foo` = (1,0,0); max is (1,0,0).
#[test]
fn is_takes_max_of_args() {
    assert_eq!(specificity_of(":is(em, #foo)"), Specificity::new(1, 0, 0));
}

/// §17 L4590-4593: `:not(em, strong#foo)` → (1,0,1). Same max rule.
#[test]
fn not_takes_max_of_args() {
    assert_eq!(
        specificity_of(":not(em, strong#foo)"),
        Specificity::new(1, 0, 1)
    );
}

/// §17 L4579-4582: `.qux:where(em, #foo#bar#baz)` → (0,1,0).
/// `:where()` always contributes zero specificity regardless of args.
#[test]
fn where_zero_specificity() {
    assert_eq!(
        specificity_of(".qux:where(em, #foo#bar#baz)"),
        Specificity::new(0, 1, 0)
    );
}

/// §17 L4584-4588: `:nth-child(even of li, .item)` → (0,2,0).
/// The pseudo-class contributes (0,1,0), plus max of `li` (0,0,1) and
/// `.item` (0,1,0) which is (0,1,0). Total = (0,2,0).
#[test]
fn nth_child_of_s_adds_max() {
    assert_eq!(
        specificity_of(":nth-child(even of li, .item)"),
        Specificity::new(0, 2, 0)
    );
}

/// §17 L4560-4564: `:nth-child(even)` without `of S` → (0,1,0) (just
/// the pseudo-class).
#[test]
fn nth_child_without_of_s() {
    assert_eq!(
        specificity_of(":nth-child(even)"),
        Specificity::new(0, 1, 0)
    );
}

/// §17 L4555-4558: `:has(.a)` argument is a relative selector. The
/// implicit `:scope` pseudo-class is part of the complex selector,
/// contributing (0,1,0); `.a` contributes (0,1,0). Max over the list
/// is (0,2,0). `:has()` itself is replaced by this max.
#[test]
fn has_takes_max_of_relative_args() {
    assert_eq!(specificity_of(":has(.a)"), Specificity::new(0, 2, 0));
}

/// §17 L4624-4625: `#s12:not(FOO)` → (1,0,1). The `#s12` is (1,0,0);
/// `:not(FOO)` is replaced by max of `FOO` = (0,0,1). Total = (1,0,1).
#[test]
fn compound_id_with_not_foo() {
    assert_eq!(specificity_of("#s12:not(FOO)"), Specificity::new(1, 0, 1));
}

/// §17 L4536: top-level `ComplexSelector::specificity()` method.
#[test]
fn complex_selector_method() {
    let list: SelectorList = parse_a_selector("UL OL LI.red").expect("parses");
    let cs: &ComplexSelector = &list.0[0];
    assert_eq!(cs.specificity(), Specificity::new(0, 1, 3));
}

/// §17 L4547-4548: `SelectorList::specificity_max()` returns the max
/// specificity over all complex selectors in the list.
#[test]
fn selector_list_max_method() {
    // List with 3 selectors of increasing specificity.
    let list: SelectorList = parse_a_selector("div, .a, #id").expect("parses");
    assert_eq!(list.0.len(), 3);
    // div = (0,0,1); .a = (0,1,0); #id = (1,0,0). Max = (1,0,0).
    assert_eq!(list.specificity_max(), Specificity::new(1, 0, 0));
}

/// §17 L4547-4548: empty list → (0,0,0).
#[test]
fn empty_list_max() {
    let empty = SelectorList::default();
    assert_eq!(empty.specificity_max(), Specificity::default());
}

/// `Specificity` is re-exported at the crate root for ergonomics.
#[test]
fn specificity_re_exported_at_root() {
    let _: muskitty_selectors::Specificity = muskitty_selectors::Specificity::new(1, 2, 3);
}
