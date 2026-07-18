//! SP-3 unit tests for §6 attribute selector parsing.
//!
//! Covers §6.1 (presence + value matchers), §6.2 (substring matchers),
//! §6.3 (i/s case-sensitivity modifier), and §6.4 (namespace prefix).

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    AttrMatcher, AttrModifier, AttrValue, ComplexSelectorUnit, CompoundSelector, NsPrefix,
    NsPrefixKind, SelectorList, SubclassSelector, WqName,
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
        "expected no combinator in SP-3 single-compound selectors"
    );
    &unit.compound
}

/// Helper: extract the single Attribute subclass from a compound.
fn single_attr(compound: &CompoundSelector) -> &muskitty_selectors::types::AttributeSelector {
    assert_eq!(
        compound.subclasses.len(),
        1,
        "expected 1 subclass, got {}",
        compound.subclasses.len()
    );
    match &compound.subclasses[0] {
        SubclassSelector::Attribute(a) => a,
        other => panic!("expected Attribute, got {:?}", other),
    }
}

/// `"[disabled]"` parses as a presence selector: matcher None, value
/// None, modifier None (§6.1 L2023-2035).
#[test]
fn attr_presence() {
    let list = parse_a_selector("[disabled]").expect("[disabled] should parse");
    let compound = single_compound(&list);
    assert!(compound.type_selector.is_none());
    let attr = single_attr(compound);
    assert_eq!(attr.name.local_name, "disabled");
    assert!(attr.name.ns_prefix.is_none());
    assert!(attr.matcher.is_none());
    assert!(attr.value.is_none());
    assert!(attr.modifier.is_none());
}

/// `"[lang=\"en\"]"` parses as Exact matcher with a string value
/// (§6.1 L2037-2054).
#[test]
fn attr_exact_string_value() {
    let list = parse_a_selector("[lang=\"en\"]").expect("[lang=\"en\"] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.name.local_name, "lang");
    assert_eq!(attr.matcher, Some(AttrMatcher::Exact));
    assert_eq!(attr.value, Some(AttrValue::String("en".into())));
    assert!(attr.modifier.is_none());
}

/// `"[lang=en]"` parses as Exact matcher with an unquoted ident value
/// (§6.1 L2061).
#[test]
fn attr_exact_ident_value() {
    let list = parse_a_selector("[lang=en]").expect("[lang=en] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.name.local_name, "lang");
    assert_eq!(attr.matcher, Some(AttrMatcher::Exact));
    assert_eq!(attr.value, Some(AttrValue::Ident("en".into())));
    assert!(attr.modifier.is_none());
}

/// `"[class~=\"foo\"]"` parses as Includes matcher (§6.1 L2038-2047).
#[test]
fn attr_includes() {
    let list = parse_a_selector("[class~=\"foo\"]").expect("[class~=\"foo\"] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.matcher, Some(AttrMatcher::Includes));
    assert_eq!(attr.value, Some(AttrValue::String("foo".into())));
}

/// `"[lang|=en]"` parses as DashMatch — this exercises the
/// disambiguation between `|` (ns-prefix separator) and `|=`
/// (dash-match attr-matcher) per §3 L4691-4694.
#[test]
fn attr_dash_match() {
    let list = parse_a_selector("[lang|=en]").expect("[lang|=en] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.name.local_name, "lang");
    assert!(attr.name.ns_prefix.is_none(), "|= must not be ns-prefix");
    assert_eq!(attr.matcher, Some(AttrMatcher::DashMatch));
    assert_eq!(attr.value, Some(AttrValue::Ident("en".into())));
}

/// `"[href^=\"https\"]"` parses as Prefix matcher (§6.2 L2143-2148).
#[test]
fn attr_prefix() {
    let list = parse_a_selector("[href^=\"https\"]").expect("[href^=\"https\"] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.matcher, Some(AttrMatcher::Prefix));
    assert_eq!(attr.value, Some(AttrValue::String("https".into())));
}

/// `"[href$=\".pdf\"]"` parses as Suffix matcher (§6.2 L2150-2155).
#[test]
fn attr_suffix() {
    let list = parse_a_selector("[href$=\".pdf\"]").expect("[href$=\".pdf\"] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.matcher, Some(AttrMatcher::Suffix));
    assert_eq!(attr.value, Some(AttrValue::String(".pdf".into())));
}

/// `"[class*=\"btn\"]"` parses as Substring matcher (§6.2 L2157-2162).
#[test]
fn attr_substring() {
    let list = parse_a_selector("[class*=\"btn\"]").expect("[class*=\"btn\"] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.matcher, Some(AttrMatcher::Substring));
    assert_eq!(attr.value, Some(AttrValue::String("btn".into())));
}

/// `"[attr=value i]"` parses with CaseInsensitive modifier (§6.3
/// L2204-2211).
#[test]
fn attr_modifier_i() {
    let list = parse_a_selector("[attr=value i]").expect("[attr=value i] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.matcher, Some(AttrMatcher::Exact));
    assert_eq!(attr.value, Some(AttrValue::Ident("value".into())));
    assert_eq!(attr.modifier, Some(AttrModifier::CaseInsensitive));
}

/// `"[attr=value s]"` parses with CaseSensitive modifier (§6.3
/// L2221-2225).
#[test]
fn attr_modifier_s() {
    let list = parse_a_selector("[attr=value s]").expect("[attr=value s] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(attr.matcher, Some(AttrMatcher::Exact));
    assert_eq!(attr.value, Some(AttrValue::Ident("value".into())));
    assert_eq!(attr.modifier, Some(AttrModifier::CaseSensitive));
}

/// `"[svg|href]"` parses as a presence selector with a named
/// namespace prefix on the attribute name (§6.4 L2266-2298).
#[test]
fn attr_with_ns_prefix() {
    let list = parse_a_selector("[svg|href]").expect("[svg|href] should parse");
    let compound = single_compound(&list);
    let attr = single_attr(compound);
    assert_eq!(
        attr.name,
        WqName {
            ns_prefix: Some(NsPrefix {
                prefix: NsPrefixKind::Named("svg".into()),
            }),
            local_name: "href".into(),
        }
    );
    assert!(attr.matcher.is_none());
    assert!(attr.value.is_none());
    assert!(attr.modifier.is_none());
}
