//! An+B microsyntax parser.
//!
//! Implements the `<an-plus-b>` production per CSS Syntax Module Level
//! 3 §7 "The An+B Microsyntax" (L2875-3107). Used by the `:nth-child()`,
//! `:nth-last-child()`, `:nth-of-type()`, and `:nth-last-of-type()`
//! pseudo-classes (Selectors Level 4 §13).
//!
//! # Supported forms
//!
//! Per CSS Syntax §7 L3026-3107 (whitespace around the `+`/`-` that
//! separates the An and B parts is permitted but optional):
//!
//! | Form                                        | (A, B)               |
//! | ------------------------------------------- | -------------------- |
//! | `odd`                                       | (2, 1)               |
//! | `even`                                      | (2, 0)               |
//! | `<integer>` (e.g. `5`)                      | (0, 5)               |
//! | `<n-dimension>` (e.g. `2n`)                 | (value, 0)           |
//! | `n` / `+n` / `-n` (ident-token)             | (1, 0) / (1, 0) / (-1, 0) |
//! | `<ndashdigit-dimension>` (`2n-3`)            | (value, -3)          |
//! | `n-3` / `+n-3` (`<ndashdigit-ident>`)        | (1, -3)              |
//! | `-n-3` (`<dashndashdigit-ident>`)           | (-1, -3)             |
//! | `<n-dimension> <signed-integer>`            | (value, integer)      |
//! | `n <signed-integer>` / `-n <signed-integer>` | (±1, integer)        |
//! | `<ndash-dimension> <signless-integer>`      | (value, -integer)    |
//! | `n- <signless-integer>` / `-n- <signless>`   | (±1, -integer)       |
//! | `<n-dimension> ['+'|'-'] <signless-integer>`| (value, ±integer)    |
//! | `n ['+'|'-'] <signless>` / `-n ['+'|'-'] ...`| (±1, ±integer)       |
//!
//! # Note on the `<signless-integer>` distinction
//!
//! The CSS tokenizer's [`Numeric`](muskitty_css::tokenizer::Numeric)
//! struct does not currently expose whether a `<number-token>` carried
//! a sign character. Therefore this parser accepts any integer
//! `<number-token>` where the spec demands a `<signless-integer>`.
//! This is more permissive than the spec but produces identical
//! results for all valid inputs; only invalid forms like `2n- -3`
//! (which the spec rejects) are accepted with a best-effort B value.
//!
//! # Note on the `of S` clause
//!
//! This module parses only the An+B production. The optional
//! `of <selector-list>` clause accepted by `:nth-child()` and
//! `:nth-last-child()` (§13.3 L3968 / §13.4 L4077) is parsed by
//! [`crate::parser::simple::parse_optional_of_selector_list`]
//! after this function returns.

use crate::error::SelectorParseError;
use crate::types::AnPlusB;
use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// Kind of "n-unit" parsed from a Dimension unit string or an Ident
/// value. Encodes the `n`-shape; the sign of `A` is determined by the
/// caller based on whether the unit came from a Dimension (sign in
/// the leading number) or an Ident (sign as part of the ident value).
enum NUnit {
    /// `n` — B is 0 unless followed by a B-part.
    Plain,
    /// `n-` — must be followed by `<signless-integer>`; B = -integer.
    Dash,
    /// `n-<digits>` — B is already encoded (and is negative).
    DashDigits(i64),
}

/// Parse the `<an-plus-b>` production per CSS Syntax §7.
///
/// Pre-condition: the next token in `stream` is the start of an An+B
/// form (leading whitespace is discarded).
///
/// Returns the parsed `AnPlusB` value, or `Err(InvalidAnPlusB)` if
/// the input does not match any valid form. On error, the stream
/// position is unspecified (some tokens may have been consumed).
pub fn parse_an_plus_b(stream: &mut TokenStream) -> Result<AnPlusB, SelectorParseError> {
    stream.discard_whitespace();

    match stream.next_token() {
        // Form: `<integer>` → (0, integer).
        Token::Number(ref numeric) => {
            if !numeric.is_integer {
                return Err(SelectorParseError::InvalidAnPlusB);
            }
            stream.discard_token();
            Ok(AnPlusB {
                a: 0,
                b: numeric.value as i64,
            })
        }
        // Forms: `<n-dimension>` / `<ndash-dimension>` /
        // `<ndashdigit-dimension>`. The A value comes from the leading
        // number; the unit encodes the n-shape.
        Token::Dimension(ref numeric, ref unit) => {
            if !numeric.is_integer {
                return Err(SelectorParseError::InvalidAnPlusB);
            }
            let a = numeric.value as i64;
            let unit_kind = parse_n_unit_kind(unit)?;
            stream.discard_token();
            finish_after_n(stream, a, unit_kind)
        }
        // Forms starting with an ident: `odd` / `even` / `n` / `-n` /
        // `n-<digits>` / `-n-<digits>` / `n-` / `-n-`.
        Token::Ident(ref s) => {
            if s.eq_ignore_ascii_case("odd") {
                stream.discard_token();
                return Ok(AnPlusB { a: 2, b: 1 });
            }
            if s.eq_ignore_ascii_case("even") {
                stream.discard_token();
                return Ok(AnPlusB { a: 2, b: 0 });
            }
            // The ident encodes both the A sign and the n-shape.
            let a = if s.starts_with('-') { -1 } else { 1 };
            let unit_kind = parse_n_unit_kind(s)?;
            stream.discard_token();
            finish_after_n(stream, a, unit_kind)
        }
        // Form: `+'? n` / `+'? <ndashdigit-ident>` / `+'? n- ...`.
        // The `+` is a Delim token (since `+` followed by `n` is not
        // a number-start in the tokenizer). Per §7 L3046-3049, no
        // whitespace is allowed between the `+` and the following
        // ident.
        Token::Delim('+') => {
            stream.discard_token(); // consume '+'
            match stream.next_token() {
                Token::Ident(ref s) => {
                    // The ident must start with 'n' (case-insensitive);
                    // a leading '-' after '+' is invalid.
                    let lower = s.to_ascii_lowercase();
                    if !lower.starts_with('n') {
                        return Err(SelectorParseError::InvalidAnPlusB);
                    }
                    let unit_kind = parse_n_unit_kind(s)?;
                    stream.discard_token();
                    finish_after_n(stream, 1, unit_kind)
                }
                _ => Err(SelectorParseError::InvalidAnPlusB),
            }
        }
        _ => Err(SelectorParseError::InvalidAnPlusB),
    }
}

/// Parse a string as an `n`-unit shape (Plain / Dash / DashDigits).
/// Does not return the A sign — the caller determines A separately
/// (from the leading number for a Dimension, or from a leading `-`
/// for an Ident).
fn parse_n_unit_kind(s: &str) -> Result<NUnit, SelectorParseError> {
    let lower = s.to_ascii_lowercase();
    if lower == "n" || lower == "-n" {
        return Ok(NUnit::Plain);
    }
    if let Some(rest) = lower.strip_prefix("n-") {
        return parse_n_dash_rest(rest);
    }
    if let Some(rest) = lower.strip_prefix("-n-") {
        return parse_n_dash_rest(rest);
    }
    Err(SelectorParseError::InvalidAnPlusB)
}

/// Helper for [`parse_n_unit_kind`]: given the part after `n-` or
/// `-n-`, return the corresponding [`NUnit`].
fn parse_n_dash_rest(rest: &str) -> Result<NUnit, SelectorParseError> {
    if rest.is_empty() {
        return Ok(NUnit::Dash);
    }
    if rest.chars().all(|c| c.is_ascii_digit()) {
        let b: i64 = rest
            .parse()
            .map_err(|_| SelectorParseError::InvalidAnPlusB)?;
        Ok(NUnit::DashDigits(-b))
    } else {
        Err(SelectorParseError::InvalidAnPlusB)
    }
}

/// After consuming an `n`-form (with A already known), finish parsing
/// the An+B production by consuming any trailing B-part.
fn finish_after_n(
    stream: &mut TokenStream,
    a: i64,
    unit_kind: NUnit,
) -> Result<AnPlusB, SelectorParseError> {
    match unit_kind {
        // B is fully encoded in the unit (e.g. `2n-3`); nothing more
        // to consume.
        NUnit::DashDigits(b) => Ok(AnPlusB { a, b }),
        // `n-` / `-n-` / `<ndash-dimension>`: must be followed by a
        // `<signless-integer>`. See the module doc for the
        // permissiveness note.
        NUnit::Dash => {
            stream.discard_whitespace();
            match stream.next_token() {
                Token::Number(ref numeric) if numeric.is_integer => {
                    stream.discard_token();
                    Ok(AnPlusB {
                        a,
                        b: -(numeric.value as i64),
                    })
                }
                _ => Err(SelectorParseError::InvalidAnPlusB),
            }
        }
        // Plain `n` / `-n` / `<n-dimension>`: optional B-part may
        // follow as `<signed-integer>` or `['+'|'-'] <signless-integer>`.
        NUnit::Plain => {
            stream.discard_whitespace();
            match stream.next_token() {
                // No B-part — terminator (or anything that's not a
                // Number or `+`/`-` Delim) ends the production. B = 0.
                Token::Eof | Token::CloseParen | Token::CloseBracket | Token::CloseBrace => {
                    Ok(AnPlusB { a, b: 0 })
                }
                // `<signed-integer>` — single Number token. The
                // tokenizer's Numeric doesn't expose sign presence, so
                // any integer Number is accepted.
                Token::Number(ref numeric) if numeric.is_integer => {
                    stream.discard_token();
                    Ok(AnPlusB {
                        a,
                        b: numeric.value as i64,
                    })
                }
                // `['+'|'-'] <signless-integer>` — Delim then Number.
                Token::Delim(sign) if sign == '+' || sign == '-' => {
                    stream.discard_token(); // consume sign
                    stream.discard_whitespace();
                    match stream.next_token() {
                        Token::Number(ref numeric) if numeric.is_integer => {
                            stream.discard_token();
                            let b_abs = numeric.value as i64;
                            let b = if sign == '-' { -b_abs } else { b_abs };
                            Ok(AnPlusB { a, b })
                        }
                        _ => Err(SelectorParseError::InvalidAnPlusB),
                    }
                }
                // Anything else: no B-part was provided; treat as end
                // of the An+B production. The pseudo-class parser will
                // fail if a `)` was expected here and not found.
                _ => Ok(AnPlusB { a, b: 0 }),
            }
        }
    }
}
