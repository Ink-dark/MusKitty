//! §5.4 Parser Entry Points tests (CP-6).
//!
//! Verifies the 9 implemented entry points (§5.4.3-§5.4.10). The
//! grammar-based hooks §5.4.1 / §5.4.2 are deferred.

use muskitty_css::parser::{
    parse_a_blocks_contents, parse_a_comma_separated_list_of_component_values,
    parse_a_component_value, parse_a_declaration, parse_a_list_of_component_values, parse_a_rule,
    parse_a_stylesheet, parse_a_stylesheets_contents,
};
use muskitty_css::tokenizer::Token;

/// §5.4.3: `"a { color: red; }"` → Stylesheet with one QualifiedRule.
#[test]
fn parse_a_stylesheet_simple() {
    let ss = parse_a_stylesheet("a { color: red; }");
    assert_eq!(ss.rules.len(), 1);
    assert!(matches!(
        ss.rules[0],
        muskitty_css::parser::Rule::QualifiedRule(_)
    ));
}

/// §5.4.4: `"a {} b {}"` → two QualifiedRules.
#[test]
fn parse_a_stylesheets_contents_returns_vec() {
    let rules = parse_a_stylesheets_contents("a {} b {}");
    assert_eq!(rules.len(), 2);
    for r in &rules {
        assert!(matches!(r, muskitty_css::parser::Rule::QualifiedRule(_)));
    }
}

/// §5.4.5: `"color: red; font: 16px;"` → BlockContents with 2 decls
/// (flushed to rules as Rule::Declarations per §5.5.5), no child rules.
#[test]
fn parse_a_blocks_contents_basic() {
    let bc = parse_a_blocks_contents("color: red; font: 16px;");
    // §5.5.5: declarations always flushed as Rule::Declarations on
    // return. So `decls` should be empty, and `rules` should contain
    // exactly one Declarations variant.
    assert!(bc.decls.is_empty());
    assert_eq!(bc.rules.len(), 1);
    match &bc.rules[0] {
        muskitty_css::parser::Rule::Declarations(decls) => {
            assert_eq!(decls.len(), 2);
            assert_eq!(decls[0].name, "color");
            assert_eq!(decls[1].name, "font");
        }
        other => panic!("expected Rule::Declarations, got {other:?}"),
    }
}

/// §5.4.6: `"@media print {}"` → Some(AtRule).
#[test]
fn parse_a_rule_at_rule() {
    let rule = parse_a_rule("@media print {}");
    assert!(matches!(rule, Some(muskitty_css::parser::Rule::AtRule(_))));
    if let Some(muskitty_css::parser::Rule::AtRule(at)) = rule {
        assert_eq!(at.name, "media");
        assert!(at.declarations.is_some() || at.child_rules.is_some());
    }
}

/// §5.4.6: `"a {}"` → Some(QualifiedRule).
#[test]
fn parse_a_rule_qualified_rule() {
    let rule = parse_a_rule("a {}");
    assert!(matches!(
        rule,
        Some(muskitty_css::parser::Rule::QualifiedRule(_))
    ));
}

/// §5.4.6: `""` → None (EOF after whitespace).
#[test]
fn parse_a_rule_eof_returns_none() {
    let rule = parse_a_rule("");
    assert!(rule.is_none());
}

/// §5.4.6: `"a {} b"` → None (after consuming `a {}` there's trailing
/// `b`, so the "if next token is EOF" check fails → syntax error).
#[test]
fn parse_a_rule_trailing_garbage_returns_none() {
    let rule = parse_a_rule("a {} b");
    assert!(rule.is_none());
}

/// §5.4.7: `"color: red"` → Some(Declaration{name:"color"}).
#[test]
fn parse_a_declaration_simple() {
    let decl = parse_a_declaration("color: red");
    assert!(decl.is_some());
    let decl = decl.unwrap();
    assert_eq!(decl.name, "color");
    assert!(!decl.important);
}

/// §5.4.8: `"red"` → Some(ComponentValue::PreservedToken(Ident("red"))).
#[test]
fn parse_a_component_value_simple() {
    let cv = parse_a_component_value("red");
    assert!(cv.is_some());
    match cv.unwrap() {
        muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(name)) => {
            assert_eq!(name, "red");
        }
        other => panic!("expected PreservedToken(Ident(\"red\")), got {other:?}"),
    }
}

/// §5.4.10: `"a, b, c"` → three groups. Per §5.4.10 the algorithm
/// does NOT discard whitespace, so the second and third groups
/// contain a leading whitespace token followed by the ident:
/// `[[a], [ws, b], [ws, c]]`.
#[test]
fn parse_a_comma_separated_list_basic() {
    let groups = parse_a_comma_separated_list_of_component_values("a, b, c");
    assert_eq!(groups.len(), 3);

    // Group 0: "a" (no leading whitespace — first token after
    // normalize).
    assert_eq!(groups[0].len(), 1);
    assert!(matches!(
        groups[0][0],
        muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(_))
    ));

    // Groups 1 & 2: leading whitespace + ident.
    for (i, g) in groups[1..].iter().enumerate() {
        assert_eq!(g.len(), 2, "group {} should have ws + ident", i + 1);
        assert!(matches!(
            g[0],
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Whitespace)
        ));
        match &g[1] {
            muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(name)) => {
                let expected = match i {
                    0 => "b",
                    1 => "c",
                    _ => unreachable!(),
                };
                assert_eq!(name, expected);
            }
            other => panic!("group {}: expected Ident, got {other:?}", i + 1),
        }
    }
}

/// §5.4.9: `"a b c"` → 3-element list of PreservedTokens (with
/// whitespace tokens interspersed → 5 elements total).
#[test]
fn parse_a_list_of_component_values_basic() {
    let list = parse_a_list_of_component_values("a b c");
    // a, ws, b, ws, c
    assert_eq!(list.len(), 5);
    assert!(matches!(
        list[0],
        muskitty_css::parser::ComponentValue::PreservedToken(Token::Ident(_))
    ));
}
