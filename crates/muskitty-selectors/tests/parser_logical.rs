//! SP-5 unit tests for §4 logical combinations: `:is()`, `:not()`,
//! `:where()`, `:has()`.
//!
//! Covers:
//! - §4.2 `:is(<forgiving-selector-list>)` — failures silently dropped
//! - §4.4 `:where(<forgiving-selector-list>)` — zero-specificity marker
//!   (specificity calc lands in SP-7; here we only verify parsing)
//! - §4.3 `:not(<complex-selector-list>)` — non-forgiving, complex
//!   selectors allowed (Level 4 feature)
//! - §4.5 `:has(<relative-selector-list>)` — non-forgiving, implicit
//!   `:scope` anchor with optional leading combinator (default
//!   descendant)

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    Combinator, ComplexSelectorUnit, CompoundSelector, PseudoClass, PseudoClassArgument,
    SelectorList, SubclassSelector,
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
        "expected no combinator on the rightmost unit"
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

/// Helper: extract the `SelectorList` argument of a parameterised
/// pseudo-class, panicking if the argument is not a `SelectorList`.
fn selector_list_argument(pc: &PseudoClass) -> &SelectorList {
    match &pc.argument {
        Some(PseudoClassArgument::SelectorList(list)) => list,
        other => panic!("expected SelectorList argument, got {:?}", other),
    }
}

/// §4.2: `:is(.a, .b)` parses as a pseudo-class named "is" whose
/// argument is a `SelectorList` of two complex selectors, one for
/// `.a` and one for `.b`.
#[test]
fn is_simple() {
    let list = parse_a_selector(":is(.a, .b)").expect(":is(.a, .b) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "is");
    let arg = selector_list_argument(pc);
    assert_eq!(arg.0.len(), 2, "expected 2 selectors in :is argument");
    // Each argument selector is a single compound containing one
    // ClassSelector subclass.
    for (i, cs) in arg.0.iter().enumerate() {
        assert_eq!(cs.units.len(), 1, "arg[{}] should have 1 unit", i);
        assert!(cs.units[0].combinator.is_none());
        let expected = if i == 0 { "a" } else { "b" };
        match &cs.units[0].compound.subclasses[0] {
            SubclassSelector::Class(c) => assert_eq!(c.class, expected),
            other => panic!("arg[{}] expected Class, got {:?}", i, other),
        }
    }
}

/// §4.2 + §3 L4765-4813: `:is(.a, %, .b)` parses with the invalid
/// middle selector silently dropped (forgiving). Result is a
/// `SelectorList` of 2 selectors (`.a` and `.b`).
#[test]
fn is_forgiving_drops_invalid() {
    let list = parse_a_selector(":is(.a, %, .b)").expect(":is(.a, %, .b) should parse forgivingly");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "is");
    let arg = selector_list_argument(pc);
    assert_eq!(
        arg.0.len(),
        2,
        "forgiving list should have 2 selectors after dropping the invalid one"
    );
    let classes: Vec<&str> = arg
        .0
        .iter()
        .map(|cs| match &cs.units[0].compound.subclasses[0] {
            SubclassSelector::Class(c) => c.class.as_str(),
            other => panic!("expected Class, got {:?}", other),
        })
        .collect();
    assert_eq!(classes, vec!["a", "b"]);
}

/// §4.4: `:where(.a)` parses as a pseudo-class named "where" whose
/// argument is a `SelectorList` of one complex selector. (Specificity
/// is computed in SP-7; here we only verify that the marker name is
/// preserved correctly.)
#[test]
fn where_zero_specificity_marker() {
    let list = parse_a_selector(":where(.a)").expect(":where(.a) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "where");
    let arg = selector_list_argument(pc);
    assert_eq!(arg.0.len(), 1);
    match &arg.0[0].units[0].compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "a"),
        other => panic!("expected Class, got {:?}", other),
    }
}

/// §4.3: `:not(.a)` parses as a pseudo-class named "not" whose argument
/// is a `SelectorList` of one complex selector.
#[test]
fn not_simple() {
    let list = parse_a_selector(":not(.a)").expect(":not(.a) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "not");
    let arg = selector_list_argument(pc);
    assert_eq!(arg.0.len(), 1);
    match &arg.0[0].units[0].compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "a"),
        other => panic!("expected Class, got {:?}", other),
    }
}

/// §4.3 Level 4 feature: `:not(.a > .b)` accepts a complex selector as
/// the argument (Level 3 only allowed simple selectors). The argument
/// `SelectorList` contains one complex selector with 2 compound units
/// joined by a Child combinator. Per the rightmost-first storage
/// convention, `units[0]` is the subject `.b` carrying the Child
/// combinator; `units[1]` is the leftmost `.a` with `combinator ==
/// None`.
#[test]
fn not_complex_selector_arg() {
    let list = parse_a_selector(":not(.a > .b)").expect(":not(.a > .b) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "not");
    let arg = selector_list_argument(pc);
    assert_eq!(arg.0.len(), 1);
    let cs = &arg.0[0];
    assert_eq!(
        cs.units.len(),
        2,
        ":not arg should be a 2-unit complex selector"
    );
    // units[0] = .b (subject, rightmost in source); carries the Child
    // combinator linking it to .a on its left.
    assert_eq!(cs.units[0].compound.subclasses.len(), 1);
    match &cs.units[0].compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "b"),
        other => panic!("expected .b at subject, got {:?}", other),
    }
    assert_eq!(cs.units[0].combinator, Some(Combinator::Child));
    // units[1] = .a (leftmost in source); no further-left unit, so
    // combinator is None.
    match &cs.units[1].compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "a"),
        other => panic!("expected .a at left, got {:?}", other),
    }
    assert_eq!(cs.units[1].combinator, None);
}

/// §4.3: `:not()` is non-forgiving — an invalid argument selector
/// causes the whole pseudo-class to fail. `:not(.a, %)` must error.
#[test]
fn not_non_forgiving_invalid_fails() {
    let result = parse_a_selector(":not(.a, %)");
    assert!(
        result.is_err(),
        ":not should be non-forgiving and reject invalid arguments"
    );
}

/// §4.5: `:has(.a)` parses with an implicit descendant combinator
/// linking the relative selector to the implicit `:scope` anchor.
/// The returned complex selector should have 2 units: `.a` (subject)
/// with combinator Descendant, and `:scope` (leftmost) with combinator
/// None.
#[test]
fn has_descendant_default() {
    let list = parse_a_selector(":has(.a)").expect(":has(.a) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "has");
    let arg = selector_list_argument(pc);
    assert_eq!(arg.0.len(), 1);
    let cs = &arg.0[0];
    assert_eq!(
        cs.units.len(),
        2,
        ":has arg should have implicit :scope prepended"
    );
    // units[0] = .a (subject), combinator = Descendant (default).
    assert_eq!(cs.units[0].combinator, Some(Combinator::Descendant));
    match &cs.units[0].compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "a"),
        other => panic!("expected .a subject, got {:?}", other),
    }
    // units[1] = :scope (implicit), combinator = None.
    assert_eq!(cs.units[1].combinator, None);
    match &cs.units[1].compound.subclasses[0] {
        SubclassSelector::PseudoClass(scope_pc) => assert_eq!(scope_pc.name, "scope"),
        other => panic!("expected :scope at leftmost, got {:?}", other),
    }
}

/// §4.5: `:has(> .a)` parses with an explicit Child combinator linking
/// the relative selector to `:scope`.
#[test]
fn has_child_explicit() {
    let list = parse_a_selector(":has(> .a)").expect(":has(> .a) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "has");
    let arg = selector_list_argument(pc);
    let cs = &arg.0[0];
    assert_eq!(cs.units.len(), 2);
    assert_eq!(cs.units[0].combinator, Some(Combinator::Child));
    match &cs.units[0].compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "a"),
        other => panic!("expected .a subject, got {:?}", other),
    }
    assert_eq!(cs.units[1].combinator, None);
    match &cs.units[1].compound.subclasses[0] {
        SubclassSelector::PseudoClass(scope_pc) => assert_eq!(scope_pc.name, "scope"),
        other => panic!("expected :scope, got {:?}", other),
    }
}

/// §4.5: `:has(+ .a)` parses with a NextSibling combinator.
#[test]
fn has_next_sibling() {
    let list = parse_a_selector(":has(+ .a)").expect(":has(+ .a) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    let arg = selector_list_argument(pc);
    let cs = &arg.0[0];
    assert_eq!(cs.units.len(), 2);
    assert_eq!(cs.units[0].combinator, Some(Combinator::NextSibling));
}

/// §4.5: `:has(~ .a)` parses with a SubsequentSibling combinator.
#[test]
fn has_subsequent_sibling() {
    let list = parse_a_selector(":has(~ .a)").expect(":has(~ .a) should parse");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    let arg = selector_list_argument(pc);
    let cs = &arg.0[0];
    assert_eq!(cs.units.len(), 2);
    assert_eq!(cs.units[0].combinator, Some(Combinator::SubsequentSibling));
}
