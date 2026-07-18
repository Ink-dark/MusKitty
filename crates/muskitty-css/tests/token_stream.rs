//! §5.3 Token Stream tests (CP-2).
//!
//! Verifies the 8 TokenStream operations defined per §5.3 L1766-1808.

use muskitty_css::parser::TokenStream;
use muskitty_css::tokenizer::Token;

/// §5.3 L1769-1773: `next_token` returns the first token at index 0.
#[test]
fn next_token_at_start() {
    let stream = TokenStream::new(vec![Token::Ident("a".to_string()), Token::Eof]);
    assert_eq!(stream.next_token(), Token::Ident("a".to_string()));
}

/// §5.3 L1769-1773 + L1811-1813: out-of-bounds index returns
/// `Token::Eof`.
#[test]
fn next_token_at_end_returns_eof() {
    let stream = TokenStream::new(vec![Token::Ident("a".to_string())]);
    // Move past the last token.
    let mut s = stream;
    s.index = 1;
    assert_eq!(s.next_token(), Token::Eof);
}

/// §5.3 L1779-1782: `consume_token` advances `index` and returns the
/// consumed token.
#[test]
fn consume_token_advances_index() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("a".to_string()),
        Token::Colon,
        Token::Eof,
    ]);
    assert_eq!(stream.consume_token(), Token::Ident("a".to_string()));
    assert_eq!(stream.index, 1);
    assert_eq!(stream.consume_token(), Token::Colon);
    assert_eq!(stream.index, 2);
}

/// §5.3 L1784-1786: `discard_token` at EOF must not panic and must not
/// move `index`.
#[test]
fn discard_token_at_eof_no_panic() {
    let mut stream = TokenStream::new(vec![Token::Ident("a".to_string())]);
    stream.index = 1; // point at EOF
    stream.discard_token();
    assert_eq!(stream.index, 1);
}

/// §5.3 L1788-1793: `mark` saves the current index; after consuming
/// several tokens, `restore_mark` returns the index to the saved
/// position.
#[test]
fn mark_and_restore_mark() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("a".to_string()),
        Token::Colon,
        Token::Ident("b".to_string()),
        Token::Eof,
    ]);
    stream.mark(); // save index 0
    stream.consume_token();
    stream.consume_token();
    assert_eq!(stream.index, 2);
    stream.restore_mark();
    assert_eq!(stream.index, 0);
}

/// §5.3 L1795-1797: `discard_mark` must NOT restore the index. A
/// subsequent `restore_mark` call (with no intervening `mark`) pops
/// the next saved mark on the stack — verify behavior on a stack of
/// two marks.
#[test]
fn discard_mark_does_not_restore() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("a".to_string()),
        Token::Colon,
        Token::Ident("b".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    stream.mark(); // mark index 0
    stream.consume_token(); // index 1
    stream.mark(); // mark index 1
    stream.consume_token(); // index 2
    stream.discard_mark(); // discard the index-1 mark, no restore
    assert_eq!(stream.index, 2);
    stream.restore_mark(); // restore to index 0 (the only remaining mark)
    assert_eq!(stream.index, 0);
}

/// §5.3 L1799-1801: `discard_whitespace` consumes a run of
/// consecutive whitespace tokens, stopping at the first non-ws
/// token.
#[test]
fn discard_whitespace_consumes_run() {
    let mut stream = TokenStream::new(vec![
        Token::Whitespace,
        Token::Whitespace,
        Token::Whitespace,
        Token::Ident("a".to_string()),
        Token::Eof,
    ]);
    stream.discard_whitespace();
    assert_eq!(stream.index, 3);
    assert_eq!(stream.next_token(), Token::Ident("a".to_string()));
}

/// §5.3 L1811-1813: The constructor implicitly appends an EOF token
/// if one is not already present.
#[test]
fn eof_implicit_appended() {
    let stream = TokenStream::new(vec![Token::Ident("a".to_string())]);
    // The tokens vec must now contain 2 entries: the ident + EOF.
    assert_eq!(stream.tokens.len(), 2);
    assert!(matches!(stream.tokens.last(), Some(Token::Eof)));
    // And if EOF was already present, no second EOF is appended.
    let stream2 = TokenStream::new(vec![Token::Ident("a".to_string()), Token::Eof]);
    assert_eq!(stream2.tokens.len(), 2);
}
