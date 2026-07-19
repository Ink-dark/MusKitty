//! SP-7 Task 2: verify `:nth-child(An+B of S)` parsing.
//!
//! Per §13.3 L3968, `:nth-child()` and `:nth-last-child()` accept an
//! optional `of S` clause where S is a selector list. §17 L4560-4564
//! uses this list for the special specificity rule. `:nth-of-type()`
//! and `:nth-last-of-type()` do NOT accept `of S` per §13.6/§13.7.

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    ComplexSelectorUnit, CompoundSelector, PseudoClass, PseudoClassArgument, SelectorList,
    SubclassSelector,
};

fn single_compound(list: &SelectorList) -> &CompoundSelector {
    assert_eq!(list.0.len(), 1);
    let unit: &ComplexSelectorUnit = &list.0[0].units[0];
    assert!(unit.combinator.is_none());
    &unit.compound
}

fn single_pseudo_class(compound: &CompoundSelector) -> &PseudoClass {
    assert_eq!(compound.subclasses.len(), 1);
    match &compound.subclasses[0] {
        SubclassSelector::PseudoClass(pc) => pc,
        other => panic!("expected PseudoClass, got {:?}", other),
    }
}

/// §13.3 L3968: `:nth-child(2n of .a, .b)` — the `of S` clause captures
/// a 2-element SelectorList argument alongside the An+B value.
#[test]
fn nth_child_with_of_selector_list() {
    let list =
        parse_a_selector(":nth-child(2n of .a, .b)").expect(":nth-child(2n of .a, .b) parses");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "nth-child");
    match &pc.argument {
        Some(PseudoClassArgument::AnPlusB(anb, Some(of_s))) => {
            assert_eq!(anb.a, 2);
            assert_eq!(anb.b, 0);
            assert_eq!(of_s.0.len(), 2, "expected 2 selectors in of S");
        }
        other => panic!("expected AnPlusB(2n, Some(list)), got {:?}", other),
    }
}

/// §13.3 L3968: `:nth-child(even)` without `of S` — the optional list
/// is `None`.
#[test]
fn nth_child_without_of() {
    let list = parse_a_selector(":nth-child(even)").expect(":nth-child(even) parses");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "nth-child");
    match &pc.argument {
        Some(PseudoClassArgument::AnPlusB(anb, None)) => {
            assert_eq!(anb.a, 2);
            assert_eq!(anb.b, 0);
        }
        other => panic!("expected AnPlusB(2n, None), got {:?}", other),
    }
}

/// §13.4 L4077: `:nth-last-child(odd of .x)` — same parsing rule as
/// `:nth-child()` for the `of S` clause.
#[test]
fn nth_last_child_with_of() {
    let list =
        parse_a_selector(":nth-last-child(odd of .x)").expect(":nth-last-child(odd of .x) parses");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "nth-last-child");
    match &pc.argument {
        Some(PseudoClassArgument::AnPlusB(anb, Some(of_s))) => {
            assert_eq!(anb.a, 2);
            assert_eq!(anb.b, 1);
            assert_eq!(of_s.0.len(), 1);
        }
        other => panic!("expected AnPlusB(2n+1, Some(list)), got {:?}", other),
    }
}

/// §13.6/§13.7: `:nth-of-type(2n)` does NOT accept `of S` syntax. If
/// the parser encounters `of` after An+B, it must fail.
#[test]
fn nth_of_type_rejects_of_clause() {
    let result = parse_a_selector(":nth-of-type(2n of .a)");
    assert!(
        result.is_err(),
        ":nth-of-type should not accept 'of S' clause"
    );
}
