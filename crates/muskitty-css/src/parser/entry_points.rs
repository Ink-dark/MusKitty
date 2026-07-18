//! §5.4 Parser Entry Points.
//!
//! Nine entry points producing high-level CSS objects from input. The
//! grammar-based hooks §5.4.1 (`parse something according to a CSS
//! grammar`, L1895-1944) and §5.4.2 (`parse a comma-separated list
//! according to a CSS grammar`, L1949-2001) are deferred — they
//! require grammar knowledge from Selectors / Values specs.
//!
//! # References
//!
//! - CSS Syntax Module Level 3 §5.4 L1816-2206.
//! - Source: `D:\CSSWG\css-syntax-3\Overview.md`.

use super::algorithms::{
    consume_a_blocks_contents, consume_a_component_value, consume_a_declaration,
    consume_a_list_of_component_values, consume_a_qualified_rule, consume_a_stylesheets_contents,
    consume_an_at_rule, BlockContents,
};
use super::token_stream::TokenStream;
use super::types::{ComponentValue, Declaration, Rule, Stylesheet};
use crate::tokenizer::{CssTokenizer, Token, Tokenizer};

/// §5.4 (L1827-1842) Normalize into a token stream.
///
/// Per §5.4 L1837-1840 (the "string" case): filter code points,
/// tokenize the result, return the stream of tokens. Our tokenizer
/// applies §5.3 preprocessing (filter code points) internally, so we
/// just drain all tokens up to and including the `<EOF-token>`.
fn normalize_from_string(input: &str) -> TokenStream {
    let mut tz = CssTokenizer::new(input);
    let mut tokens = Vec::new();
    while let Some(token) = tz.next_token() {
        let is_eof = matches!(token, Token::Eof);
        tokens.push(token);
        if is_eof {
            break;
        }
    }
    TokenStream::new(tokens)
}

/// §5.4.3 (L2005-2033) Parse a stylesheet.
///
/// Normalize → consume a stylesheet's contents → wrap in `Stylesheet`.
///
/// The spec also accepts an optional location; we do not model the
/// stylesheet location here (deferred to CSSOM integration).
pub fn parse_a_stylesheet(input: &str) -> Stylesheet {
    let mut stream = normalize_from_string(input);
    let rules = consume_a_stylesheets_contents(&mut stream);
    Stylesheet { rules }
}

/// §5.4.4 (L2037-2051) Parse a stylesheet's contents.
///
/// Normalize → consume a stylesheet's contents → return the rule list.
pub fn parse_a_stylesheets_contents(input: &str) -> Vec<Rule> {
    let mut stream = normalize_from_string(input);
    consume_a_stylesheets_contents(&mut stream)
}

/// §5.4.5 (L2055-2069) Parse a block's contents.
///
/// Normalize → consume a block's contents → return `BlockContents`.
pub fn parse_a_blocks_contents(input: &str) -> BlockContents {
    let mut stream = normalize_from_string(input);
    consume_a_blocks_contents(&mut stream)
}

/// §5.4.6 (L2073-2109) Parse a rule.
///
/// # Algorithm
///
/// 1. Normalize input.
/// 2. Discard whitespace.
/// 3. If next token is EOF → syntax error (`None`).
///    If next token is at-keyword → consume an at-rule; on `None`,
///    syntax error (`None`).
///    Otherwise → consume a qualified rule (non-nested); on nothing or
///    invalid rule error, syntax error (`None`).
/// 4. Discard whitespace.
/// 5. If next token is EOF → return rule; otherwise syntax error
///    (`None`).
pub fn parse_a_rule(input: &str) -> Option<Rule> {
    let mut stream = normalize_from_string(input);
    stream.discard_whitespace();
    let rule = match stream.next_token() {
        // §5.4.6 L2088-2089: EOF → syntax error.
        Token::Eof => return None,
        // §5.4.6 L2091-2094: at-keyword → consume an at-rule.
        Token::AtKeyword(_) => consume_an_at_rule(&mut stream, false).map(Rule::AtRule),
        // §5.4.6 L2096-2100: otherwise → consume a qualified rule. If
        // nothing or invalid rule error → syntax error (None).
        _ => consume_a_qualified_rule(&mut stream, None, false)
            .ok()
            .flatten()
            .map(Rule::QualifiedRule),
    };
    // §5.4.6 L2103: discard whitespace.
    stream.discard_whitespace();
    // §5.4.6 L2106-2108: if EOF → return rule; otherwise syntax error.
    if stream.is_empty() {
        rule
    } else {
        None
    }
}

/// §5.4.7 (L2113-2134) Parse a declaration.
///
/// Normalize → discard whitespace → consume a declaration (non-nested).
/// If anything was returned, return it; otherwise syntax error
/// (`None`).
pub fn parse_a_declaration(input: &str) -> Option<Declaration> {
    let mut stream = normalize_from_string(input);
    stream.discard_whitespace();
    consume_a_declaration(&mut stream, false)
}

/// §5.4.8 (L2138-2168) Parse a component value.
///
/// # Algorithm
///
/// 1. Normalize input.
/// 2. Discard whitespace.
/// 3. If input is empty → syntax error (`None`).
/// 4. Consume a component value, let `value` be the return value.
/// 5. Discard whitespace.
/// 6. If input is empty → return `value`; otherwise syntax error
///    (`None`).
pub fn parse_a_component_value(input: &str) -> Option<ComponentValue> {
    let mut stream = normalize_from_string(input);
    stream.discard_whitespace();
    // §5.4.8 L2153-2154: empty → syntax error.
    if stream.is_empty() {
        return None;
    }
    // §5.4.8 L2157-2158: consume a component value.
    let value = consume_a_component_value(&mut stream);
    // §5.4.8 L2161: discard whitespace.
    stream.discard_whitespace();
    // §5.4.8 L2164-2167: if empty → return value; else syntax error.
    if stream.is_empty() {
        Some(value)
    } else {
        None
    }
}

/// §5.4.9 (L2172-2183) Parse a list of component values.
///
/// Normalize → consume a list of component values (no stop token,
/// non-nested) → return the result.
pub fn parse_a_list_of_component_values(input: &str) -> Vec<ComponentValue> {
    let mut stream = normalize_from_string(input);
    consume_a_list_of_component_values(&mut stream, None, false)
}

/// §5.4.10 (L2186-2204) Parse a comma-separated list of component
/// values.
///
/// # Algorithm
///
/// 1. Normalize input.
/// 2. Let `groups` be an empty list.
/// 3. While input is not empty:
///    a. Consume a list of component values with `,` as the stop token and append to `groups`.
///    b. Discard a token (the comma; `discard_token` is a no-op on EOF).
/// 4. Return `groups`.
pub fn parse_a_comma_separated_list_of_component_values(input: &str) -> Vec<Vec<ComponentValue>> {
    let mut stream = normalize_from_string(input);
    let mut groups = Vec::new();
    while !stream.is_empty() {
        // §5.4.10 L2199-2201: consume a list of component values with
        // `,` as the stop token.
        let group = consume_a_list_of_component_values(&mut stream, Some(Token::Comma), false);
        groups.push(group);
        // §5.4.10 L2202: discard a token (the comma). On EOF, this is
        // a no-op (§5.3 L1784-1786).
        stream.discard_token();
    }
    groups
}
