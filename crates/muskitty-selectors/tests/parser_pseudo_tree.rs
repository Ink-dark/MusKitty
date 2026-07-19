//! SP-4 unit tests for §13 tree-structural pseudo-classes, An+B, and
//! §14 pseudo-element parsing.
//!
//! Covers:
//! - §13.1 `:root`, `:empty` (simple value-less pseudo-classes)
//! - §13.3 `:nth-child(An+B)` with the full range of An+B forms
//!   (integer, odd, even, n, 2n+1, -n+3)
//! - §14 modern pseudo-element (`::name`) and legacy single-colon
//!   pseudo-element (`:before`, `:after`)
//! - Unknown pseudo-class rejection (§3.7 invalid-selector handling)

use muskitty_selectors::error::SelectorParseError;
use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    AnPlusB, ComplexSelectorUnit, CompoundSelector, PseudoClass, PseudoClassArgument,
    PseudoCompoundSelector, SelectorList, SubclassSelector,
};

/// Helper: extract the single (rightmost, only) compound selector from
/// a selector list that should contain exactly one complex selector
/// with exactly one compound unit.
fn single_compound(list: &SelectorList) -> &CompoundSelector {
    assert_eq!(
        list.0.len(),
        1,
        "expected single complex selector, got {}",
        list.0.len()
    );
    let unit: &ComplexSelectorUnit = &list.0[0].units[0];
    assert!(
        unit.combinator.is_none(),
        "expected no combinator in SP-4 single-compound selectors"
    );
    &unit.compound
}

/// Helper: extract the single pseudo-class subclass from a compound.
fn single_pseudo_class(compound: &CompoundSelector) -> &PseudoClass {
    assert_eq!(
        compound.subclasses.len(),
        1,
        "expected 1 subclass, got {}",
        compound.subclasses.len()
    );
    match &compound.subclasses[0] {
        SubclassSelector::PseudoClass(pc) => pc,
        other => panic!("expected PseudoClass, got {:?}", other),
    }
}

/// `":root"` parses as a value-less pseudo-class with name "root"
/// (§13.2 L3812-3843).
#[test]
fn pseudo_class_root() {
    let list = parse_a_selector(":root").expect(":root should parse");
    let compound = single_compound(&list);
    assert!(compound.type_selector.is_none());
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "root");
    assert!(pc.argument.is_none());
}

/// `":empty"` parses as a value-less pseudo-class with name "empty"
/// (§13.1.2 L3798-3810).
#[test]
fn pseudo_class_empty() {
    let list = parse_a_selector(":empty").expect(":empty should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "empty");
    assert!(pc.argument.is_none());
}

/// `":nth-child(2)"` parses as AnPlusB { a: 0, b: 2 } — the
/// `<integer>` form per CSS Syntax §7 L3032.
#[test]
fn pseudo_class_nth_child_simple() {
    let list = parse_a_selector(":nth-child(2)").expect(":nth-child(2) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "nth-child");
    match pc
        .argument
        .as_ref()
        .expect("nth-child must have AnPlusB argument")
    {
        PseudoClassArgument::AnPlusB(an, _) => assert_eq!(*an, AnPlusB { a: 0, b: 2 }),
        other => panic!("expected AnPlusB argument, got {:?}", other),
    }
}

/// `":nth-child(odd)"` parses as AnPlusB { a: 2, b: 1 } (CSS Syntax
/// §7 L3015-3018).
#[test]
fn pseudo_class_nth_child_odd() {
    let list = parse_a_selector(":nth-child(odd)").expect(":nth-child(odd) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    match pc
        .argument
        .as_ref()
        .expect("odd must have AnPlusB argument")
    {
        PseudoClassArgument::AnPlusB(an, _) => assert_eq!(*an, AnPlusB { a: 2, b: 1 }),
        other => panic!("expected AnPlusB argument, got {:?}", other),
    }
}

/// `":nth-child(even)"` parses as AnPlusB { a: 2, b: 0 } (CSS Syntax
/// §7 L3019-3022).
#[test]
fn pseudo_class_nth_child_even() {
    let list = parse_a_selector(":nth-child(even)").expect(":nth-child(even) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    match pc
        .argument
        .as_ref()
        .expect("even must have AnPlusB argument")
    {
        PseudoClassArgument::AnPlusB(an, _) => assert_eq!(*an, AnPlusB { a: 2, b: 0 }),
        other => panic!("expected AnPlusB argument, got {:?}", other),
    }
}

/// `":nth-child(n)"` parses as AnPlusB { a: 1, b: 0 } — the bare `<n>`
/// form (CSS Syntax §7 L3041-3049).
#[test]
fn pseudo_class_nth_child_n() {
    let list = parse_a_selector(":nth-child(n)").expect(":nth-child(n) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    match pc.argument.as_ref().expect("n must have AnPlusB argument") {
        PseudoClassArgument::AnPlusB(an, _) => assert_eq!(*an, AnPlusB { a: 1, b: 0 }),
        other => panic!("expected AnPlusB argument, got {:?}", other),
    }
}

/// `":nth-child(2n+1)"` parses as AnPlusB { a: 2, b: 1 } — the
/// `<n-dimension> ['+'|'-'] <signless-integer>` form (CSS Syntax
/// §7 L3066-3074).
#[test]
fn pseudo_class_nth_child_2n_plus_1() {
    let list = parse_a_selector(":nth-child(2n+1)").expect(":nth-child(2n+1) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    match pc
        .argument
        .as_ref()
        .expect("2n+1 must have AnPlusB argument")
    {
        PseudoClassArgument::AnPlusB(an, _) => assert_eq!(*an, AnPlusB { a: 2, b: 1 }),
        other => panic!("expected AnPlusB argument, got {:?}", other),
    }
}

/// `":nth-child(-n+3)"` parses as AnPlusB { a: -1, b: 3 } — the
/// `-n ['+'|'-'] <signless-integer>` form (CSS Syntax §7 L3041 +
/// L3066-3074 with leading `-` ident).
#[test]
fn pseudo_class_nth_child_negative_n() {
    let list = parse_a_selector(":nth-child(-n+3)").expect(":nth-child(-n+3) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    match pc
        .argument
        .as_ref()
        .expect("-n+3 must have AnPlusB argument")
    {
        PseudoClassArgument::AnPlusB(an, _) => assert_eq!(*an, AnPlusB { a: -1, b: 3 }),
        other => panic!("expected AnPlusB argument, got {:?}", other),
    }
}

/// `":foobar"` is rejected — unknown pseudo-class name per §3.7
/// invalid-selector handling.
#[test]
fn pseudo_class_unknown_rejected() {
    let result = parse_a_selector(":foobar");
    match result {
        Err(SelectorParseError::UnknownPseudoClass(name)) => assert_eq!(name, "foobar"),
        other => panic!("expected UnknownPseudoClass(\"foobar\"), got {:?}", other),
    }
}

/// `"::before"` parses as a modern pseudo-element with `legacy ==
/// false` (§14 L4476-4535 modern double-colon form). The pseudo-element
/// appears in `pseudo_compounds[0].pseudo_element`; `subclasses` is
/// empty.
#[test]
fn pseudo_element_simple() {
    let list = parse_a_selector("::before").expect("::before should parse");
    let compound = single_compound(&list);
    assert!(compound.type_selector.is_none());
    assert!(compound.subclasses.is_empty());
    assert_eq!(
        compound.pseudo_compounds.len(),
        1,
        "expected one pseudo-compound, got {}",
        compound.pseudo_compounds.len()
    );
    let pc: &PseudoCompoundSelector = &compound.pseudo_compounds[0];
    assert_eq!(pc.pseudo_element.name, "before");
    assert!(
        !pc.pseudo_element.legacy,
        "::before must have legacy == false"
    );
    assert!(pc.trailing_pseudo_classes.is_empty());
}

/// `":before"` parses as a legacy pseudo-element with `legacy == true`
/// (§14 L4476-4535 legacy single-colon form for backwards
/// compatibility).
#[test]
fn pseudo_element_legacy_single_colon() {
    let list = parse_a_selector(":before").expect(":before should parse");
    let compound = single_compound(&list);
    assert!(compound.type_selector.is_none());
    assert!(compound.subclasses.is_empty());
    assert_eq!(compound.pseudo_compounds.len(), 1);
    let pc: &PseudoCompoundSelector = &compound.pseudo_compounds[0];
    assert_eq!(pc.pseudo_element.name, "before");
    assert!(
        pc.pseudo_element.legacy,
        ":before must have legacy == true (single-colon form)"
    );
}

/// `":after"` must NOT be parsed as a pseudo-class. §14 mandates that
/// the four legacy pseudo-element names (`before`, `after`,
/// `first-line`, `first-letter`) be redirected to the pseudo-element
/// path when written with a single colon. This test verifies the
/// redirection by asserting that `subclasses` contains no `PseudoClass`
/// named "after" and that `pseudo_compounds[0].pseudo_element` carries
/// the legacy name.
#[test]
fn pseudo_element_legacy_rejected_as_pseudo_class() {
    let list = parse_a_selector(":after").expect(":after should parse as legacy pseudo-element");
    let compound = single_compound(&list);
    // The subclass list must NOT contain a PseudoClass named "after".
    for sc in &compound.subclasses {
        if let SubclassSelector::PseudoClass(pc) = sc {
            assert_ne!(
                pc.name, "after",
                ":after must not be parsed as a pseudo-class"
            );
        }
    }
    // It must instead appear as a legacy pseudo-element.
    assert_eq!(compound.pseudo_compounds.len(), 1);
    let pe = &compound.pseudo_compounds[0].pseudo_element;
    assert_eq!(pe.name, "after");
    assert!(pe.legacy, ":after must be legacy == true");
}
