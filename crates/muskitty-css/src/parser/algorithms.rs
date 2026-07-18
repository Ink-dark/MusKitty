//! §5.5 Parser Algorithms.
//!
//! Implementation of the 11 algorithms defined in CSS Syntax Module
//! Level 3 §5.5. This module covers:
//! - CP-3 (lower-level algorithms): §5.5.7-§5.5.11.
//! - CP-4 (declaration algorithms): §5.5.6.
//! - CP-5 (stylesheet/rule/block algorithms): §5.5.1-§5.5.5.

use super::token_stream::TokenStream;
use super::types::{
    AtRule, BlockKind, ComponentValue, Declaration, Function, ParseError, QualifiedRule, Rule,
    SimpleBlock,
};
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

// ─── §5.5.6 Declaration algorithms ───────────────────────────────────

/// §5.5.6 (L2639-2717) Consume a declaration.
///
/// `nested`: when true, an unbalanced `}-token` returns `None` without
/// consuming (caller will handle the closing brace); when false, it
/// is consumed as part of the declaration's value.
///
/// # Algorithm steps
///
/// 1. If next token is ident, consume it as the declaration name.
///    Otherwise, consume remnants of a bad declaration, return `None`.
/// 2. Discard whitespace.
/// 3. If next token is `:`, discard it. Otherwise, bad declaration,
///    return `None`.
/// 4. Discard whitespace.
/// 5. Consume a list of component values with `;` as stop token.
/// 6. If last two non-whitespace values are `!` + `important` (ASCII
///    case-insensitive), remove them and set `important` flag.
/// 7. Strip trailing whitespace tokens.
/// 8. Custom property (`--foo`): set `original_text` (deferred —
///    requires source-text tracking in `TokenStream`). Otherwise,
///    if value contains a top-level {}-block AND any other non-ws
///    value, return `None` (only the whole value may be a {}-block
///    for non-custom properties).
/// 9. (§5.5.6 L2707-2712: `unicode-range` descriptor handling
///    deferred — requires source-text tracking.)
pub fn consume_a_declaration(input: &mut TokenStream, nested: bool) -> Option<Declaration> {
    // Step 1 (§5.5.6 L2650-2659): ident-token as declaration name.
    let name = match input.next_token() {
        Token::Ident(name) => {
            input.consume_token();
            name
        }
        _ => {
            consume_the_remnants_of_a_bad_declaration(input, nested);
            return None;
        }
    };

    // Step 2 (§5.5.6 L2661-2662): discard whitespace.
    input.discard_whitespace();

    // Step 3 (§5.5.6 L2664-2671): colon.
    match input.next_token() {
        Token::Colon => input.discard_token(),
        _ => {
            consume_the_remnants_of_a_bad_declaration(input, nested);
            return None;
        }
    }

    // Step 4 (§5.5.6 L2673-2674): discard whitespace.
    input.discard_whitespace();

    // Step 5 (§5.5.6 L2676-2680): consume value until `;`.
    let mut value = consume_a_list_of_component_values(input, Some(Token::Semicolon), nested);

    // Step 6 (§5.5.6 L2682-2687): strip !important from the tail.
    let important = strip_important(&mut value);

    // Step 7 (§5.5.6 L2689-2691): strip trailing whitespace.
    while matches!(
        value.last(),
        Some(ComponentValue::PreservedToken(Token::Whitespace))
    ) {
        value.pop();
    }

    let decl = Declaration {
        name,
        value,
        important,
        original_text: None,
    };

    // Step 8 (§5.5.6 L2693-2705): custom property original_text +
    // top-level {}-block validity check.
    if is_custom_property_name(&decl.name) {
        // §5.5.6 L2693-2698: original_text should be set. Deferred —
        // requires `TokenStream` to retain original source text. For
        // now, leave `None`.
    } else {
        // §5.5.6 L2700-2705: if value contains a top-level {}-block
        // AND any other non-whitespace value, return nothing.
        if has_top_level_curly_block_with_other_values(&decl.value) {
            return None;
        }
    }

    // §5.5.6 L2707-2712: `unicode-range` descriptor handling deferred
    // (requires source-text tracking for re-tokenization).

    // §5.5.6 L2714-2717: if the declaration is valid, return it;
    // otherwise return nothing. We treat "valid" as "we successfully
    // parsed a name + colon + value", which is the syntactic validity.
    // Grammar-level validity is left to higher-level callers (CSSOM).
    Some(decl)
}

/// §5.5.6 (L2721-2741) Consume the remnants of a bad declaration.
///
/// Repeatedly process input:
/// - EOF or `;` → discard, return.
/// - `}-token`: if `nested`, return without consuming; else discard.
/// - anything else → consume a component value (discard result).
pub fn consume_the_remnants_of_a_bad_declaration(input: &mut TokenStream, nested: bool) {
    loop {
        match input.next_token() {
            Token::Eof | Token::Semicolon => {
                input.discard_token();
                return;
            }
            Token::CloseBrace => {
                if nested {
                    return;
                }
                input.discard_token();
            }
            _ => {
                // Discard the consumed component value.
                let _ = consume_a_component_value(input);
            }
        }
    }
}

/// §5.5.6 step 6 (L2682-2687): If the last two non-whitespace values are
/// a `!` delim token followed by an `important` ident (ASCII
/// case-insensitive), remove them (and any trailing whitespace
/// between/after) and return `true`. Otherwise return `false`.
///
/// This does NOT strip trailing whitespace after the `!important` —
/// that is the caller's responsibility (step 7).
fn strip_important(value: &mut Vec<ComponentValue>) -> bool {
    // Find the index just past the last non-whitespace value.
    let mut end = value.len();
    while end > 0
        && matches!(
            value[end - 1],
            ComponentValue::PreservedToken(Token::Whitespace)
        )
    {
        end -= 1;
    }
    if end < 2 {
        return false;
    }
    let last = &value[end - 1];
    let prev = &value[end - 2];
    let is_important_ident = |v: &ComponentValue| match v {
        ComponentValue::PreservedToken(Token::Ident(s)) => s.eq_ignore_ascii_case("important"),
        _ => false,
    };
    let is_bang_delim =
        |v: &ComponentValue| matches!(v, ComponentValue::PreservedToken(Token::Delim('!')));
    if is_bang_delim(prev) && is_important_ident(last) {
        // Truncate to `end - 2`, removing the `!important` (and any
        // trailing whitespace after it).
        value.truncate(end - 2);
        return true;
    }
    false
}

/// §5.5.6 L2693: A "custom property name string" is an ident-token
/// whose value starts with `--` (two hyphens).
fn is_custom_property_name(name: &str) -> bool {
    name.starts_with("--")
}

/// §5.5.6 L2700-2705: A declaration value contains a top-level
/// {}-block AND any other non-whitespace value. (Only the whole
/// value may be a {}-block for non-custom properties.)
fn has_top_level_curly_block_with_other_values(value: &[ComponentValue]) -> bool {
    let mut has_curly_block = false;
    let mut has_other = false;
    for v in value {
        match v {
            ComponentValue::SimpleBlock(SimpleBlock {
                kind: BlockKind::Curly,
                ..
            }) => {
                has_curly_block = true;
            }
            ComponentValue::PreservedToken(Token::Whitespace) => {}
            _ => {
                has_other = true;
            }
        }
    }
    has_curly_block && has_other
}

// ─── §5.5.1-§5.5.5 Stylesheet/rule/block algorithms ──────────────────

/// §5.5.1 (L2223-2279) Consume a stylesheet's contents.
///
/// Repeatedly process input:
/// - whitespace / CDO / CDC → discard.
/// - EOF → return rules.
/// - at-keyword → consume_an_at_rule; if `Some`, append.
/// - else → consume_a_qualified_rule; if `Some(rule)`, append.
pub fn consume_a_stylesheets_contents(input: &mut TokenStream) -> Vec<Rule> {
    let mut rules = Vec::new();
    loop {
        match input.next_token() {
            Token::Whitespace | Token::Cdo | Token::Cdc => {
                input.discard_token();
            }
            Token::Eof => return rules,
            Token::AtKeyword(_) => {
                if let Some(rule) = consume_an_at_rule(input, false) {
                    rules.push(Rule::AtRule(rule));
                }
            }
            _ => {
                if let Ok(Some(rule)) = consume_a_qualified_rule(input, None, false) {
                    rules.push(Rule::QualifiedRule(rule));
                }
            }
        }
    }
}

/// §5.5.2 (L2281-2337) Consume an at-rule.
///
/// `nested`: when true, an unbalanced `}-token` returns the rule
/// without consuming (caller will close the block); when false,
/// `}-token` is part of the prelude.
pub fn consume_an_at_rule(input: &mut TokenStream, nested: bool) -> Option<AtRule> {
    let name = match input.consume_token() {
        Token::AtKeyword(name) => name,
        _ => unreachable!("consume_an_at_rule called on non-at-keyword"),
    };
    let mut rule = AtRule {
        name,
        prelude: Vec::new(),
        declarations: None,
        child_rules: None,
    };
    loop {
        match input.next_token() {
            Token::Semicolon | Token::Eof => {
                input.discard_token();
                return Some(rule);
            }
            Token::CloseBrace => {
                if nested {
                    // §5.5.2 L2307-2310: nested → return rule (caller
                    // handles the closing brace).
                    return Some(rule);
                }
                // §5.5.2 L2313-2316: non-nested → consume to prelude
                // (parse error, but tolerated).
                let t = input.consume_token();
                rule.prelude.push(ComponentValue::PreservedToken(t));
            }
            Token::OpenBrace => {
                // §5.5.2 L2317-2331: consume_a_block → split into
                // declarations + child rules.
                let block = consume_a_block(input);
                let (decls, rules) = split_block_contents(block);
                rule.declarations = Some(decls);
                rule.child_rules = Some(rules);
                return Some(rule);
            }
            _ => {
                // §5.5.2 L2333-2336: anything else → consume a
                // component value, append to prelude.
                rule.prelude.push(consume_a_component_value(input));
            }
        }
    }
}

/// §5.5.3 (L2340-2466) Consume a qualified rule.
///
/// `stop_token`: optional token that aborts (returns `Ok(None)`).
/// `nested`: passed down to consume_a_block's contents (controls
/// `}-token` handling in nested contexts).
///
/// # Return values
///
/// - `Ok(Some(rule))` — rule successfully consumed.
/// - `Ok(None)` — "return nothing" (e.g. EOF or stop_token).
/// - `Err(ParseError)` — "invalid rule error" (e.g. custom-property-in-prelude
///   at top level after consuming the block).
pub fn consume_a_qualified_rule(
    input: &mut TokenStream,
    stop_token: Option<Token>,
    nested: bool,
) -> Result<Option<QualifiedRule>, ParseError> {
    let mut rule = QualifiedRule {
        prelude: Vec::new(),
        declarations: Vec::new(),
        child_rules: Vec::new(),
    };
    loop {
        let next = input.next_token();
        // §5.5.3 L2355-2359: EOF or stop_token → parse error, return
        // nothing.
        if matches!(next, Token::Eof) {
            return Ok(None);
        }
        if stop_token.as_ref().is_some_and(|s| *s == next) {
            return Ok(None);
        }
        match next {
            Token::CloseBrace => {
                // §5.5.3 L2361-2368: parse error.
                if nested {
                    return Ok(None);
                }
                let t = input.consume_token();
                rule.prelude.push(ComponentValue::PreservedToken(t));
            }
            Token::OpenBrace => {
                // §5.5.3 L2370-2460.
                // §5.5.3 L2372-2383: check if prelude looks like
                // `--<ident> :` (custom-property-like). If so:
                //   nested → consume remnants of bad declaration, return
                //           nothing.
                //   non-nested → consume a block, return invalid rule
                //                error.
                if looks_like_custom_property_in_prelude(&rule.prelude) {
                    if nested {
                        consume_the_remnants_of_a_bad_declaration(input, true);
                        return Ok(None);
                    } else {
                        let _ = consume_a_block(input);
                        return Err(ParseError); // invalid rule error
                    }
                }
                let block = consume_a_block(input);
                let (decls, rules) = split_block_contents(block);
                rule.declarations = decls;
                rule.child_rules = rules;
                return Ok(Some(rule));
            }
            _ => {
                // §5.5.3 L2462-2465: anything else → consume a
                // component value, append to prelude.
                rule.prelude.push(consume_a_component_value(input));
            }
        }
    }
}

/// §5.5.4 (L2469-2484) Consume a block.
///
/// Precondition: next token is `{-token`. Discard it, consume block
/// contents, discard `}-token (or EOF), return the contents.
pub fn consume_a_block(input: &mut TokenStream) -> BlockContents {
    debug_assert!(matches!(input.next_token(), Token::OpenBrace));
    input.discard_token();
    let contents = consume_a_blocks_contents(input);
    // §5.5.4 L2481: discard the closing `}-token (or EOF if implicit).
    input.discard_token();
    contents
}

/// §5.5.5 (L2486-2636) Consume a block's contents.
///
/// Returns a list of rules and lists-of-declarations (modeled as
/// [`BlockContents`] carrying `Vec<Declaration>` and `Vec<Rule>`).
/// Algorithm:
/// - whitespace / `;` → discard.
/// - EOF / `}` → return.
/// - at-keyword → flush decls into rules (as `Rule::Declarations`),
///   then `consume_an_at_rule(nested=true)`.
/// - else → mark; `consume_a_declaration(nested=true)`; if
///   `Some(decl)`, append to decls and discard mark. Otherwise
///   `restore_mark`, then `consume_a_qualified_rule(nested=true,
///   stop=`;`)`; on `Ok(Some(rule))`, flush decls to rules, append
///   rule; on `Err(())`, flush decls to rules; on `Ok(None)`, do
///   nothing.
pub fn consume_a_blocks_contents(input: &mut TokenStream) -> BlockContents {
    let mut rules: Vec<Rule> = Vec::new();
    let mut decls: Vec<Declaration> = Vec::new();
    loop {
        match input.next_token() {
            Token::Whitespace | Token::Semicolon => {
                input.discard_token();
            }
            Token::Eof | Token::CloseBrace => {
                // §5.5.5 L2514-2517: end of block — flush decls.
                if !decls.is_empty() {
                    rules.push(Rule::Declarations(std::mem::take(&mut decls)));
                }
                return BlockContents {
                    decls: Vec::new(),
                    rules,
                };
            }
            Token::AtKeyword(_) => {
                // §5.5.5 L2519-2528: at-keyword flushes decls.
                if !decls.is_empty() {
                    rules.push(Rule::Declarations(std::mem::take(&mut decls)));
                }
                if let Some(at_rule) = consume_an_at_rule(input, true) {
                    rules.push(Rule::AtRule(at_rule));
                }
            }
            _ => {
                // §5.5.5 L2530-2562: mark + declaration/rule ambiguity.
                input.mark();
                if let Some(decl) = consume_a_declaration(input, true) {
                    decls.push(decl);
                    input.discard_mark();
                } else {
                    input.restore_mark();
                    match consume_a_qualified_rule(input, Some(Token::Semicolon), true) {
                        Ok(Some(rule)) => {
                            // §5.5.5 L2556-2561: rule returned.
                            if !decls.is_empty() {
                                rules.push(Rule::Declarations(std::mem::take(&mut decls)));
                            }
                            rules.push(Rule::QualifiedRule(rule));
                        }
                        Ok(None) => {
                            // §5.5.5 L2546-2547: "If nothing was
                            // returned, do nothing."
                        }
                        Err(ParseError) => {
                            // §5.5.5 L2549-2554: invalid rule error.
                            if !decls.is_empty() {
                                rules.push(Rule::Declarations(std::mem::take(&mut decls)));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Result of `consume_a_block` / `consume_a_blocks_contents`. Combines
/// a list of declarations (possibly empty) and a list of child rules.
///
/// Note: when `consume_a_blocks_contents` returns, the declarations are
/// always empty — any pending decls have been flushed into `rules` as
/// `Rule::Declarations` variants (§5.5.5 L2514-2562). The `decls`
/// field is kept for forward-compatibility with future CSSOM
/// integration that may want a separate declaration list.
#[derive(Debug, Clone, Default)]
pub struct BlockContents {
    pub decls: Vec<Declaration>,
    pub rules: Vec<Rule>,
}

/// Split block contents from CP-5's `consume_a_block` into the
/// AtRule / QualifiedRule's expected shape:
///   - declarations → `Some(decls)` for AtRule, `decls` field for
///     QualifiedRule
///   - rules → `Some(rules)` for AtRule, `child_rules` field for
///     QualifiedRule
fn split_block_contents(block: BlockContents) -> (Vec<Declaration>, Vec<Rule>) {
    (block.decls, block.rules)
}

/// §5.5.3 L2372-2383: Detect whether a qualified rule's prelude
/// looks like `--<ident> :` (a custom property declaration
/// masquerading as a rule). The first two non-whitespace tokens of
/// the prelude must be:
///   - Ident starting with `--`
///   - Colon
fn looks_like_custom_property_in_prelude(prelude: &[ComponentValue]) -> bool {
    let mut iter = prelude
        .iter()
        .filter(|v| !matches!(v, ComponentValue::PreservedToken(Token::Whitespace)));
    let first = iter.next();
    let second = iter.next();
    matches!(
        (first, second),
        (
            Some(ComponentValue::PreservedToken(Token::Ident(name))),
            Some(ComponentValue::PreservedToken(Token::Colon))
        ) if name.starts_with("--")
    )
}
