//! Compound-selector parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <compound-selector> = [ <type-selector>? <subclass-selector>* ]!
//! ```
//!
//! The `!` indicates the production is required to be non-empty: a
//! compound selector must contain at least one simple selector.
//!
//! SP-3 scope: subclass-selector supports `id`, `class`, and
//! `attribute`. `pseudo-class` (SP-4) is added in the next batch;
//! pseudo-compound selectors (pseudo-element + trailing pseudo-classes)
//! are added in SP-4 / SP-6.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4671 + L4684.

use crate::error::SelectorParseError;
use crate::parser::simple::{
    parse_attribute_selector, parse_class_selector, parse_id_selector, parse_type_selector,
};
use crate::types::{CompoundSelector, SubclassSelector};
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §3 L4671: Parse a `<compound-selector>`.
///
/// Returns `Ok(CompoundSelector)` containing at least one simple
/// selector (type selector or subclass selector). Returns
/// `Err(InvalidSelector)` if the input at the current position does
/// not start a compound selector (i.e. none of type/class/id/attribute
/// matches and there is no pseudo-class either — the latter is an
/// SP-4 extension and currently falls through to "not a compound
/// selector").
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

    // Subclass selectors may appear in any order after the type
    // selector (§3 L753-760). Loop until we stop recognising a
    // subclass starter.
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
        // SP-4: parse_pseudo_class / parse_pseudo_element here.
        break;
    }

    // The `!` in the grammar requires the compound selector to be
    // non-empty: either a type selector or at least one subclass
    // selector.
    if compound.type_selector.is_none() && compound.subclasses.is_empty() {
        // Identify the offending token for a useful error message.
        let next = stream.next_token();
        let msg = match next {
            Token::Eof => "expected a compound selector, got end of input".into(),
            _ => format!("expected a compound selector, got {:?}", next),
        };
        return Err(SelectorParseError::InvalidSelector(msg));
    }

    Ok(compound)
}
