//! §5.5.7-§5.5.11 lower-level parser algorithm tests (CP-3).

use muskitty_css::parser::{
    consume_a_component_value, consume_a_function, consume_a_list_of_component_values,
    consume_a_simple_block, consume_a_unicode_range_value, BlockKind, ComponentValue, TokenStream,
};
use muskitty_css::tokenizer::Token;

/// §5.5.8: An ident-token (non-block, non-function) is consumed as a
/// PreservedToken.
#[test]
fn component_value_preserved_token() {
    let mut stream = TokenStream::new(vec![Token::Ident("foo".to_string()), Token::Eof]);
    let cv = consume_a_component_value(&mut stream);
    match cv {
        ComponentValue::PreservedToken(Token::Ident(s)) => assert_eq!(s, "foo"),
        other => panic!("expected PreservedToken(Ident), got {other:?}"),
    }
}

/// §5.5.9: `{ foo }` produces a Curly SimpleBlock containing one
/// PreservedToken(Ident("foo")).
#[test]
fn component_value_simple_block_curly() {
    let mut stream = TokenStream::new(vec![
        Token::OpenBrace,
        Token::Ident("foo".to_string()),
        Token::CloseBrace,
        Token::Eof,
    ]);
    let cv = consume_a_component_value(&mut stream);
    match cv {
        ComponentValue::SimpleBlock(b) => {
            assert_eq!(b.kind, BlockKind::Curly);
            assert_eq!(b.value.len(), 1);
        }
        other => panic!("expected SimpleBlock, got {other:?}"),
    }
}

/// §5.5.9: `[ 1 2 ]` produces a Square SimpleBlock containing two
/// Number tokens.
#[test]
fn component_value_simple_block_square() {
    use muskitty_css::tokenizer::Numeric;
    let mut stream = TokenStream::new(vec![
        Token::OpenBracket,
        Token::Number(Numeric {
            value: 1.0,
            is_integer: true,
        }),
        Token::Number(Numeric {
            value: 2.0,
            is_integer: true,
        }),
        Token::CloseBracket,
        Token::Eof,
    ]);
    let cv = consume_a_component_value(&mut stream);
    match cv {
        ComponentValue::SimpleBlock(b) => {
            assert_eq!(b.kind, BlockKind::Square);
            assert_eq!(b.value.len(), 2);
        }
        other => panic!("expected SimpleBlock, got {other:?}"),
    }
}

/// §5.5.10: `foo(1)` produces a Function("foo", [Number(1)]).
#[test]
fn component_value_function() {
    use muskitty_css::tokenizer::Numeric;
    let mut stream = TokenStream::new(vec![
        Token::Function("foo".to_string()),
        Token::Number(Numeric {
            value: 1.0,
            is_integer: true,
        }),
        Token::CloseParen,
        Token::Eof,
    ]);
    let cv = consume_a_component_value(&mut stream);
    match cv {
        ComponentValue::Function(f) => {
            assert_eq!(f.name, "foo");
            assert_eq!(f.value.len(), 1);
        }
        other => panic!("expected Function, got {other:?}"),
    }
}

/// §5.5.9 L2822-2825: An unclosed `{`-block at EOF must still return
/// the block (with whatever was consumed before EOF).
#[test]
fn simple_block_unclosed_at_eof() {
    let mut stream = TokenStream::new(vec![
        Token::OpenBrace,
        Token::Ident("foo".to_string()),
        // EOF implicit
    ]);
    let block = consume_a_simple_block(&mut stream);
    assert_eq!(block.kind, BlockKind::Curly);
    assert_eq!(block.value.len(), 1);
}

/// §5.5.10 L2847-2850: An unclosed `foo(` at EOF must still return
/// the function (with whatever was consumed before EOF).
#[test]
fn function_unclosed_at_eof() {
    let mut stream = TokenStream::new(vec![
        Token::Function("foo".to_string()),
        Token::Ident("bar".to_string()),
        // EOF implicit
    ]);
    let f = consume_a_function(&mut stream);
    assert_eq!(f.name, "foo");
    assert_eq!(f.value.len(), 1);
}

/// §5.5.7: With `;` as the stop token, the list ends when `;` is
/// encountered (without consuming the `;`).
#[test]
fn list_of_components_until_semicolon() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("a".to_string()),
        Token::Semicolon,
        Token::Ident("b".to_string()),
        Token::Eof,
    ]);
    let values = consume_a_list_of_component_values(&mut stream, Some(Token::Semicolon), false);
    assert_eq!(values.len(), 1);
    // Stream is now positioned at the `;`.
    assert!(matches!(stream.next_token(), Token::Semicolon));
}

/// §5.5.7 L2761-2764: With nested=true, an unbalanced `}-token`
/// returns the list without consuming the `}` (caller handles it).
#[test]
fn list_of_components_nested_close_brace_returns() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("a".to_string()),
        Token::CloseBrace,
        Token::Ident("b".to_string()),
        Token::Eof,
    ]);
    let values = consume_a_list_of_component_values(&mut stream, None, true);
    assert_eq!(values.len(), 1);
    // Stream is still positioned at `}` (not consumed).
    assert!(matches!(stream.next_token(), Token::CloseBrace));
}

/// §5.5.7 L2766-2769: With nested=false, an unbalanced `}-token` is a
/// parse error → consumed and appended to the list as a PreservedToken.
#[test]
fn list_of_components_top_level_close_brace_is_error() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("a".to_string()),
        Token::CloseBrace,
        Token::Ident("b".to_string()),
        Token::Eof,
    ]);
    let values = consume_a_list_of_component_values(&mut stream, None, false);
    assert_eq!(values.len(), 3);
    assert!(matches!(
        values[0],
        ComponentValue::PreservedToken(Token::Ident(_))
    ));
    assert!(matches!(
        values[1],
        ComponentValue::PreservedToken(Token::CloseBrace)
    ));
    assert!(matches!(
        values[2],
        ComponentValue::PreservedToken(Token::Ident(_))
    ));
}

/// §5.5.11: `"U+1234"` tokenized with `unicode_ranges_allowed=true`
/// produces a UnicodeRange token; consume_a_list_of_component_values
/// wraps it as a single PreservedToken.
#[test]
fn unicode_range_value_consumes_range_token() {
    let values = consume_a_unicode_range_value("U+1234");
    assert_eq!(values.len(), 1);
    assert!(matches!(
        values[0],
        ComponentValue::PreservedToken(Token::UnicodeRange(_, _))
    ));
}
