//! SP-2 unit tests for §5/§6.5/§6.6 type/class/id selector parsing.

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    Combinator, ComplexSelectorUnit, CompoundSelector, NsPrefix, NsPrefixKind, SelectorList,
    SubclassSelector, TypeSelector, TypeSelectorName,
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
        "expected no combinator in SP-2 single-compound selectors"
    );
    &unit.compound
}

/// `"div"` parses as a type selector with no ns prefix and name
/// `Name("div")` (§5 L1808-1824).
#[test]
fn type_selector_simple_div() {
    let list = parse_a_selector("div").expect("div should parse");
    let compound = single_compound(&list);
    let ts = compound
        .type_selector
        .as_ref()
        .expect("expected a type selector");
    assert!(ts.ns_prefix.is_none());
    assert_eq!(ts.name, TypeSelectorName::Name("div".to_string()));
    assert!(compound.subclasses.is_empty());
}

/// `"*"` parses as the universal selector with no ns prefix (§5
/// L1825-1866).
#[test]
fn type_selector_universal_star() {
    let list = parse_a_selector("*").expect("* should parse");
    let compound = single_compound(&list);
    let ts = compound
        .type_selector
        .as_ref()
        .expect("expected a type selector");
    assert!(ts.ns_prefix.is_none());
    assert_eq!(ts.name, TypeSelectorName::Universal);
}

/// `"svg|rect"` parses as a type selector with ns_prefix Named("svg")
/// and name Name("rect") (§5 L1867-1872 + L1877-1879).
#[test]
fn type_selector_ns_named() {
    let list = parse_a_selector("svg|rect").expect("svg|rect should parse");
    let compound = single_compound(&list);
    let ts = compound
        .type_selector
        .as_ref()
        .expect("expected a type selector");
    let ns = ts.ns_prefix.as_ref().expect("expected ns prefix");
    assert_eq!(ns.prefix, NsPrefixKind::Named("svg".to_string()));
    assert_eq!(ts.name, TypeSelectorName::Name("rect".to_string()));
}

/// `"*|div"` parses as a type selector with ns_prefix Any and name
/// Name("div") (§5 L1881-1884).
#[test]
fn type_selector_ns_any() {
    let list = parse_a_selector("*|div").expect("*|div should parse");
    let compound = single_compound(&list);
    let ts = compound
        .type_selector
        .as_ref()
        .expect("expected a type selector");
    let ns = ts.ns_prefix.as_ref().expect("expected ns prefix");
    assert_eq!(ns.prefix, NsPrefixKind::Any);
    assert_eq!(ts.name, TypeSelectorName::Name("div".to_string()));
}

/// `"|div"` parses as a type selector with ns_prefix None (empty
/// prefix) and name Name("div") (§5 L1886-1888).
#[test]
fn type_selector_ns_none() {
    let list = parse_a_selector("|div").expect("|div should parse");
    let compound = single_compound(&list);
    let ts = compound
        .type_selector
        .as_ref()
        .expect("expected a type selector");
    let ns = ts.ns_prefix.as_ref().expect("expected ns prefix");
    assert_eq!(ns.prefix, NsPrefixKind::None);
    assert_eq!(ts.name, TypeSelectorName::Name("div".to_string()));
}

/// `".foo"` parses as a class selector with no type selector (§6.5
/// L2376-2462 + §3 L4689).
#[test]
fn class_selector_simple() {
    let list = parse_a_selector(".foo").expect(".foo should parse");
    let compound = single_compound(&list);
    assert!(compound.type_selector.is_none());
    assert_eq!(compound.subclasses.len(), 1);
    match &compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "foo"),
        other => panic!("expected Class, got {:?}", other),
    }
}

/// `"div.foo"` parses as a compound selector with a type selector
/// followed by a class selector (§3 L4671).
#[test]
fn class_selector_after_type() {
    let list = parse_a_selector("div.foo").expect("div.foo should parse");
    let compound = single_compound(&list);
    let ts = compound
        .type_selector
        .as_ref()
        .expect("expected a type selector");
    assert_eq!(ts.name, TypeSelectorName::Name("div".to_string()));
    assert_eq!(compound.subclasses.len(), 1);
    match &compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, "foo"),
        other => panic!("expected Class, got {:?}", other),
    }
}

/// `"#main"` parses as an id selector (§6.6 L2463-2533 + §3 L4687 +
/// L4729 — hash-token's value must be an identifier).
#[test]
fn id_selector_simple() {
    let list = parse_a_selector("#main").expect("#main should parse");
    let compound = single_compound(&list);
    assert!(compound.type_selector.is_none());
    assert_eq!(compound.subclasses.len(), 1);
    match &compound.subclasses[0] {
        SubclassSelector::Id(id) => assert_eq!(id.id, "main"),
        other => panic!("expected Id, got {:?}", other),
    }
}

/// `"#123abc"` is rejected: the hash-token's value is not an
/// identifier (HashType::Unrestricted), so it does not form a valid
/// `<id-selector>` per §3 L4729.
#[test]
fn id_selector_hex_digits_rejected() {
    let result = parse_a_selector("#123abc");
    assert!(
        result.is_err(),
        "#123abc should not parse as an id selector"
    );
}

/// `"a, b"` parses as a selector list with two complex selectors (§3
/// L4651-4653).
#[test]
fn selector_list_two_comma_separated() {
    let list = parse_a_selector("a, b").expect("a, b should parse");
    assert_eq!(list.0.len(), 2);
    let cs0 = &list.0[0];
    let cs1 = &list.0[1];
    assert_eq!(cs0.units.len(), 1);
    assert_eq!(cs1.units.len(), 1);
    assert!(cs0.units[0].combinator.is_none());
    assert!(cs1.units[0].combinator.is_none());
    // Sanity: Combinator enum is referenced for SP-6 forward-compat.
    let _ = Combinator::Descendant;
    let _ = NsPrefix {
        prefix: NsPrefixKind::Named("x".into()),
    };
    let _ = TypeSelector {
        ns_prefix: None,
        name: TypeSelectorName::Universal,
    };
}
