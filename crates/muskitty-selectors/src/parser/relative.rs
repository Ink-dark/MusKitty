//! Relative-selector parsing (§4.5 `:has()` argument).
//!
//! A relative selector is a complex selector that begins with an
//! optional combinator (default descendant) and is anchored against
//! an implicit `:scope` element. Used by `:has()`.
//!
//! Per §3 L4811 note, `forgiving-selector-list` is reserved for
//! `:is()` and `:where()` only; `:has()` takes a non-forgiving
//! `relative-selector-list`.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §4.5 L1650-1804
//! (:has), §3 L1051-1102 (relative selectors), §3 L4811 note.

use crate::error::SelectorParseError;
use crate::parser::complex::parse_complex_selector;
use crate::types::{
    Combinator, ComplexSelector, ComplexSelectorUnit, CompoundSelector, PseudoClass, SelectorList,
    SubclassSelector,
};
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §4.5 L1700-1735: Parse a `<relative-selector>`.
///
/// A relative selector starts with an optional leading combinator
/// (`>`, `+`, `~`). If no combinator is present, descendant is the
/// default per §4.5 L1765-1769. The selector is anchored against an
/// implicit `:scope` element which is materialised as the leftmost
/// compound in the returned [`ComplexSelector`].
///
/// # Returned structure
///
/// The returned `ComplexSelector` has its leftmost compound (i.e.
/// `units[len-1]`) set to `:scope` with `combinator: None`; the
/// previously-leftmost unit (the rightmost compound of the relative
/// selector proper) carries the leading combinator (default
/// `Descendant`) in its `combinator` field, linking it to the
/// implicit `:scope`.
///
/// # Example
///
/// `> .a` parses to `units = [{ .a, Some(Child) }, { :scope, None }]`.
pub fn parse_relative_selector(
    stream: &mut TokenStream,
) -> Result<ComplexSelector, SelectorParseError> {
    stream.discard_whitespace();

    // Optional leading combinator (§4.5 L1700-1735). Default is
    // descendant per L1765-1769.
    let leading = match stream.next_token() {
        Token::Delim('>') => {
            stream.discard_token();
            Some(Combinator::Child)
        }
        Token::Delim('+') => {
            stream.discard_token();
            Some(Combinator::NextSibling)
        }
        Token::Delim('~') => {
            stream.discard_token();
            Some(Combinator::SubsequentSibling)
        }
        _ => None,
    };

    if leading.is_some() {
        stream.discard_whitespace();
    }

    // Parse the rest as a complex selector.
    let mut complex = parse_complex_selector(stream)?;

    // Prepend the implicit :scope. The previously-leftmost unit
    // (last in `units`) gets the leading combinator (default Descendant).
    let effective = leading.unwrap_or(Combinator::Descendant);
    complex
        .units
        .last_mut()
        .expect("complex selector has at least one unit")
        .combinator = Some(effective);
    complex.units.push(ComplexSelectorUnit {
        compound: CompoundSelector {
            type_selector: None,
            subclasses: vec![SubclassSelector::PseudoClass(PseudoClass {
                name: "scope".to_string(),
                argument: None,
            })],
            pseudo_compounds: Vec::new(),
        },
        combinator: None,
    });

    Ok(complex)
}

/// §4.5 L1700: Parse a `<relative-selector-list>`.
///
/// Comma-separated list of relative selectors. Non-forgiving: any
/// invalid selector fails the whole list (per §3 L4811 note —
/// `forgiving-selector-list` is reserved for `:is()` and `:where()`).
pub fn parse_relative_selector_list(
    stream: &mut TokenStream,
) -> Result<SelectorList, SelectorParseError> {
    let mut selectors = Vec::new();
    stream.discard_whitespace();
    selectors.push(parse_relative_selector(stream)?);

    loop {
        stream.discard_whitespace();
        match stream.next_token() {
            Token::Comma => {
                stream.discard_token();
                stream.discard_whitespace();
                if is_terminator(&stream.next_token()) {
                    return Err(SelectorParseError::InvalidSelector(
                        "trailing comma in relative selector list".into(),
                    ));
                }
                selectors.push(parse_relative_selector(stream)?);
            }
            _ => break,
        }
    }

    Ok(SelectorList(selectors))
}

fn is_terminator(token: &Token) -> bool {
    matches!(
        token,
        Token::Eof | Token::CloseParen | Token::CloseBrace | Token::CloseBracket
    )
}
