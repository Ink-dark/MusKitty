//! §5.2 CSS Parsing Results data structure tests (CP-1).
//!
//! Verifies basic construction, `Default` impls, and enum variant
//! structure for the parser's data types.

use muskitty_css::parser::{AtRule, Declaration, QualifiedRule, Rule, Stylesheet};

/// `Stylesheet::default()` produces an empty rule list (§5.2 L1632-1633).
#[test]
fn stylesheet_default_empty() {
    let ss = Stylesheet::default();
    assert!(ss.rules.is_empty());
}

/// `Rule::AtRule(...)` variant constructs correctly (§5.2 L1635-1637).
#[test]
fn rule_at_rule_variant() {
    let at = AtRule {
        name: "import".to_string(),
        prelude: Vec::new(),
        declarations: None,
        child_rules: None,
    };
    let rule = Rule::AtRule(at);
    assert!(matches!(rule, Rule::AtRule(_)));
    if let Rule::AtRule(a) = rule {
        assert_eq!(a.name, "import");
        assert!(a.declarations.is_none());
        assert!(a.child_rules.is_none());
    }
}

/// `Rule::QualifiedRule(...)` variant constructs correctly (§5.2 L1635-1637).
#[test]
fn rule_qualified_rule_variant() {
    let qr = QualifiedRule {
        prelude: Vec::new(),
        declarations: Vec::new(),
        child_rules: Vec::new(),
    };
    let rule = Rule::QualifiedRule(qr);
    assert!(matches!(rule, Rule::QualifiedRule(_)));
    if let Rule::QualifiedRule(q) = rule {
        assert!(q.prelude.is_empty());
        assert!(q.declarations.is_empty());
        assert!(q.child_rules.is_empty());
    }
}

/// `Rule::Declarations(vec![])` variant — placeholder for §5.5.5 mixed
/// list output. Construction must work even with empty declaration runs.
#[test]
fn rule_declarations_variant() {
    let rule = Rule::Declarations(Vec::new());
    assert!(matches!(rule, Rule::Declarations(_)));
    if let Rule::Declarations(decls) = rule {
        assert!(decls.is_empty());
    }
}

/// §5.2 L1639-1650: An at-rule can be either a "statement" form (no
/// block, declarations/rules both `None`) or a "block" form (with
/// `Some(Vec)` for both). Both must be representable.
#[test]
fn at_rule_statement_vs_block() {
    // Statement at-rule: ends in `;`, no block.
    let stmt = AtRule {
        name: "import".to_string(),
        prelude: Vec::new(),
        declarations: None,
        child_rules: None,
    };
    assert!(stmt.declarations.is_none());
    assert!(stmt.child_rules.is_none());

    // Block at-rule: ends in `{...}`, has declarations and rules.
    let block = AtRule {
        name: "media".to_string(),
        prelude: Vec::new(),
        declarations: Some(Vec::new()),
        child_rules: Some(Vec::new()),
    };
    assert!(block.declarations.is_some());
    assert!(block.child_rules.is_some());
}

/// §5.2 L1663-1669: The `important` flag on a declaration can be set.
#[test]
fn declaration_important_flag() {
    let decl = Declaration {
        name: "color".to_string(),
        value: Vec::new(),
        important: true,
        original_text: None,
    };
    assert!(decl.important);
    assert_eq!(decl.name, "color");
    assert!(decl.value.is_empty());
}
