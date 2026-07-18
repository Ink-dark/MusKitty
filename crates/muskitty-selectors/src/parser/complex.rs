//! Complex-selector parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <complex-selector> = <complex-selector-unit> [ <combinator>? <complex-selector-unit> ]*
//! ```
//!
//! SP-2 scope: parses a single compound selector wrapped as a
//! one-element complex selector. Combinator parsing (Descendant /
//! Child / NextSibling / SubsequentSibling) lands in SP-6.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4664 + L4665.

use crate::error::SelectorParseError;
use crate::parser::compound::parse_compound_selector;
use crate::types::{ComplexSelector, ComplexSelectorUnit};
use muskitty_css::parser::TokenStream;

/// §3 L4664: Parse a `<complex-selector>`.
///
/// SP-2: parses exactly one `<complex-selector-unit>` (a compound
/// selector with `combinator: None`). SP-6 extends this to handle
/// combinators.
pub fn parse_complex_selector(
    stream: &mut TokenStream,
) -> Result<ComplexSelector, SelectorParseError> {
    let compound = parse_compound_selector(stream)?;
    Ok(ComplexSelector {
        units: vec![ComplexSelectorUnit {
            compound,
            combinator: None,
        }],
    })
}
