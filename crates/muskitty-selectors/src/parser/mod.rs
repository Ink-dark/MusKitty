//! Selectors Level 4 parser entry points.
//!
//! Implements the §18 "API Hooks" Parse A Selector / Parse A Relative
//! Selector algorithms by delegating to the submodule parsers.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §18 L4816-5026
//! (API hooks), §3 L4640-4815 (grammar).

pub mod an_plus_b;
pub mod complex;
pub mod compound;
pub mod list;
pub mod relative;
pub mod simple;

use crate::error::SelectorParseError;
use crate::types::SelectorList;
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §18 L4828-4849: Parse A Selector.
///
/// Tokenises `source` with the muskitty-css tokenizer and parses the
/// resulting token stream as a `<complex-selector-list>` per §3
/// L4651-4653. Returns the parsed [`SelectorList`] on success, or a
/// [`SelectorParseError`] describing the failure mode.
///
/// Empty input or whitespace-only input returns
/// [`SelectorParseError::EmptySelector`] per §3 L1317-1347. Trailing
/// tokens after the selector list (other than whitespace) produce an
/// `InvalidSelector` error: a selector source must consume the entire
/// input.
pub fn parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    let tokens = muskitty_css::tokenize(source);
    let mut stream = TokenStream::new(tokens);

    // §3 L1317-1347: empty input or whitespace-only input is not a
    // valid selector list. Distinguish this case from a structurally
    // invalid selector by returning EmptySelector.
    stream.discard_whitespace();
    if matches!(stream.next_token(), Token::Eof) {
        return Err(SelectorParseError::EmptySelector);
    }

    let list = list::parse_selector_list(&mut stream)?;
    // Reject trailing garbage (whitespace is fine).
    stream.discard_whitespace();
    if !stream.is_empty() {
        return Err(SelectorParseError::InvalidSelector(format!(
            "trailing tokens after selector: {:?}",
            stream.next_token()
        )));
    }
    Ok(list)
}

/// §18 L4853-4875: Parse A Relative Selector.
///
/// Like [`parse_a_selector`] but the source is interpreted as a
/// relative selector (relative to an implicit `:scope` element, per
/// §3 L1051-1102). Used by `:has()` arguments.
///
/// Delegates to [`crate::parser::relative::parse_relative_selector_list`]
/// after tokenisation.
pub fn parse_a_relative_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    let tokens = muskitty_css::tokenize(source);
    let mut stream = TokenStream::new(tokens);

    stream.discard_whitespace();
    if matches!(stream.next_token(), Token::Eof) {
        return Err(SelectorParseError::EmptySelector);
    }

    let list = relative::parse_relative_selector_list(&mut stream)?;
    stream.discard_whitespace();
    if !stream.is_empty() {
        return Err(SelectorParseError::InvalidSelector(format!(
            "trailing tokens after relative selector: {:?}",
            stream.next_token()
        )));
    }
    Ok(list)
}
