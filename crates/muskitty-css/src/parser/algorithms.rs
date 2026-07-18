//! §5.5 Parser Algorithms.
//!
//! Implementation of the 11 algorithms defined in CSS Syntax Module
//! Level 3 §5.5. This module covers CP-3 (lower-level algorithms):
//! - §5.5.7 consume_a_list_of_component_values
//! - §5.5.8 consume_a_component_value
//! - §5.5.9 consume_a_simple_block
//! - §5.5.10 consume_a_function
//! - §5.5.11 consume_a_unicode_range_value
//!
//! CP-4 will add §5.5.6 (consume_a_declaration +
//! remnants_of_a_bad_declaration); CP-5 will add §5.5.1-§5.5.5.

use super::token_stream::TokenStream;
use super::types::{BlockKind, ComponentValue, Function, SimpleBlock};
use crate::tokenizer::Token;

/// §5.5.8 (L2776-2796) Consume a component value.
///
/// Dispatch on the next token:
/// - `{-token` / `[-token` / `(-token` → consume_a_simple_block
/// - `function-token` → consume_a_function
/// - anything else → consume and return the token as PreservedToken
pub fn consume_a_component_value(input: &mut TokenStream) -> ComponentValue {
    match input.next_token() {
        Token::OpenBrace | Token::OpenBracket | Token::OpenParen => {
            ComponentValue::SimpleBlock(consume_a_simple_block(input))
        }
        Token::Function(_) => ComponentValue::Function(consume_a_function(input)),
        other => {
            input.consume_token();
            ComponentValue::PreservedToken(other)
        }
    }
}

/// §5.5.9 (L2799-2829) Consume a simple block.
///
/// Precondition: next token is `{-token` / `[-token` / `(-token`. The
/// mirror variant becomes the ending token (e.g. `[` → `]`).
/// Repeatedly consume component values until the ending token or EOF.
pub fn consume_a_simple_block(input: &mut TokenStream) -> SimpleBlock {
    let opening = input.next_token();
    let kind = match opening {
        Token::OpenBrace => BlockKind::Curly,
        Token::OpenBracket => BlockKind::Square,
        Token::OpenParen => BlockKind::Paren,
        _ => unreachable!("consume_a_simple_block called on non-opening token"),
    };
    let ending = match kind {
        BlockKind::Curly => Token::CloseBrace,
        BlockKind::Square => Token::CloseBracket,
        BlockKind::Paren => Token::CloseParen,
    };
    // §5.5.9 L2818: discard the opening token.
    input.discard_token();

    let mut block = SimpleBlock {
        kind,
        value: Vec::new(),
    };
    loop {
        // §5.5.9 L2822-2825: EOF or ending token → discard, return block.
        let next = input.next_token();
        if matches!(next, Token::Eof) || next == ending {
            input.discard_token();
            return block;
        }
        // §5.5.9 L2827-2829: anything else → consume a component value
        // and append.
        block.value.push(consume_a_component_value(input));
    }
}

/// §5.5.10 (L2832-2854) Consume a function.
///
/// Precondition: next token is a `function-token`. Consume the function
/// token, then consume component values until `)-token` or EOF.
pub fn consume_a_function(input: &mut TokenStream) -> Function {
    let name = match input.consume_token() {
        Token::Function(name) => name,
        _ => unreachable!("consume_a_function called on non-function token"),
    };
    let mut function = Function {
        name,
        value: Vec::new(),
    };
    loop {
        // §5.5.10 L2847-2850: EOF or `)-token` → discard, return function.
        match input.next_token() {
            Token::Eof | Token::CloseParen => {
                input.discard_token();
                return function;
            }
            // §5.5.10 L2852-2854: anything else → consume a component
            // value and append.
            _ => function.value.push(consume_a_component_value(input)),
        }
    }
}

/// §5.5.7 (L2745-2774) Consume a list of component values.
///
/// `stop_token`: optional token that ends the list (e.g. `;` for
/// declarations). `nested`: when true, an unbalanced `}-token` ends
/// the list without consuming; when false, `}-token` is a parse
/// error and is consumed into the list.
pub fn consume_a_list_of_component_values(
    input: &mut TokenStream,
    stop_token: Option<Token>,
    nested: bool,
) -> Vec<ComponentValue> {
    let mut values = Vec::new();
    loop {
        let next = input.next_token();
        // §5.5.7 L2757-2759: EOF → return values.
        if matches!(next, Token::Eof) {
            return values;
        }
        // §5.5.7 L2757-2759: stop_token → return values.
        if stop_token.as_ref().is_some_and(|s| *s == next) {
            return values;
        }
        match next {
            Token::CloseBrace => {
                if nested {
                    // §5.5.7 L2761-2764: nested → return values without
                    // consuming the `}-token (caller handles it).
                    return values;
                }
                // §5.5.7 L2766-2769: parse error. Consume and append.
                input.consume_token();
                values.push(ComponentValue::PreservedToken(next));
            }
            // §5.5.7 L2771-2773: anything else → consume a component
            // value and append.
            _ => values.push(consume_a_component_value(input)),
        }
    }
}

/// §5.5.11 (L2857-2872) Consume the value of a
/// `@font-face/unicode-range` descriptor.
///
/// Tokenize `input_string` with `unicode_ranges_allowed=true`, then
/// consume a list of component values from the resulting stream.
///
/// Per §5.5.11 L2869-2871 note: "The existence of this algorithm is
/// due to a design mistake in early CSS. It should never be
/// reproduced."
pub fn consume_a_unicode_range_value(input_string: &str) -> Vec<ComponentValue> {
    use crate::tokenizer::{CssTokenizer, Tokenizer};
    let mut tz = CssTokenizer::new(input_string);
    tz.set_unicode_ranges_allowed(true);
    let mut tokens: Vec<Token> = Vec::new();
    while let Some(token) = tz.next_token() {
        let is_eof = matches!(token, Token::Eof);
        tokens.push(token);
        if is_eof {
            break;
        }
    }
    let mut stream = TokenStream::new(tokens);
    consume_a_list_of_component_values(&mut stream, None, false)
}
