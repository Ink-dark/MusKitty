//! Selectors Level 4 parser entry points.
//!
//! Implements the §18 "API Hooks" Parse A Selector / Parse A Relative
//! Selector algorithms. The full parsing logic (tokenisation reuse,
//! compound / complex / list parsing) lands in later SP batches
//! (SP-2 .. SP-6). For SP-1 the entry points return
//! [`SelectorParseError::NotImplemented`].
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §18 L4816-5026.

use crate::error::SelectorParseError;
use crate::types::SelectorList;

/// §18 L4828-4849: Parse A Selector.
///
/// Tokenises `source` with the muskitty-css tokenizer and parses the
/// resulting token stream as a selector list per §3 grammar. Returns
/// the parsed [`SelectorList`] on success, or a
/// [`SelectorParseError`] describing the failure mode.
///
/// SP-1 skeleton: not yet implemented.
pub fn parse_a_selector(_source: &str) -> Result<SelectorList, SelectorParseError> {
    Err(SelectorParseError::NotImplemented)
}

/// §18 L4853-4875: Parse A Relative Selector.
///
/// Like [`parse_a_selector`] but the source is interpreted as a
/// relative selector (relative to an implicit `:scope` element, per
/// §3 L1051-1102). Used by `:has()` arguments.
///
/// SP-1 skeleton: not yet implemented.
pub fn parse_a_relative_selector(_source: &str) -> Result<SelectorList, SelectorParseError> {
    Err(SelectorParseError::NotImplemented)
}
