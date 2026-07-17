//! MusKitty CSS Parser
//!
//! Implements the CSS Syntax Module Level 3 tokenization and parsing
//! algorithm.
//!
//! # Architecture
//!
//! The parser follows the two-stage model described in CSS Syntax §3.1:
//! 1. **Tokenization** ([`tokenizer`]) — consumes a stream of Unicode code
//!    points and emits tokens (ident, function, at-keyword, hash, string,
//!    number, dimension, etc.).
//! 2. **Tree construction** (deferred to Phase 2 sub-stage 4, CSSOM) —
//!    consumes tokens and builds the stylesheet object model.
//!
//! Unlike the HTML tokenizer (which is a state machine with ~80 explicit
//! states per WHATWG §13.2.5), the CSS tokenizer is a recursive-descent
//! algorithm organized around §4.3 "Tokenizer Algorithms": a single
//! `consume_a_token` entry point (§4.3.1) dispatches to sub-algorithms
//! such as `consume_an_ident_like_token` (§4.3.4), `consume_a_numeric_token`
//! (§4.3.3), `consume_a_string_token` (§4.3.5), and `consume_a_url_token`
//! (§4.3.6). These in turn call algorithm primitives like
//! `consume_an_escaped_code_point` (§4.3.7), `consume_an_ident_sequence`
//! (§4.3.12), and `consume_a_number` (§4.3.13).
//!
//! # References
//!
//! - CSS Syntax Module Level 3: <https://drafts.csswg.org/css-syntax-3/>
//! - WPT CSS test suite: <https://github.com/web-platform-tests/wpt/tree/master/css>

pub mod tokenizer;

use crate::tokenizer::{CssTokenizer, Token, Tokenizer};

/// Tokenize a CSS input string into a vector of tokens.
///
/// Implements the tokenization stage of CSS Syntax §3.1: construct a
/// tokenizer over `input` (after §5.3 input preprocessing, which the
/// tokenizer applies internally), then drain all tokens up to and
/// including `<EOF-token>`.
///
/// Returns the token stream without the trailing `<EOF-token>`. Parse
/// errors are currently discarded; a future API will expose them.
///
/// # Examples
///
/// ```
/// use muskitty_css::tokenize;
/// use muskitty_css::tokenizer::Token;
///
/// // C-0: only whitespace and simple punctuation tokens are implemented;
/// // ident/number/string/etc. are added in subsequent commits.
/// let tokens = tokenize(": , ;");
/// assert!(matches!(tokens[0], Token::Colon));
/// assert!(matches!(tokens[1], Token::Whitespace));
/// assert!(matches!(tokens[2], Token::Comma));
/// assert!(matches!(tokens[3], Token::Whitespace));
/// assert!(matches!(tokens[4], Token::Semicolon));
/// ```
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tz = CssTokenizer::new(input);
    let mut out = Vec::new();
    while let Some(token) = tz.next_token() {
        if matches!(token, Token::Eof) {
            break;
        }
        out.push(token);
    }
    out
}
