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
pub mod simple;

use crate::error::SelectorParseError;
use crate::types::SelectorList;
use muskitty_css::parser::TokenStream;

/// §18 L4828-4849: Parse A Selector.
///
/// Tokenises `source` with the muskitty-css tokenizer and parses the
/// resulting token stream as a `<complex-selector-list>` per §3
/// L4651-4653. Returns the parsed [`SelectorList`] on success, or a
/// [`SelectorParseError`] describing the failure mode.
///
/// Trailing tokens after the selector list (other than whitespace)
/// produce an `InvalidSelector` error: a selector source must consume
/// the entire input.
pub fn parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    let tokens = muskitty_css::tokenize(source);
    let mut stream = TokenStream::new(tokens);
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
/// SP-2 skeleton: relative-selector parsing (implicit leading
/// combinator + complex-selector) lands in SP-5 together with
/// `:has()`.
pub fn parse_a_relative_selector(_source: &str) -> Result<SelectorList, SelectorParseError> {
    Err(SelectorParseError::NotImplemented)
}
