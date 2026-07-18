//! §5.5.6 Declaration algorithm tests (CP-4).

use muskitty_css::parser::{consume_a_declaration, ComponentValue, TokenStream};
use muskitty_css::tokenizer::Token;

/// §5.5.6: `color: red;` parses to a Declaration with name="color",
/// value=[PreservedToken(Ident("red"))], important=false.
#[test]
fn declaration_basic() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    let decl = consume_a_declaration(&mut stream, false).expect("declaration should parse");
    assert_eq!(decl.name, "color");
    assert!(!decl.important);
    assert_eq!(decl.value.len(), 1);
    assert!(matches!(
        &decl.value[0],
        ComponentValue::PreservedToken(Token::Ident(s)) if s == "red"
    ));
}

/// §5.5.6: Without a trailing `;`, EOF still terminates the value list.
#[test]
fn declaration_without_semicolon_at_eof() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        // EOF implicit
    ]);
    let decl = consume_a_declaration(&mut stream, false).expect("declaration should parse");
    assert_eq!(decl.name, "color");
    assert_eq!(decl.value.len(), 1);
}

/// §5.5.6: Missing colon → bad declaration → remnants consumes through
/// `;` and returns `None`.
#[test]
fn declaration_no_colon_returns_none() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    assert!(consume_a_declaration(&mut stream, false).is_none());
    // Stream should now be at EOF (remnants consumed everything through `;`).
    assert!(matches!(stream.next_token(), Token::Eof));
}

/// §5.5.6: Missing leading ident → bad declaration → remnants consumes
/// through `;` and returns `None`.
#[test]
fn declaration_no_ident_returns_none() {
    let mut stream = TokenStream::new(vec![
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    assert!(consume_a_declaration(&mut stream, false).is_none());
    assert!(matches!(stream.next_token(), Token::Eof));
}

/// §5.5.6 step 6: `color: red !important;` strips the `!important` and
/// sets the `important` flag.
#[test]
fn declaration_important_flag() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Whitespace,
        Token::Delim('!'),
        Token::Ident("important".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    let decl = consume_a_declaration(&mut stream, false).expect("declaration should parse");
    assert!(decl.important);
    // The value should contain only `red` (after stripping `!important`
    // and trailing whitespace).
    assert_eq!(decl.value.len(), 1);
    assert!(matches!(
        &decl.value[0],
        ComponentValue::PreservedToken(Token::Ident(s)) if s == "red"
    ));
}

/// §5.5.6 step 6: `!important` is ASCII case-insensitive.
#[test]
fn declaration_important_case_insensitive() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Whitespace,
        Token::Delim('!'),
        Token::Ident("IMPORTANT".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    let decl = consume_a_declaration(&mut stream, false).expect("declaration should parse");
    assert!(decl.important);
}

/// §5.5.6 step 8: A custom property name (`--foo`) is accepted; the
/// value is preserved as-is (no top-level {}-block validity check for
/// custom properties).
#[test]
fn declaration_custom_property() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("--foo".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("bar".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    let decl = consume_a_declaration(&mut stream, false).expect("custom property should parse");
    assert_eq!(decl.name, "--foo");
    assert_eq!(decl.value.len(), 1);
    assert!(matches!(
        &decl.value[0],
        ComponentValue::PreservedToken(Token::Ident(s)) if s == "bar"
    ));
}

/// §5.5.6 step 8: A non-custom property value that contains a top-level
/// {}-block AND any other non-whitespace value is invalid (returns
/// `None`). `color: {} red;` is such a case.
#[test]
fn declaration_top_level_curly_with_other_returns_none() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::OpenBrace,
        Token::CloseBrace,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    assert!(consume_a_declaration(&mut stream, false).is_none());
}
