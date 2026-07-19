//! SP-1 unit tests for §3 selector data model.
//!
//! Verifies the basic data structures can be constructed and that
//! `Default` impls and `PartialEq` derivations behave as expected. No
//! parsing is exercised in SP-1 (parser entry points return
//! `NotImplemented`).

use muskitty_css::tokenizer::Token;
use muskitty_selectors::types::{
    AnPlusB, AttrMatcher, Combinator, CompoundSelector, PseudoClassArgument, SelectorList,
    TypeSelectorName,
};

/// `SelectorList::default()` is an empty selector list (§3 L856-857).
#[test]
fn selector_list_default_empty() {
    let list = SelectorList::default();
    assert!(list.0.is_empty());
}

/// `CompoundSelector::default()` has no type selector and empty
/// subclass / pseudo_compound lists (§3 L746-760).
#[test]
fn compound_selector_default_no_type() {
    let compound = CompoundSelector::default();
    assert!(compound.type_selector.is_none());
    assert!(compound.subclasses.is_empty());
    assert!(compound.pseudo_compounds.is_empty());
}

/// The four `Combinator` variants (§3 L798-805) are pairwise unequal.
#[test]
fn combinator_equality() {
    use Combinator::{Child, Descendant, NextSibling, SubsequentSibling};
    assert_ne!(Descendant, Child);
    assert_ne!(Descendant, NextSibling);
    assert_ne!(Descendant, SubsequentSibling);
    assert_ne!(Child, NextSibling);
    assert_ne!(Child, SubsequentSibling);
    assert_ne!(NextSibling, SubsequentSibling);
    // Equality is reflexive for each variant.
    assert_eq!(Descendant, Descendant);
}

/// All six `AttrMatcher` variants (§6.1 L2023-2135 + §6.2 L2137-2162)
/// can be constructed.
#[test]
fn attr_matcher_variants() {
    let matchers = [
        AttrMatcher::Exact,
        AttrMatcher::Includes,
        AttrMatcher::DashMatch,
        AttrMatcher::Prefix,
        AttrMatcher::Suffix,
        AttrMatcher::Substring,
    ];
    assert_eq!(matchers.len(), 6);
    // Spot-check inequality to exercise PartialEq.
    assert_ne!(AttrMatcher::Exact, AttrMatcher::Includes);
    assert_ne!(AttrMatcher::Prefix, AttrMatcher::Suffix);
}

/// All three `PseudoClassArgument` variants can be constructed.
#[test]
fn pseudo_class_argument_variants() {
    let an_plus_b = PseudoClassArgument::AnPlusB(AnPlusB::default(), None);
    let selector_list = PseudoClassArgument::SelectorList(SelectorList::default());
    let raw = PseudoClassArgument::Raw(vec![Token::Ident("en".to_string())]);
    assert!(matches!(an_plus_b, PseudoClassArgument::AnPlusB(_, None)));
    assert!(matches!(
        selector_list,
        PseudoClassArgument::SelectorList(_)
    ));
    assert!(matches!(raw, PseudoClassArgument::Raw(_)));
}

/// `TypeSelectorName::Universal` and `TypeSelectorName::Name("div")`
/// are not equal (§5 L1808-1866).
#[test]
fn type_selector_universal_vs_named() {
    let universal = TypeSelectorName::Universal;
    let named = TypeSelectorName::Name("div".to_string());
    assert_ne!(universal, named);
    assert_eq!(universal, TypeSelectorName::Universal);
    assert_eq!(named, TypeSelectorName::Name("div".to_string()));
}
