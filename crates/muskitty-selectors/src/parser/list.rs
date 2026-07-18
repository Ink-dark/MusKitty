//! Selector-list parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <complex-selector-list> = <complex-selector>#
//! ```
//!
//! Where `#` indicates a comma-separated list of one or more
//! productions. Trailing comma is not allowed.
//!
//! Also implements `forgiving-selector-list` (§3 L4765-4813) used by
//! `:is()` / `:where()`: parses each complex selector independently
//! and silently drops the ones that fail.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4651-4653 +
//! L4765-4813.

use crate::error::SelectorParseError;
use crate::parser::complex::parse_complex_selector;
use crate::types::SelectorList;
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §3 L4651-4653: Parse a `<complex-selector-list>` =
/// `<complex-selector>#`.
///
/// Parses one or more complex selectors separated by commas. Stops at
/// EOF or any token that does not start a complex selector (e.g.
/// `)` for a function argument, `}` for a block). The terminator is
/// left unconsumed.
///
/// Whitespace around commas is discarded (§3 L4704-4727 — whitespace is
/// allowed between a comma and the following complex selector).
///
/// Returns `Err(InvalidSelector)` if no complex selector was parsed
/// (empty list), or if a comma is followed by EOF / a terminator (a
/// trailing comma).
pub fn parse_selector_list(stream: &mut TokenStream) -> Result<SelectorList, SelectorParseError> {
    let mut selectors = Vec::new();

    // Required first complex selector.
    stream.discard_whitespace();
    selectors.push(parse_complex_selector(stream)?);

    // Optional trailing complex selectors separated by commas.
    loop {
        stream.discard_whitespace();
        match stream.next_token() {
            Token::Comma => {
                stream.discard_token(); // consume ','
                stream.discard_whitespace();
                // Must be followed by another complex selector.
                if is_terminator(&stream.next_token()) {
                    return Err(SelectorParseError::InvalidSelector(
                        "trailing comma in selector list".into(),
                    ));
                }
                selectors.push(parse_complex_selector(stream)?);
            }
            _ => break, // terminator or other token — stop, leave it unconsumed.
        }
    }

    Ok(SelectorList(selectors))
}

/// §3 L4765-4813: Parse a `forgiving-selector-list`.
///
/// Like [`parse_selector_list`], but each complex selector is parsed
/// independently. Selectors that fail to parse are silently dropped
/// instead of failing the whole list. If every selector fails, the
/// returned list is empty (this is permitted by the forgiving
/// production).
pub fn parse_forgiving_selector_list(
    stream: &mut TokenStream,
) -> Result<SelectorList, SelectorParseError> {
    let mut selectors = Vec::new();

    stream.discard_whitespace();
    // First selector — if it fails, just skip it (no preceding comma
    // to consume, the caller manages stream state for non-forgiving
    // invocations).
    match parse_complex_selector(stream) {
        Ok(cs) => selectors.push(cs),
        Err(_) => {
            // Skip a single token to make progress; this is a
            // best-effort recovery. The spec doesn't precisely
            // describe recovery, but per §3 L4789-4799 "parse as a
            // forgiving selector list" delegates to "parse a list of
            // complex-real-selectors" which itself drops failures.
            stream.discard_token();
        }
    }

    loop {
        stream.discard_whitespace();
        match stream.next_token() {
            Token::Comma => {
                stream.discard_token(); // consume ','
                stream.discard_whitespace();
                if is_terminator(&stream.next_token()) {
                    // Trailing comma in forgiving mode: treat as end
                    // of list, do not error.
                    break;
                }
                match parse_complex_selector(stream) {
                    Ok(cs) => selectors.push(cs),
                    Err(_) => stream.discard_token(),
                }
            }
            _ => break,
        }
    }

    Ok(SelectorList(selectors))
}

/// Heuristic: a token that cannot start a complex selector and
/// therefore terminates a selector list. Anything not in this set is
/// treated as a potential complex-selector starter.
fn is_terminator(token: &Token) -> bool {
    matches!(
        token,
        Token::Eof | Token::CloseParen | Token::CloseBrace | Token::CloseBracket
    )
}
