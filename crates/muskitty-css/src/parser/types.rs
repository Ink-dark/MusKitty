//! CSS Parsing Results data structures (§5.2).
//!
//! Per §5.2 L1625-1721 of CSS Syntax Module Level 3: the result of
//! parsing can be a stylesheet, a rule (at-rule or qualified rule), a
//! declaration, or a component value (preserved token, function, or
//! simple block).

use crate::tokenizer::Token;

/// §5.2 L1632-1633: A stylesheet has a list of rules.
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

/// §5.2 L1635-1637: A rule is either an at-rule or a qualified rule.
///
/// Also carries a `Declarations` variant (§5.5.5 L2492-2498) for the
/// mixed list output of `consume_a_blocks_contents` — a block's
/// contents can be a sequence of declarations interleaved with rules,
/// and we model each declaration-list run as a `Rule::Declarations`.
#[derive(Debug, Clone)]
pub enum Rule {
    AtRule(AtRule),
    QualifiedRule(QualifiedRule),
    /// §5.5.5: a list of declarations (a "run" of consecutive
    /// declarations inside a block, before the next child rule). At
    /// the CSSOM boundary this gets materialized as either
    /// `CSSStyleDeclaration` or `CSSNestedDeclarations`.
    Declarations(Vec<Declaration>),
}

/// §5.2 L1639-1650: An at-rule has a name, a prelude (list of component
/// values), and optionally a list of declarations and a list of child
/// rules (only for "block at-rules" ending in a {}-block).
#[derive(Debug, Clone)]
pub struct AtRule {
    /// The at-rule name (e.g. "media", "import"). Does not include the
    /// leading `@`.
    pub name: String,
    /// The prelude: component values between the name and the block
    /// or semicolon.
    pub prelude: Vec<ComponentValue>,
    /// Block at-rules only: declarations inside the {}-block. `None`
    /// for statement at-rules (ending in `;`).
    pub declarations: Option<Vec<Declaration>>,
    /// Block at-rules only: child rules inside the {}-block. `None`
    /// for statement at-rules.
    pub child_rules: Option<Vec<Rule>>,
}

/// §5.2 L1652-1657: A qualified rule has a prelude, declarations, and
/// child rules.
#[derive(Debug, Clone)]
pub struct QualifiedRule {
    /// The prelude (e.g. a selector for style rules).
    pub prelude: Vec<ComponentValue>,
    /// Declarations inside the {}-block.
    pub declarations: Vec<Declaration>,
    /// Child rules inside the {}-block (for nested rules like
    /// `@media`).
    pub child_rules: Vec<Rule>,
}

/// §5.2 L1663-1669: A declaration has a name, a value (list of
/// component values), an `important` flag, and an optional
/// `original_text`.
#[derive(Debug, Clone)]
pub struct Declaration {
    /// The property or descriptor name (e.g. "color",
    /// "font-family").
    pub name: String,
    /// The value: component values between `:` and `;` (or end of
    /// block).
    pub value: Vec<ComponentValue>,
    /// Whether the declaration had an `!important` flag.
    pub important: bool,
    /// §5.2 L1668-1669 + §5.5.6 L2693-2698: only set for custom
    /// property declarations (`--foo: ...`), to allow var()
    /// resolution to access the original source text.
    pub original_text: Option<String>,
}

/// §5.2 L1681-1685: A component value is one of the preserved tokens,
/// a function, or a simple block.
#[derive(Debug, Clone)]
pub enum ComponentValue {
    /// §5.2 L1687-1703: A preserved token (any token except
    /// function-token, `{-token`, `(-token`, `[-token` — those are
    /// always consumed into higher-level objects).
    PreservedToken(Token),
    /// §5.2 L1705-1708: A function.
    Function(Function),
    /// §5.2 L1710-1719: A simple block.
    SimpleBlock(SimpleBlock),
}

/// §5.2 L1705-1708: A function has a name and a value (list of
/// component values).
#[derive(Debug, Clone)]
pub struct Function {
    /// The function name (e.g. "translate", "var"). Does not include
    /// the leading ident or the trailing `(`.
    pub name: String,
    /// The arguments: component values between `(` and `)`.
    pub value: Vec<ComponentValue>,
}

/// §5.2 L1710-1719: A simple block has an associated token (the
/// opening token) and a value (list of component values).
#[derive(Debug, Clone)]
pub struct SimpleBlock {
    /// Which kind of block this is: `{}`, `[]`, or `()`.
    pub kind: BlockKind,
    /// The component values inside the block.
    pub value: Vec<ComponentValue>,
}

/// §5.2 L1710-1719: The associated token kind of a simple block,
/// mirroring the opening `{` / `[` / `(`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// `{}-block` (L1718): opens with `{-token`, closes with
    /// `}-token`.
    Curly,
    /// `[]-block` (L1718): opens with `[-token`, closes with
    /// `]-token`.
    Square,
    /// `()-block` (L1718): opens with `(-token`, closes with
    /// `)-token`.
    Paren,
}
