//! Compound-selector parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <compound-selector> = [ <type-selector>? <subclass-selector>* ]!
//!                        <pseudo-compound-selector>*
//! ```
//!
//! The `!` indicates the inner bracketed group is required to be
//! non-empty *unless* a pseudo-compound selector (pseudo-element +
//! trailing pseudo-classes) is present: `::before` alone is a valid
//! compound selector.
//!
//! SP-4 scope: subclass-selector supports `id`, `class`, `attribute`,
//! and `pseudo-class`; pseudo-compound selectors (modern `::name` and
//! legacy `:before`/`:after`/`:first-line`/`:first-letter`) are parsed
//! here and any trailing pseudo-classes attach to the most recent
//! pseudo-compound per §3 L762-787.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4671 + L4684 +
//! L762-787, §13 L3792-4359 (tree-structural pseudo-classes), §14
//! (pseudo-elements).

use crate::error::SelectorParseError;
use crate::parser::simple::{
    parse_attribute_selector, parse_class_selector, parse_id_selector,
    parse_pseudo_class_or_legacy, parse_pseudo_element, parse_type_selector, PseudoClassOrLegacy,
};
use crate::types::{CompoundSelector, PseudoCompoundSelector, SubclassSelector};
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §3 L4671 + L762-787: Parse a `<compound-selector>`.
///
/// Returns `Ok(CompoundSelector)` containing at least one simple
/// selector (type selector, subclass selector, or pseudo-compound
/// selector). Returns `Err(InvalidSelector)` if the input at the
/// current position does not start a compound selector.
///
/// # Phase structure
///
/// 1. **Subclass phase**: parse `id` / `class` / `attribute` /
///    `pseudo-class` until none of them match. A legacy pseudo-element
///    (`:before` / `:after` / `:first-line` / `:first-letter`) detected
///    via [`parse_pseudo_class_or_legacy`] terminates this phase and
///    starts the pseudo-compound phase with that pseudo-element as the
///    first entry.
/// 2. **Pseudo-compound phase**: parse zero or more `::name` modern
///    pseudo-elements, each optionally followed by trailing
///    pseudo-classes that attach to the most recent pseudo-compound per
///    §3 L762-787.
pub fn parse_compound_selector(
    stream: &mut TokenStream,
) -> Result<CompoundSelector, SelectorParseError> {
    // §3 L750-752: type selector (or universal selector) must come
    // first if present.
    let type_selector = parse_type_selector(stream)?;
    let mut compound = CompoundSelector {
        type_selector,
        ..CompoundSelector::default()
    };

    // ── Subclass phase ───────────────────────────────────────────
    // §3 L753-760: subclass selectors may appear in any order after
    // the type selector. §3 L4684: pseudo-class is a subclass-selector.
    // §14: legacy `:before` etc. redirect to the pseudo-compound phase.
    loop {
        if let Some(id) = parse_id_selector(stream)? {
            compound.subclasses.push(SubclassSelector::Id(id));
            continue;
        }
        if let Some(class) = parse_class_selector(stream)? {
            compound.subclasses.push(SubclassSelector::Class(class));
            continue;
        }
        if let Some(attr) = parse_attribute_selector(stream)? {
            compound.subclasses.push(SubclassSelector::Attribute(attr));
            continue;
        }
        match parse_pseudo_class_or_legacy(stream)? {
            PseudoClassOrLegacy::None => break,
            PseudoClassOrLegacy::Class(pc) => {
                compound.subclasses.push(SubclassSelector::PseudoClass(pc));
                continue;
            }
            PseudoClassOrLegacy::LegacyElement(pe) => {
                // §14 legacy single-colon pseudo-element. Subclasses
                // cannot follow a pseudo-element; switch to the
                // pseudo-compound phase.
                compound.pseudo_compounds.push(PseudoCompoundSelector {
                    pseudo_element: pe,
                    trailing_pseudo_classes: Vec::new(),
                });
                break;
            }
        }
    }

    // ── Pseudo-compound phase ───────────────────────────────────
    // §3 L762-787: zero or more pseudo-compound selectors may follow.
    // Each begins with a pseudo-element (modern `::name` or legacy
    // `:before`); any pseudo-classes appearing after a pseudo-element
    // in source order attach to that pseudo-element's
    // `trailing_pseudo_classes`.
    loop {
        // Try modern `::name` first. parse_pseudo_element leaves the
        // stream unmodified when it returns None.
        if let Some(pe) = parse_pseudo_element(stream)? {
            compound.pseudo_compounds.push(PseudoCompoundSelector {
                pseudo_element: pe,
                trailing_pseudo_classes: Vec::new(),
            });
            continue;
        }
        // Then try pseudo-class or legacy pseudo-element. A regular
        // pseudo-class here attaches to the most recent pseudo-compound
        // (e.g. `::before:hover`); a legacy pseudo-element starts a
        // new pseudo-compound entry.
        match parse_pseudo_class_or_legacy(stream)? {
            PseudoClassOrLegacy::None => break,
            PseudoClassOrLegacy::Class(pc) => {
                if let Some(last) = compound.pseudo_compounds.last_mut() {
                    last.trailing_pseudo_classes.push(pc);
                } else {
                    // Unreachable: the subclass phase only breaks out
                    // via `None` (which means parse_pseudo_class_or_legacy
                    // will return None here too) or `LegacyElement`
                    // (which already pushed one). The only other entry
                    // path is a successful `parse_pseudo_element`
                    // above, which also pushes one. Defensively treat
                    // the stray pseudo-class as a subclass.
                    compound.subclasses.push(SubclassSelector::PseudoClass(pc));
                }
            }
            PseudoClassOrLegacy::LegacyElement(pe) => {
                compound.pseudo_compounds.push(PseudoCompoundSelector {
                    pseudo_element: pe,
                    trailing_pseudo_classes: Vec::new(),
                });
            }
        }
    }

    // The `!` in the grammar requires the compound selector to be
    // non-empty: a type selector, a subclass selector, or a
    // pseudo-compound selector (§3 L762-787 allows a compound selector
    // to consist solely of a pseudo-element, e.g. `::before`).
    if compound.type_selector.is_none()
        && compound.subclasses.is_empty()
        && compound.pseudo_compounds.is_empty()
    {
        let next = stream.next_token();
        let msg = match next {
            Token::Eof => "expected a compound selector, got end of input".into(),
            _ => format!("expected a compound selector, got {:?}", next),
        };
        return Err(SelectorParseError::InvalidSelector(msg));
    }

    Ok(compound)
}
