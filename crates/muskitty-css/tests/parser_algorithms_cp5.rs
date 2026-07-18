//! §5.5.1-§5.5.5 stylesheet/rule/block algorithm tests (CP-5).

use muskitty_css::parser::{
    consume_a_blocks_contents, consume_a_qualified_rule, consume_a_stylesheets_contents,
    consume_an_at_rule, BlockContents, ParseError, Rule, TokenStream,
};
use muskitty_css::tokenizer::Token;

/// §5.5.1: Empty input produces an empty rule list.
#[test]
fn stylesheet_empty_input() {
    let mut stream = TokenStream::new(vec![]);
    let rules = consume_a_stylesheets_contents(&mut stream);
    assert!(rules.is_empty());
}

/// §5.5.1: `a {} ` produces one QualifiedRule with prelude=[Ident("a")]
/// and empty declarations.
#[test]
fn stylesheet_single_rule() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("a".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::CloseBrace,
        Token::Eof,
    ]);
    let rules = consume_a_stylesheets_contents(&mut stream);
    assert_eq!(rules.len(), 1);
    match &rules[0] {
        Rule::QualifiedRule(q) => {
            assert_eq!(q.prelude.len(), 2); // Ident + Whitespace
            assert!(q.declarations.is_empty());
            assert!(q.child_rules.is_empty());
        }
        other => panic!("expected QualifiedRule, got {other:?}"),
    }
}

/// §5.5.1 + §5.5.2: `@import "x";` produces an AtRule with name="import",
/// prelude=[String("x")], declarations=None, child_rules=None.
#[test]
fn stylesheet_at_rule_statement() {
    let mut stream = TokenStream::new(vec![
        Token::AtKeyword("import".to_string()),
        Token::Whitespace,
        Token::String("x".to_string()),
        Token::Semicolon,
        Token::Eof,
    ]);
    let rules = consume_a_stylesheets_contents(&mut stream);
    assert_eq!(rules.len(), 1);
    match &rules[0] {
        Rule::AtRule(a) => {
            assert_eq!(a.name, "import");
            assert!(a.declarations.is_none());
            assert!(a.child_rules.is_none());
        }
        other => panic!("expected AtRule, got {other:?}"),
    }
}

/// §5.5.2: `@media print { a {} }` produces a block at-rule with one
/// child QualifiedRule.
#[test]
fn stylesheet_at_rule_block() {
    let mut stream = TokenStream::new(vec![
        Token::AtKeyword("media".to_string()),
        Token::Whitespace,
        Token::Ident("print".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::Ident("a".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::CloseBrace,
        Token::CloseBrace,
        Token::Eof,
    ]);
    let rules = consume_a_stylesheets_contents(&mut stream);
    assert_eq!(rules.len(), 1);
    match &rules[0] {
        Rule::AtRule(a) => {
            assert_eq!(a.name, "media");
            // Block at-rule: declarations and child_rules are both Some.
            assert!(a.declarations.is_some());
            let child_rules = a.child_rules.as_ref().expect("child_rules should be Some");
            assert_eq!(child_rules.len(), 1);
            assert!(matches!(&child_rules[0], Rule::QualifiedRule(_)));
        }
        other => panic!("expected AtRule, got {other:?}"),
    }
}

/// §5.5.1: CDO (`<!--`) and CDC (`-->`) tokens are discarded at the
/// stylesheet top level.
#[test]
fn stylesheet_cdo_cdc_discarded() {
    let mut stream = TokenStream::new(vec![
        Token::Cdo,
        Token::Ident("a".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::CloseBrace,
        Token::Cdc,
        Token::Eof,
    ]);
    let rules = consume_a_stylesheets_contents(&mut stream);
    assert_eq!(rules.len(), 1);
    assert!(matches!(&rules[0], Rule::QualifiedRule(_)));
}

/// §5.5.2: When `nested=true`, encountering `}` returns the at-rule
/// without consuming the `}`.
#[test]
fn at_rule_nested_close_brace_returns() {
    let mut stream = TokenStream::new(vec![
        Token::AtKeyword("media".to_string()),
        Token::CloseBrace,
        Token::Eof,
    ]);
    let rule = consume_an_at_rule(&mut stream, true).expect("at-rule should be returned");
    assert_eq!(rule.name, "media");
    // The `}` was not consumed.
    assert!(matches!(stream.next_token(), Token::CloseBrace));
}

/// §5.5.3 L2377-2383: A qualified rule whose prelude looks like
/// `--foo:` (custom property) at nested=true → consume remnants +
/// return `Ok(None)`.
#[test]
fn qualified_rule_custom_property_in_prelude_nested() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("--foo".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("hover".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::CloseBrace,
        Token::Eof,
    ]);
    let result = consume_a_qualified_rule(&mut stream, None, true);
    assert!(matches!(result, Ok(None)));
}

/// §5.5.3 L2377-2383: A qualified rule whose prelude looks like
/// `--foo:` at top level (nested=false) → consume the block + return
/// `Err(ParseError)` (invalid rule error).
#[test]
fn qualified_rule_custom_property_in_prelude_top_level() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("--foo".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("hover".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::CloseBrace,
        Token::Eof,
    ]);
    let result = consume_a_qualified_rule(&mut stream, None, false);
    assert!(matches!(result, Err(ParseError)));
}

/// §5.5.5: A block containing `color: red; a {}` produces a
/// declarations run followed by a QualifiedRule.
#[test]
fn block_contents_mixed_decls_and_rules() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::Whitespace,
        Token::Ident("a".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::CloseBrace,
        Token::Eof,
    ]);
    let bc: BlockContents = consume_a_blocks_contents(&mut stream);
    assert_eq!(bc.rules.len(), 2);
    assert!(matches!(&bc.rules[0], Rule::Declarations(_)));
    if let Rule::Declarations(decls) = &bc.rules[0] {
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].name, "color");
    }
    assert!(matches!(&bc.rules[1], Rule::QualifiedRule(_)));
}

/// §5.5.5 L2519-2528: An at-keyword flushes any pending declarations
/// into the rules list before consuming the at-rule.
#[test]
fn block_contents_at_rule_flushes_decls() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::Whitespace,
        Token::AtKeyword("media".to_string()),
        Token::Whitespace,
        Token::Ident("print".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::CloseBrace,
        Token::Eof,
    ]);
    let bc = consume_a_blocks_contents(&mut stream);
    // Expect 2 rules: Declarations([color]) + AtRule(media).
    assert_eq!(bc.rules.len(), 2);
    assert!(matches!(&bc.rules[0], Rule::Declarations(_)));
    assert!(matches!(&bc.rules[1], Rule::AtRule(_)));
}

/// §5.5.5 L2530-2562: When consume_a_declaration returns None, the mark
/// is restored and consume_a_qualified_rule is tried. If that returns
/// a rule, decls (empty here) are flushed (no-op) and the rule is
/// appended.
#[test]
fn block_contents_invalid_decl_then_rule_restores_mark() {
    // `font+ 1; a {}` — `font+ 1` is not a valid declaration (no colon),
    // so consume_a_declaration will fail, then consume_a_qualified_rule
    // is tried with stop=`;`. `font+ 1` (without a `{`) → consumes to
    // `;` (stop_token) → Ok(None) → do nothing. Then `a {}` → rule.
    let mut stream = TokenStream::new(vec![
        Token::Ident("font".to_string()),
        Token::Delim('+'),
        Token::Whitespace,
        Token::Number(muskitty_css::tokenizer::Numeric {
            value: 1.0,
            is_integer: true,
        }),
        Token::Semicolon,
        Token::Whitespace,
        Token::Ident("a".to_string()),
        Token::Whitespace,
        Token::OpenBrace,
        Token::CloseBrace,
        Token::Eof,
    ]);
    let bc = consume_a_blocks_contents(&mut stream);
    // We expect just the QualifiedRule "a" — the `font+ 1` was a
    // failed declaration followed by a failed qualified rule, both
    // leaving nothing.
    assert_eq!(bc.rules.len(), 1);
    assert!(matches!(&bc.rules[0], Rule::QualifiedRule(_)));
}

/// §5.5.5: A block containing only declarations produces a single
/// `Rule::Declarations` entry.
#[test]
fn block_contents_only_decls() {
    let mut stream = TokenStream::new(vec![
        Token::Ident("color".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Ident("red".to_string()),
        Token::Semicolon,
        Token::Whitespace,
        Token::Ident("font".to_string()),
        Token::Colon,
        Token::Whitespace,
        Token::Dimension(
            muskitty_css::tokenizer::Numeric {
                value: 16.0,
                is_integer: true,
            },
            "px".to_string(),
        ),
        Token::Semicolon,
        Token::Eof,
    ]);
    let bc = consume_a_blocks_contents(&mut stream);
    assert_eq!(bc.rules.len(), 1);
    if let Rule::Declarations(decls) = &bc.rules[0] {
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0].name, "color");
        assert_eq!(decls[1].name, "font");
    } else {
        panic!("expected Rule::Declarations, got {:?}", bc.rules[0]);
    }
}
