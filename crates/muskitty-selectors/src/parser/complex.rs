//! Complex-selector parsing.
//!
//! Implements the §3 grammar production:
//!
//! ```text
//! <complex-selector> = <complex-selector-unit> [ <combinator>? <complex-selector-unit> ]*
//! ```
//!
//! SP-2..SP-6 scope: parses one or more compound selectors joined by
//! the four §15 combinators (Descendant / Child / NextSibling /
//! SubsequentSibling). Trailing combinators (e.g. `a >`) produce an
//! `InvalidSelector` error. Mixed combinators (`a > b + c`), pseudo-
//! class-terminated compounds (`a > b:hover`), and selector lists
//! with trailing-comma / trailing-combinator / empty-input rejection
//! are all handled (see tests/parser_complex.rs).
//!
//! # Storage convention
//!
//! Per [`crate::types::ComplexSelector`]: storage is rightmost-first.
//! `units[0]` is the subject (rightmost compound in source order);
//! `units[len-1]` is the leftmost compound in source order. The
//! combinator on `units[idx]` links it to `units[idx+1]` (the next
//! leftward compound) and is stored on the rightward unit. The
//! leftmost unit (`units[len-1]`) always has `combinator == None`.
//!
//! For example, `.a > .b` parses to `units = [{ .b, Some(Child) },
//! { .a, None }]` — `.b` is the subject (`units[0]`) carrying the
//! Child combinator that links it to `.a`; `.a` is the leftmost unit
//! with `combinator == None`.
//!
//! Spec source: `D:\CSSWG\selectors-4\Overview.md`, §3 L4664-4665,
//! §15 L4360-4532 (combinators), §3 L4704-4741 (whitespace rules).

use crate::error::SelectorParseError;
use crate::parser::compound::parse_compound_selector;
use crate::types::{Combinator, ComplexSelector, ComplexSelectorUnit};
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §3 L4664: Parse a `<complex-selector>`.
///
/// Parses one or more `<compound-selector>`s joined by combinators.
/// Storage is rightmost-first: `units[0]` is the subject (rightmost
/// compound), `units[len-1]` is the leftmost compound. The combinator
/// on each unit links it to the next leftward unit (`units[idx+1]`);
/// the leftmost unit has `combinator == None`.
///
/// # Combinator handling (§15 L4360-4532)
///
/// - `>` (Child), `+` (NextSibling), `~` (SubsequentSibling) —
///   explicit combinators; optional whitespace is allowed before and
///   after.
/// - Whitespace alone — implicit Descendant combinator; only valid
///   when followed by another compound selector. Trailing whitespace
///   (followed by a terminator) is not treated as a combinator.
/// - Trailing combinator (e.g. `a >`, `a +`) — `Err(InvalidSelector)`.
///
/// # Termination
///
/// The parser stops at any token that cannot extend the complex
/// selector: EOF, `,` (list separator), or a block-closing token
/// (`)`, `}`, `]`). The terminator is left unconsumed for the caller
/// (e.g. `parse_selector_list` consumes `,`; the API entry point
/// checks for unexpected trailing tokens).
pub fn parse_complex_selector(
    stream: &mut TokenStream,
) -> Result<ComplexSelector, SelectorParseError> {
    // Build in source order (left-to-right), then reverse so storage
    // is rightmost-first. The combinator goes on the rightward unit
    // (the one just parsed), per the storage convention documented on
    // [`crate::types::ComplexSelector`].
    let mut units: Vec<ComplexSelectorUnit> = Vec::new();
    let first_compound = parse_compound_selector(stream)?;
    units.push(ComplexSelectorUnit {
        compound: first_compound,
        combinator: None,
    });

    loop {
        // Detect leading whitespace (potential implicit descendant
        // combinator). §3 L4724-4727: whitespace between two
        // complex-selector-units is required if no explicit combinator
        // is present.
        let had_whitespace = matches!(stream.next_token(), Token::Whitespace);
        if had_whitespace {
            stream.discard_whitespace();
        }

        // Try an explicit combinator (§15 L4422 / L4463 / L4505).
        let explicit_combinator = match stream.next_token() {
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

        if let Some(comb) = explicit_combinator {
            // Explicit combinator: consume optional trailing whitespace,
            // then require another compound selector. A terminator
            // here means a trailing combinator (e.g. `a >`).
            stream.discard_whitespace();
            if is_complex_terminator(&stream.next_token()) {
                return Err(SelectorParseError::InvalidSelector(
                    "trailing combinator in complex selector".into(),
                ));
            }
            let next_compound = parse_compound_selector(stream)?;
            // Combinator goes on the new (rightward) unit.
            units.push(ComplexSelectorUnit {
                compound: next_compound,
                combinator: Some(comb),
            });
            continue;
        }

        // No explicit combinator. Without preceding whitespace this
        // is the end of the complex selector.
        if !had_whitespace {
            break;
        }

        // Had whitespace but no explicit combinator: either an
        // implicit descendant combinator (followed by another
        // compound) or trailing whitespace (followed by a
        // terminator).
        if is_complex_terminator(&stream.next_token()) {
            break; // trailing whitespace
        }

        // Implicit descendant combinator (§15 L4363). Parse the next
        // compound.
        let next_compound = parse_compound_selector(stream)?;
        units.push(ComplexSelectorUnit {
            compound: next_compound,
            combinator: Some(Combinator::Descendant),
        });
    }

    // Reverse so storage is rightmost-first: units[0] = subject
    // (rightmost in source), units[len-1] = leftmost in source.
    units.reverse();
    Ok(ComplexSelector { units })
}

/// A token that cannot follow a compound selector in a complex
/// selector without an intervening combinator. Includes the
/// selector-list separator (`,`) and block-closing tokens that
/// terminate a selector list.
fn is_complex_terminator(token: &Token) -> bool {
    matches!(
        token,
        Token::Eof | Token::Comma | Token::CloseParen | Token::CloseBrace | Token::CloseBracket
    )
}
