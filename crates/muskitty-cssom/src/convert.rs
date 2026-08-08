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
    CssContainerRule, CssCounterStyleRule, CssDeclaration, CssFontFaceRule, CssImportRule,
    CssKeyframeRule, CssKeyframesRule, CssLayerBlockRule, CssLayerStatementRule, CssMediaRule,
    CssNamespaceRule, CssPageRule, CssPropertyRule, CssRule, CssScopeRule, CssStyleDeclaration,
    CssStyleRule, CssStyleSheet, CssSupportsRule, OtherRule,
};
use muskitty_css::parser::{AtRule, ComponentValue, Declaration, QualifiedRule, Rule, Stylesheet};
use muskitty_css::tokenizer::Token;

/// 从 css-parser 的 [`Stylesheet`] 转换为 CSSOM 的 [`CssStyleSheet`]。
///
/// 顶层 `Rule::Declarations`（裸声明，§5.5.1 视为 parse error 残留）
/// 被跳过；只转换 `Rule::QualifiedRule` 与 `Rule::AtRule`。
/// origin 默认为 Author（P2-15：需要其他 origin 用
/// [`from_stylesheet_with_origin`]）。
pub fn from_stylesheet(ss: &Stylesheet) -> CssStyleSheet {
    from_stylesheet_with_origin(ss, crate::Origin::Author)
}

/// 从 [`Stylesheet`] 转换，并显式指定 cascade origin（P2-15）。
pub fn from_stylesheet_with_origin(ss: &Stylesheet, origin: crate::Origin) -> CssStyleSheet {
    CssStyleSheet {
        origin,
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
///
/// `collect_decls`：PERF-8 —— 条件组（@media/@supports/@container/
/// @layer）丢弃 extra_decls，传 `false` 避免无用的 `Vec` 分配；style
/// rule 与 @font-face 等需要合并，传 `true`。
fn convert_child_rules(rules: &[Rule], collect_decls: bool) -> (Vec<CssRule>, Vec<CssDeclaration>) {
    let mut css_rules = Vec::new();
    let mut extra_decls = Vec::new();
    for r in rules {
        match r {
            Rule::QualifiedRule(qr) => css_rules.push(CssRule::Style(convert_qualified_rule(qr))),
            Rule::AtRule(ar) => css_rules.push(convert_at_rule(ar)),
            // 嵌套裸声明：合并到父 style 块（plan 的简化策略）
            Rule::Declarations(decls) => {
                if collect_decls {
                    for d in decls {
                        extra_decls.push(convert_declaration(d));
                    }
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
    let (css_rules, extra_decls) = convert_child_rules(&qr.child_rules, true);
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
/// P2-14: @font-face/@page/@keyframes/@counter-style/@property/@scope 类型化，
/// 不再落入 `Other`（尤其 @keyframes 的子块不再转 `Style` 污染 cascade）。
fn convert_at_rule(ar: &AtRule) -> CssRule {
    match ar.name.to_ascii_lowercase().as_str() {
        "import" => CssRule::Import(convert_import(ar)),
        "media" => CssRule::Media(convert_media(ar)),
        "font-face" => CssRule::FontFace(convert_font_face(ar)),
        "page" => CssRule::Page(convert_page(ar)),
        "keyframes" => CssRule::Keyframes(convert_keyframes(ar)),
        "namespace" => CssRule::Namespace(convert_namespace(ar)),
        "counter-style" => CssRule::CounterStyle(convert_counter_style(ar)),
        "supports" => CssRule::Supports(convert_supports(ar)),
        "layer" => convert_layer(ar),
        "container" => CssRule::Container(convert_container(ar)),
        "property" => CssRule::Property(convert_property(ar)),
        "scope" => CssRule::Scope(convert_scope(ar)),
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
    let (css_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]), false);
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
    let (css_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]), false);
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
        let (css_rules, _) = convert_child_rules(child_rules, false);
        // 层名取第一个点分隔名（P1-7）；空 → 匿名层
        let name = extract_dotted_layer_names(&ar.prelude).into_iter().next();
        CssRule::LayerBlock(CssLayerBlockRule { name, css_rules })
    } else {
        // statement 形式：@layer name1, name2;
        let names = extract_dotted_layer_names(&ar.prelude);
        CssRule::LayerStatement(CssLayerStatementRule { names })
    }
}

/// `@container` → [`CssContainerRule`]。
fn convert_container(ar: &AtRule) -> CssContainerRule {
    let (css_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]), false);
    CssContainerRule {
        condition: ar.prelude.clone(),
        css_rules,
    }
}

/// `@font-face` → [`CssFontFaceRule`]。
///
/// B5（P1-5 根因修复）后描述符进入 `AtRule.declarations`，这里转成
/// `CssStyleDeclaration`。
fn convert_font_face(ar: &AtRule) -> CssFontFaceRule {
    CssFontFaceRule {
        style: convert_declaration_block(ar.declarations.as_deref()),
    }
}

/// `@page` → [`CssPageRule`]。
fn convert_page(ar: &AtRule) -> CssPageRule {
    CssPageRule {
        selectors: ar.prelude.clone(),
        style: convert_declaration_block(ar.declarations.as_deref()),
    }
}

/// `@keyframes` → [`CssKeyframesRule`]。
///
/// 关键帧块（`from`/`to`/`0%`）是子层 [`Rule::QualifiedRule`]，转为
/// [`CssKeyframeRule`]（P2-14：不再落入 `CssRule::Style`，避免 cascade
/// 当普通 style rule 参与元素匹配）。
fn convert_keyframes(ar: &AtRule) -> CssKeyframesRule {
    let name = first_ident(&ar.prelude);
    let mut keyframes = Vec::new();
    if let Some(child_rules) = &ar.child_rules {
        for r in child_rules {
            if let Rule::QualifiedRule(qr) = r {
                keyframes.push(CssKeyframeRule {
                    key_text: qr.prelude.clone(),
                    style: convert_declaration_block(Some(&qr.declarations)),
                });
            }
        }
    }
    CssKeyframesRule { name, keyframes }
}

/// `@counter-style` → [`CssCounterStyleRule`]。
fn convert_counter_style(ar: &AtRule) -> CssCounterStyleRule {
    CssCounterStyleRule {
        name: first_ident(&ar.prelude).unwrap_or_default(),
        style: convert_declaration_block(ar.declarations.as_deref()),
    }
}

/// `@property` → [`CssPropertyRule`]。
fn convert_property(ar: &AtRule) -> CssPropertyRule {
    CssPropertyRule {
        name: first_ident(&ar.prelude).unwrap_or_default(),
        style: convert_declaration_block(ar.declarations.as_deref()),
    }
}

/// `@scope` → [`CssScopeRule`]。
fn convert_scope(ar: &AtRule) -> CssScopeRule {
    let (css_rules, _) = convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]), false);
    CssScopeRule {
        prelude: ar.prelude.clone(),
        css_rules,
    }
}

/// 未识别 at-rule → [`OtherRule`]。
fn convert_other(ar: &AtRule) -> OtherRule {
    let (child_rules, extra_decls) =
        convert_child_rules(ar.child_rules.as_deref().unwrap_or(&[]), true);
    // P1-5：@font-face 等块 at-rule 的声明经 B5 根因修复后已进入
    // `ar.declarations`；若子规则里仍夹带裸声明（convert_child_rules
    // 收集的 `Rule::Declarations`），并入末尾兜底，避免丢失。
    let mut declarations: Option<Vec<CssDeclaration>> = ar
        .declarations
        .as_ref()
        .map(|decls| decls.iter().map(convert_declaration).collect());
    if let Some(decls) = &mut declarations {
        decls.extend(extra_decls);
    }
    OtherRule {
        name: ar.name.clone(),
        prelude: ar.prelude.clone(),
        declarations,
        child_rules,
    }
}

/// 转换 [`Declaration`] → [`CssDeclaration`]。
///
/// P2-16: 透传 `original_text`（custom property 的源文本，供 var()
/// 解析使用）。
fn convert_declaration(d: &Declaration) -> CssDeclaration {
    CssDeclaration {
        name: d.name.clone(),
        value: d.value.clone(),
        important: d.important,
        original_text: d.original_text.clone(),
    }
}

/// 把 `Option<&[Declaration]>` 转为 [`CssStyleDeclaration`]。
///
/// 供 @font-face/@page/@counter-style/@property 等 descriptor at-rule
/// 使用（B5 后描述符在 `ar.declarations`）。
fn convert_declaration_block(decls: Option<&[Declaration]>) -> CssStyleDeclaration {
    let mut style = CssStyleDeclaration::new();
    if let Some(decls) = decls {
        for d in decls {
            style.push(convert_declaration(d));
        }
    }
    style
}

/// 取 component value 列表中的首个 Ident（用于 at-rule 名）。
fn first_ident(prelude: &[ComponentValue]) -> Option<String> {
    prelude.iter().find_map(|cv| match cv {
        ComponentValue::PreservedToken(Token::Ident(s)) => Some(s.clone()),
        _ => None,
    })
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

/// 从 prelude 提取点分隔的层名列表（P1-7）。
///
/// `@layer a.b.c, d;` → `["a.b.c", "d"]`。Ident 与 `Delim('.')` 拼成
/// 一个名字，`Comma` 分隔多个名字，空白跳过（CSSOM §8.4 L1988-1994：
/// CSSOMString 形式为 `<dotted-ident>#`）。
fn extract_dotted_layer_names(prelude: &[ComponentValue]) -> Vec<String> {
    let mut names = Vec::new();
    let mut current = String::new();
    for cv in prelude {
        match cv {
            ComponentValue::PreservedToken(Token::Whitespace) => {}
            ComponentValue::PreservedToken(Token::Comma) => {
                names.push(std::mem::take(&mut current));
            }
            ComponentValue::PreservedToken(Token::Ident(s)) => current.push_str(s),
            ComponentValue::PreservedToken(Token::Delim('.')) => current.push('.'),
            _ => {}
        }
    }
    if !current.is_empty() {
        names.push(current);
    }
    names
}
