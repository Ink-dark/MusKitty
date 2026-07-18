//! MusKitty CSS Parser
//!
//! Implements the CSS Syntax Module Level 3 tokenization and parsing
//! algorithms.
//!
//! # Architecture
//!
//! The parser follows the two-stage model described in CSS Syntax §3.1:
//! 1. **Tokenization** ([`tokenizer`]) — consumes a stream of Unicode
//!    code points and emits tokens (§4.3, fully implemented).
//! 2. **Parsing** ([`parser`]) — consumes tokens and produces CSS
//!    objects: stylesheets, rules, declarations, component values
//!    (§5, fully implemented).
//!
//! # Top-level API
//!
//! - [`parse_stylesheet`] — full stylesheet parse (§5.4.3)
//! - [`parse_rule`] — single rule parse (§5.4.6)
//! - [`parse_declaration`] — single declaration parse (§5.4.7)
//! - [`parse_component_value`] — single component value (§5.4.8)
//! - [`parse_list_of_component_values`] — list of component values (§5.4.9)
//! - [`parse_comma_separated_list_of_component_values`] — comma-separated list (§5.4.10)
//! - [`tokenize`] — token stream only (§4.3)
//!
//! # References
//!
//! - CSS Syntax Module Level 3: <https://drafts.csswg.org/css-syntax-3/>
//! - Spec source (Markdown): `D:\CSSWG\css-syntax-3\Overview.md`
//! - WPT CSS test suite: <https://github.com/web-platform-tests/wpt/tree/master/css>

pub mod parser;
pub mod tokenizer;

use crate::parser::{
    parse_a_comma_separated_list_of_component_values, parse_a_component_value, parse_a_declaration,
    parse_a_list_of_component_values, parse_a_rule, parse_a_stylesheet, ComponentValue,
    Declaration, Rule, Stylesheet,
};
use crate::tokenizer::{CssTokenizer, Token, Tokenizer};

// Re-export the key parser types at the crate root for convenience.
pub use parser::{BlockKind, Function, SimpleBlock};

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
/// let tokens = tokenize("color: red;");
/// assert!(matches!(tokens[0], Token::Ident(_)));
/// assert!(matches!(tokens[1], Token::Colon));
/// assert!(matches!(tokens[2], Token::Whitespace));
/// assert!(matches!(tokens[3], Token::Ident(_)));
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

/// Parse a CSS string into a [`Stylesheet`] (§5.4.3).
///
/// Implements `parse a stylesheet` from CSS Syntax §5.4.3: tokenize
/// the input (with §5.3 preprocessing), then `consume a stylesheet's
/// contents` (§5.5.1) to produce the list of rules.
///
/// # Examples
///
/// ```
/// use muskitty_css::parse_stylesheet;
///
/// let ss = parse_stylesheet("a { color: red; }");
/// assert_eq!(ss.rules.len(), 1);
/// ```
pub fn parse_stylesheet(input: &str) -> Stylesheet {
    parse_a_stylesheet(input)
}

/// Parse a CSS string into a single [`Rule`] (§5.4.6).
///
/// Returns `None` for a syntax error (empty input or trailing garbage).
///
/// # Examples
///
/// ```
/// use muskitty_css::parse_rule;
///
/// assert!(parse_rule("@media print {}").is_some());
/// assert!(parse_rule("").is_none());
/// ```
pub fn parse_rule(input: &str) -> Option<Rule> {
    parse_a_rule(input)
}

/// Parse a CSS string into a single [`Declaration`] (§5.4.7).
///
/// Returns `None` for a syntax error (malformed declaration).
///
/// # Examples
///
/// ```
/// use muskitty_css::parse_declaration;
///
/// let decl = parse_declaration("color: red").unwrap();
/// assert_eq!(decl.name, "color");
/// ```
pub fn parse_declaration(input: &str) -> Option<Declaration> {
    parse_a_declaration(input)
}

/// Parse a CSS string into a single [`ComponentValue`] (§5.4.8).
///
/// Returns `None` for a syntax error (empty input or trailing
/// garbage after the component value).
///
/// # Examples
///
/// ```
/// use muskitty_css::parse_component_value;
/// use muskitty_css::parser::ComponentValue;
/// use muskitty_css::tokenizer::Token;
///
/// let cv = parse_component_value("red").unwrap();
/// assert!(matches!(cv, ComponentValue::PreservedToken(Token::Ident(_))));
/// ```
pub fn parse_component_value(input: &str) -> Option<ComponentValue> {
    parse_a_component_value(input)
}

/// Parse a CSS string into a list of [`ComponentValue`] (§5.4.9).
///
/// # Examples
///
/// ```
/// use muskitty_css::parse_list_of_component_values;
///
/// // "a b c" → 5 component values: a, ws, b, ws, c.
/// let list = parse_list_of_component_values("a b c");
/// assert_eq!(list.len(), 5);
/// ```
pub fn parse_list_of_component_values(input: &str) -> Vec<ComponentValue> {
    parse_a_list_of_component_values(input)
}

/// Parse a CSS string into a comma-separated list of [`ComponentValue`]
/// (§5.4.10).
///
/// # Examples
///
/// ```
/// use muskitty_css::parse_comma_separated_list_of_component_values;
///
/// // "a, b, c" → 3 groups. Per §5.4.10, the algorithm does not
/// // discard leading whitespace between commas, so groups after the
/// // first carry a leading whitespace token.
/// let groups = parse_comma_separated_list_of_component_values("a, b, c");
/// assert_eq!(groups.len(), 3);
/// ```
pub fn parse_comma_separated_list_of_component_values(input: &str) -> Vec<Vec<ComponentValue>> {
    parse_a_comma_separated_list_of_component_values(input)
}
