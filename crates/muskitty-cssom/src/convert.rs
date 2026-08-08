//! 从 css-parser 语法层到 CSSOM 语义层的单向转换。
//!
//! 规范源:
//! - CSSOM §8.4 L1627-1634 (parse a CSS rule)
//! - CSSOM §8.6 L2338-2349 (parse a CSS declaration block)
//! - CSS Syntax §5.2 (语法层数据结构)
//!
//! 转换是 one-way 的：语法→语义。转换后 CSSOM 树独立存在，不反向
//! 引用 css-parser 的 `Stylesheet`，避免生命周期耦合。

use crate::{
    CssContainerRule, CssDeclaration, CssImportRule, CssLayerBlockRule, CssLayerStatementRule,
    CssMediaRule, CssNamespaceRule, CssRule, CssStyleDeclaration, CssStyleRule, CssStyleSheet,
    CssSupportsRule, OtherRule,
};
use muskitty_css::parser::{AtRule, ComponentValue, Declaration, QualifiedRule, Rule, Stylesheet};
use muskitty_css::tokenizer::Token;

/// 从 css-parser 的 [`Stylesheet`] 转换为 CSSOM 的 [`CssStyleSheet`]。
///
/// 顶层 `Rule::Declarations`（裸声明，§5.5.1 视为 parse error 残留）
/// 被跳过；只转换 `Rule::QualifiedRule` 与 `Rule::AtRule`。
pub fn from_stylesheet(ss: &Stylesheet) -> CssStyleSheet {
    CssStyleSheet {
        origin: crate::Origin::Author,
        location: None,
        media: Vec::new(),
        title: String::new(),
        alternate: false,
        disabled: false,
        css_rules: convert_rules(&ss.rules),
    }
}

/// 转换 rule 列表，跳过 `Rule::Declarations`。
fn convert_rules(rules: &[Rule]) -> Vec<CssRule> {
    rules
        .iter()
        .filter_map(|r| match r {
            Rule::QualifiedRule(qr) => Some(CssRule::Style(convert_qualified_rule(qr))),
            Rule::AtRule(ar) => Some(convert_at_rule(ar)),
            // 顶层裸声明：CSS Syntax §5.5.1 视为 parse error，跳过
            Rule::Declarations(_) => None,
        })
        .collect()
}

/// 转换子 rule 列表，`Rule::Declarations` 合并到父 style 块。
///
/// 返回 `(child_css_rules, extra_declarations)`：`extra_declarations`
/// 是从 `Rule::Declarations` 收集的声明，由调用方合并到父
/// `CssStyleDeclaration`。
fn convert_child_rules(rules: &[Rule]) -> (Vec<CssRule>, Vec<CssDeclaration>) {
    let mut css_rules = Vec::new();
    let mut extra_decls = Vec::new();
    for r in rules {
        match r {
            Rule::QualifiedRule(qr) => css_rules.push(CssRule::Style(convert_qualified_rule(qr))),
            Rule::AtRule(ar) => css_rules.push(convert_at_rule(ar)),
            // 嵌套裸声明：合并到父 style 块（plan 的简化策略）
            Rule::Declarations(decls) => {
                for d in decls {
                    extra_decls.push(convert_declaration(d));
                }
            }
        }
    }
    (css_rules, extra_decls)
}

/// 转换 [`QualifiedRule`] → [`CssStyleRule`]。
fn convert_qualified_rule(qr: &QualifiedRule) -> CssStyleRule {
    let mut style = CssStyleDeclaration::new();
    for d in &qr.declarations {
        style.push(convert_declaration(d));
    }
    let (css_rules, extra_decls) = convert_child_rules(&qr.child_rules);
    // 嵌套裸声明追加到父 style 末尾（cascade 后出现胜出）
    for d in extra_decls {
        style.push(d);
    }
    CssStyleRule {
        selectors: qr.prelude.clone(),
        style,
        css_rules,
    }
}

/// 转换 [`AtRule`] → 对应的 [`CssRule`] 变体。
///
/// P1-6: at-rule 名大小写不敏感（CSS Syntax §6.3.4），统一转小写再分发。
fn convert_at_rule(ar: &AtRule) -> CssRule {
    match ar.name.to_ascii_lowercase().as_str() {
        "import" => CssRule::Import(convert_import(ar)),
        "media" => CssRule::Media(convert_media(ar)),
        "namespace" => CssRule::Namespace(convert_namespace(ar)),
        "supports" => CssRule::Supports(convert_supports(ar)),
        "layer" => convert_layer(ar),
        "container" => CssRule::Container(convert_container(ar)),
        _ => CssRule::Other(convert_other(ar)),
    }
}

/// `@import` → [`CssImportRule`]。
///
/// prelude 第一个 string/url 是 href，其余（跳过首尾空白）是 media。
fn convert_import(ar: &AtRule) -> CssImportRule {
    let mut href = String::new();
    let mut media = Vec::new();
    let mut found_href = false;
    for cv in &ar.prelude {
        if !found_href {
            if let Some(s) = extract_string_or_url(cv) {
                href = s;
                found_href = true;
                continue;
            }
            // 跳过 href 前的空白
            if matches!(cv, ComponentValue::PreservedToken(Token::Whitespace)) {
                continue;
            }
            // 其它 token 也跳过（容错）
            continue;
        }
        // href 之后的都归入 media（保留原始 component values）
        media.push(cv.clone());
    }
    CssImportRule { href, media }
}

/// `@media` → [`CssMediaRule`]。
fn convert_media(ar: &AtRule) -> CssMediaRule {
    let (css_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]));
    CssMediaRule {
        condition: ar.prelude.clone(),
        css_rules,
    }
}

/// `@namespace` → [`CssNamespaceRule`]。
///
/// prelude 形如 `[ <prefix> ]? <string>` 或 `[ <prefix> ]? url(...)`。
fn convert_namespace(ar: &AtRule) -> CssNamespaceRule {
    let mut prefix = None;
    let mut namespace_uri = String::new();
    let mut seen_prefix = false;
    for cv in &ar.prelude {
        match cv {
            ComponentValue::PreservedToken(Token::Whitespace) => continue,
            ComponentValue::PreservedToken(Token::Ident(s)) if !seen_prefix => {
                prefix = Some(s.clone());
                seen_prefix = true;
            }
            _ => {
                if let Some(s) = extract_string_or_url(cv) {
                    namespace_uri = s;
                }
            }
        }
    }
    CssNamespaceRule {
        namespace_uri,
        prefix,
    }
}

/// `@supports` → [`CssSupportsRule`]。
fn convert_supports(ar: &AtRule) -> CssSupportsRule {
    let (css_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]));
    CssSupportsRule {
        condition: ar.prelude.clone(),
        css_rules,
    }
}

/// `@layer` → [`CssLayerBlockRule`]（block 形式）或
/// [`CssLayerStatementRule`]（statement 形式，无 block）。
fn convert_layer(ar: &AtRule) -> CssRule {
    if let Some(child_rules) = &ar.child_rules {
        // block 形式：@layer [name] { rules }
        let (css_rules, _) = convert_child_rules(child_rules);
        let name = extract_first_ident(&ar.prelude);
        CssRule::LayerBlock(CssLayerBlockRule { name, css_rules })
    } else {
        // statement 形式：@layer name1, name2;
        let names = extract_ident_list(&ar.prelude);
        CssRule::LayerStatement(CssLayerStatementRule { names })
    }
}

/// `@container` → [`CssContainerRule`]。
fn convert_container(ar: &AtRule) -> CssContainerRule {
    let (css_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]));
    CssContainerRule {
        condition: ar.prelude.clone(),
        css_rules,
    }
}

/// 未识别 at-rule → [`OtherRule`]。
fn convert_other(ar: &AtRule) -> OtherRule {
    let (child_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]));
    let declarations = ar
        .declarations
        .as_ref()
        .map(|decls| decls.iter().map(convert_declaration).collect());
    OtherRule {
        name: ar.name.clone(),
        prelude: ar.prelude.clone(),
        declarations,
        child_rules,
    }
}

/// 转换 [`Declaration`] → [`CssDeclaration`]。
///
/// 丢弃 `original_text`（CSSOM 层不关心 custom property 的源文本跟踪，
/// 那是 css-values/var() 解析时用的）。
fn convert_declaration(d: &Declaration) -> CssDeclaration {
    CssDeclaration {
        name: d.name.clone(),
        value: d.value.clone(),
        important: d.important,
    }
}

// ── 辅助函数 ──────────────────────────────────────────────────────

/// 从 component value 中提取 string 或 url 内容。
///
/// - `Token::String(s)` → `Some(s)`
/// - `Token::Url(s)` → `Some(s)`
/// - `Function("url", [String])` → `Some(s)`（§4.3.8：带引号的
///   `url("...")` 是 Function 形态而非 Url token；P1-4）
/// - 其它 → `None`
fn extract_string_or_url(cv: &ComponentValue) -> Option<String> {
    match cv {
        ComponentValue::PreservedToken(Token::String(s)) => Some(s.clone()),
        ComponentValue::PreservedToken(Token::Url(s)) => Some(s.clone()),
        ComponentValue::Function(f) if f.name.eq_ignore_ascii_case("url") => {
            if let [ComponentValue::PreservedToken(Token::String(s))] = f.value.as_slice() {
                Some(s.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 从 component value 列表中提取第一个 ident（跳过空白）。
fn extract_first_ident(prelude: &[ComponentValue]) -> Option<String> {
    for cv in prelude {
        match cv {
            ComponentValue::PreservedToken(Token::Whitespace) => continue,
            ComponentValue::PreservedToken(Token::Ident(s)) => return Some(s.clone()),
            _ => return None,
        }
    }
    None
}

/// 从 component value 列表中提取所有 ident（跳过空白和逗号）。
fn extract_ident_list(prelude: &[ComponentValue]) -> Vec<String> {
    prelude
        .iter()
        .filter_map(|cv| match cv {
            ComponentValue::PreservedToken(Token::Ident(s)) => Some(s.clone()),
            _ => None,
        })
        .collect()
}
