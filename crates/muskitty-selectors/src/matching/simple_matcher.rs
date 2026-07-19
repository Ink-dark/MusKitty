//! Simple-selector matching: type / universal / class / id / attribute.
//!
//! Implements the matching rules for §5 (elemental), §6.5 (class),
//! §6.6 (id), §6 (attribute) selectors against an [`Element`].
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L858-873,
//! §5 L1808-1995, §6 L1996-2533.

use crate::matching::Element;
use crate::types::{
    AttrMatcher, AttrValue, AttributeSelector, ClassSelector, CompoundSelector, IdSelector,
    NsPrefixKind, SubclassSelector, TypeSelector, TypeSelectorName,
};

/// §3 L746-760: match a compound selector against an element. Returns
/// `true` if every component (type / subclasses / pseudo-compounds)
/// matches.
///
/// Pseudo-class / pseudo-element matching is delegated to
/// [`crate::matching::pseudo_matcher`]; this function handles type
/// and subclass matching and delegates pseudo matching.
pub fn matches_compound<E: Element>(compound: &CompoundSelector, element: &E) -> bool {
    // Type selector (or universal) — must match first.
    if let Some(ref ts) = compound.type_selector {
        if !matches_type_selector(ts, element) {
            return false;
        }
    }
    // Subclass selectors (id / class / attribute / pseudo-class).
    for sub in &compound.subclasses {
        if !matches_subclass(sub, element) {
            return false;
        }
    }
    // Pseudo-compounds (pseudo-element + trailing pseudo-classes).
    // SP-8: pseudo-elements do not exist in the element tree, so a
    // compound with any pseudo-compound never matches a real element.
    // (Pseudo-element matching would require pseudo-element tree
    // abstraction, out of scope for SP-8.)
    if !compound.pseudo_compounds.is_empty() {
        return false;
    }
    true
}

/// §5 L1808-1866: match a type selector (or universal).
pub fn matches_type_selector<E: Element>(ts: &TypeSelector, element: &E) -> bool {
    match &ts.name {
        TypeSelectorName::Universal => {
            // §5 L1825-1866: `*` matches any element. Namespace
            // prefix matters: `*|*` matches any namespace, `ns|*`
            // matches only `ns`, `|*` matches no namespace.
            match ts.ns_prefix.as_ref().map(|p| &p.prefix) {
                None | Some(NsPrefixKind::Any) => true,
                Some(NsPrefixKind::Named(_)) => {
                    // For HTML trees, named-namespace universal
                    // selectors are uncommon; we conservatively
                    // require namespace_uri to be present (non-HTML).
                    // Strict namespace matching is out of scope for
                    // SP-8; we accept any non-None namespace here.
                    element.namespace_uri().is_some()
                }
                Some(NsPrefixKind::None) => element.namespace_uri().is_none(),
            }
        }
        TypeSelectorName::Name(name) => {
            // HTML is case-insensitive for tag names.
            if !name.eq_ignore_ascii_case(&element.local_name()) {
                return false;
            }
            // Namespace prefix matching — same conservative approach
            // as universal. None/Any accept any namespace.
            match ts.ns_prefix.as_ref().map(|p| &p.prefix) {
                None | Some(NsPrefixKind::Any) => true,
                Some(NsPrefixKind::Named(_)) => element.namespace_uri().is_some(),
                Some(NsPrefixKind::None) => element.namespace_uri().is_none(),
            }
        }
    }
}

/// §3 L4674-4685: match a subclass selector.
fn matches_subclass<E: Element>(sub: &SubclassSelector, element: &E) -> bool {
    match sub {
        SubclassSelector::Id(id) => matches_id(id, element),
        SubclassSelector::Class(cls) => matches_class(cls, element),
        SubclassSelector::Attribute(attr) => matches_attribute(attr, element),
        SubclassSelector::PseudoClass(pc) => {
            crate::matching::pseudo_matcher::matches_pseudo_class(pc, element)
        }
    }
}

/// §6.6 L2463-2533: `#id` matches when element's `id` attribute
/// equals the selector's id.
fn matches_id<E: Element>(sel: &IdSelector, element: &E) -> bool {
    element.id().as_deref() == Some(sel.id.as_str())
}

/// §6.5 L2376-2462: `.class` matches when element's class list
/// contains the selector's class name.
fn matches_class<E: Element>(sel: &ClassSelector, element: &E) -> bool {
    element.classes().iter().any(|c| c == &sel.class)
}

/// §6 L1996-2533: attribute selector matching.
pub fn matches_attribute<E: Element>(sel: &AttributeSelector, element: &E) -> bool {
    let value = match element.get_attribute(&sel.name.local_name) {
        Some(v) => v,
        None => return false,
    };
    match &sel.matcher {
        None => true, // presence selector `[attr]`
        Some(matcher) => match &sel.value {
            None => false, // invalid: matcher without value
            Some(attr_val) => {
                let target = attr_value_str(attr_val);
                match matcher {
                    AttrMatcher::Exact => value == target,
                    AttrMatcher::Includes => {
                        // Whitespace-separated list contains target.
                        value.split_ascii_whitespace().any(|tok| tok == target)
                    }
                    AttrMatcher::DashMatch => {
                        // §6.1 L2055-2080: exact or prefix followed
                        // by hyphen.
                        value == target || value.starts_with(&format!("{target}-"))
                    }
                    AttrMatcher::Prefix => value.starts_with(target),
                    AttrMatcher::Suffix => value.ends_with(target),
                    AttrMatcher::Substring => value.contains(target),
                }
            }
        },
    }
}

fn attr_value_str(v: &AttrValue) -> &str {
    match v {
        AttrValue::String(s) => s,
        AttrValue::Ident(s) => s,
    }
}
