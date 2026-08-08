//! 序列化（CSSOM §3 + §8.4-§8.6）。
//!
//! 规范源: `d:\csswg\cssom-1\Overview.md`
//! - §3 L72-167: Common Serializing Idioms
//! - §8.4 L1637-1834: CSS Rule 序列化
//! - §8.5 L2352-2364: CSS Declaration 序列化
//! - §8.6 L2370-2414: CSS Declaration Block 序列化

use crate::{
    CssContainerRule, CssCounterStyleRule, CssDeclaration, CssFontFaceRule, CssImportRule,
    CssKeyframeRule, CssKeyframesRule, CssLayerBlockRule, CssLayerStatementRule, CssMediaRule,
    CssNamespaceRule, CssPageRule, CssPropertyRule, CssRule, CssScopeRule, CssStyleDeclaration,
    CssStyleRule, CssStyleSheet, CssSupportsRule, OtherRule,
};
use muskitty_css::parser::{BlockKind, ComponentValue, Function, SimpleBlock};
use muskitty_css::tokenizer::{HashType, Numeric, Token};

/// 序列化 trait。
pub trait ToCss {
    /// 序列化为 CSS 字符串。
    fn to_css_string(&self) -> String;
}

// ── §3 Common Serializing Idioms ──────────────────────────────────

/// §3 L75-76: escape a character（`\` + char）。
fn escape_char(c: char) -> String {
    format!("\\{}", c)
}

/// §3 L78-82: escape a character as code point（`\` + hex + space）。
fn escape_as_code_point(c: char) -> String {
    format!("\\{:x} ", c as u32)
}

/// §3 L84-107: serialize an identifier。
pub fn serialize_identifier(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c == '\0' {
            out.push('\u{FFFD}');
        } else if (c as u32) <= 0x1f
            || c == '\u{7f}'
            || (i == 0 && c.is_ascii_digit())
            || (i == 1 && c.is_ascii_digit() && name.starts_with('-'))
        {
            // 三种情况都按 code point 转义（§3 L88-97）
            out.push_str(&escape_as_code_point(c));
        } else if i == 0 && c == '-' && name.len() == 1 {
            out.push_str(&escape_char(c));
        } else if (c as u32) >= 0x80 || c == '-' || c == '_' || c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push_str(&escape_char(c));
        }
    }
    out
}

/// §3 L127-138: serialize a string（用 `"` 包裹）。
pub fn serialize_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        if c == '\0' {
            out.push('\u{FFFD}');
        } else if (c as u32) <= 0x1f || c == '\u{7f}' {
            out.push_str(&escape_as_code_point(c));
        } else if c == '"' || c == '\\' {
            out.push_str(&escape_char(c));
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

/// §3 L144-147: serialize a URL（`url(` + string + `)`）。
pub fn serialize_url(url: &str) -> String {
    format!("url({})", serialize_string(url))
}

// ── 数字格式化 ────────────────────────────────────────────────────

/// 格式化数值（整数无小数点，浮点数最小表示）。
fn format_number(n: f64) -> String {
    if n == n.trunc() && n.abs() < 1e16 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

/// 序列化 [`Numeric`]。
fn serialize_numeric(n: &Numeric) -> String {
    format_number(n.value)
}

// ── Token 序列化 ──────────────────────────────────────────────────

/// 序列化 [`Token`]（作为 preserved token）。
fn serialize_token(token: &Token) -> String {
    match token {
        Token::Ident(s) => serialize_identifier(s),
        Token::Function(s) => format!("{}(", serialize_identifier(s)),
        Token::AtKeyword(s) => format!("@{}", serialize_identifier(s)),
        Token::Hash(s, HashType::Id) => format!("#{}", serialize_identifier(s)),
        Token::Hash(s, HashType::Unrestricted) => format!("#{}", s),
        Token::String(s) => serialize_string(s),
        Token::BadString => String::new(),
        Token::Url(s) => serialize_url(s),
        Token::BadUrl => String::new(),
        Token::Delim(c) => c.to_string(),
        Token::Number(n) => serialize_numeric(n),
        Token::Percentage(n) => format!("{}%", serialize_numeric(n)),
        Token::Dimension(n, unit) => {
            format!("{}{}", serialize_numeric(n), serialize_identifier(unit))
        }
        Token::UnicodeRange(start, end) => serialize_unicode_range(*start, *end),
        Token::Whitespace => " ".to_string(),
        Token::Comment(s) => format!("/*{}*/", s),
        Token::Colon => ":".to_string(),
        Token::Semicolon => ";".to_string(),
        Token::Comma => ",".to_string(),
        Token::OpenBracket => "[".to_string(),
        Token::CloseBracket => "]".to_string(),
        Token::OpenParen => "(".to_string(),
        Token::CloseParen => ")".to_string(),
        Token::OpenBrace => "{".to_string(),
        Token::CloseBrace => "}".to_string(),
        Token::Cdo => "<!--".to_string(),
        Token::Cdc => "-->".to_string(),
        Token::Eof => String::new(),
    }
}

/// 序列化 unicode-range。
fn serialize_unicode_range(start: Option<u32>, end: Option<u32>) -> String {
    match (start, end) {
        (Some(s), Some(e)) if s == e => format!("U+{:04X}", s),
        (Some(s), Some(e)) => format!("U+{:04X}-{:04X}", s, e),
        (Some(s), None) => format!("U+{:04X}", s),
        _ => "U+0".to_string(),
    }
}

// ── ComponentValue 序列化 ─────────────────────────────────────────

/// 序列化 [`ComponentValue`] 列表（§9: serialize a list of component
/// values）。
pub fn serialize_component_values(cvs: &[ComponentValue]) -> String {
    cvs.iter().map(serialize_component_value).collect()
}

/// 序列化单个 [`ComponentValue`]。
pub fn serialize_component_value(cv: &ComponentValue) -> String {
    match cv {
        ComponentValue::PreservedToken(t) => serialize_token(t),
        ComponentValue::Function(f) => serialize_function(f),
        ComponentValue::SimpleBlock(b) => serialize_simple_block(b),
    }
}

/// 序列化 [`Function`]：`name(arg1 arg2 ...)`。
fn serialize_function(f: &Function) -> String {
    format!(
        "{}({})",
        serialize_identifier(&f.name),
        serialize_component_values(&f.value)
    )
}

/// 序列化 [`SimpleBlock`]。
fn serialize_simple_block(b: &SimpleBlock) -> String {
    let (open, close) = match b.kind {
        BlockKind::Curly => ("{", "}"),
        BlockKind::Square => ("[", "]"),
        BlockKind::Paren => ("(", ")"),
    };
    format!("{}{}{}", open, serialize_component_values(&b.value), close)
}

// ── CssDeclaration 序列化（§8.5 L2352-2364）──────────────────────

impl ToCss for CssDeclaration {
    fn to_css_string(&self) -> String {
        let mut s = format!("{}: {}", self.name, serialize_component_values(&self.value));
        if self.important {
            s.push_str(" !important");
        }
        s
    }
}

// ── CssStyleDeclaration 序列化（§8.6 L2370-2414）─────────────────

impl ToCss for CssStyleDeclaration {
    fn to_css_string(&self) -> String {
        // 简化版：遍历 declarations，每个声明后加分号，用空格连接。
        // 不做 shorthand 合并（推迟到 Cascade）。
        // PERF-6/7：直写 &mut String，避免中间 Vec<String> 分配。
        let mut s = String::new();
        for (i, d) in self.declarations.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&d.to_css_string());
            s.push(';');
        }
        s
    }
}

// ── CssRule 序列化（§8.4 L1637-1834）─────────────────────────────

impl ToCss for CssRule {
    fn to_css_string(&self) -> String {
        match self {
            CssRule::Style(r) => r.to_css_string(),
            CssRule::Import(r) => r.to_css_string(),
            CssRule::Media(r) => r.to_css_string(),
            CssRule::FontFace(r) => r.to_css_string(),
            CssRule::Page(r) => r.to_css_string(),
            CssRule::Keyframes(r) => r.to_css_string(),
            CssRule::Keyframe(r) => r.to_css_string(),
            CssRule::Namespace(r) => r.to_css_string(),
            CssRule::CounterStyle(r) => r.to_css_string(),
            CssRule::Supports(r) => r.to_css_string(),
            CssRule::LayerBlock(r) => r.to_css_string(),
            CssRule::LayerStatement(r) => r.to_css_string(),
            CssRule::Container(r) => r.to_css_string(),
            CssRule::Property(r) => r.to_css_string(),
            CssRule::Scope(r) => r.to_css_string(),
            CssRule::Other(r) => r.to_css_string(),
        }
    }
}

impl ToCss for CssStyleRule {
    /// §8.4 L1641-1674: `selectors { decls }` + nested rules。
    fn to_css_string(&self) -> String {
        let selectors = serialize_component_values(&self.selectors);
        let mut s = String::new();
        s.push_str(&selectors);
        // 去掉 selector 末尾可能的空白
        let s_trimmed = s.trim_end();
        let mut result = format!("{} {{ ", s_trimmed);
        if !self.style.is_empty() {
            result.push_str(&self.style.to_css_string());
        }
        for child in &self.css_rules {
            if !self.style.is_empty() || !result.ends_with("{ ") {
                result.push(' ');
            }
            result.push_str(&child.to_css_string());
        }
        result.push_str(" }");
        result
    }
}

impl ToCss for CssImportRule {
    /// §8.4 L1679-1697: `@import url("href") media;`
    fn to_css_string(&self) -> String {
        let mut s = format!("@import {}", serialize_url(&self.href));
        if !self.media.is_empty() {
            s.push(' ');
            s.push_str(&serialize_component_values(&self.media));
        }
        s.push(';');
        s
    }
}

impl ToCss for CssMediaRule {
    /// §8.4 L1699-1796: `@media condition { rules }`
    fn to_css_string(&self) -> String {
        let condition = serialize_component_values(&self.condition);
        serialize_block_at_rule("media", &condition, &self.css_rules)
    }
}

impl ToCss for CssNamespaceRule {
    /// §8.4 L1798-1818: `@namespace [prefix] url;`
    fn to_css_string(&self) -> String {
        let mut s = String::from("@namespace");
        if let Some(p) = &self.prefix {
            s.push(' ');
            s.push_str(&serialize_identifier(p));
        }
        s.push(' ');
        s.push_str(&serialize_url(&self.namespace_uri));
        s.push(';');
        s
    }
}

impl ToCss for CssSupportsRule {
    /// §8.4: `@supports condition { rules }`
    fn to_css_string(&self) -> String {
        let condition = serialize_component_values(&self.condition);
        serialize_block_at_rule("supports", &condition, &self.css_rules)
    }
}

impl ToCss for CssLayerBlockRule {
    /// §8.4: `@layer [name] { rules }`
    fn to_css_string(&self) -> String {
        let prelude = match &self.name {
            Some(n) => serialize_identifier(n),
            None => String::new(),
        };
        serialize_block_at_rule("layer", &prelude, &self.css_rules)
    }
}

impl ToCss for CssLayerStatementRule {
    /// §8.4: `@layer name1, name2;`
    fn to_css_string(&self) -> String {
        // PERF-6/7：直写 &mut String，避免中间 Vec<String> 分配。
        let mut s = String::from("@layer ");
        for (i, n) in self.names.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&serialize_identifier(n));
        }
        s.push(';');
        s
    }
}

impl ToCss for CssContainerRule {
    /// §8.4: `@container condition { rules }`
    fn to_css_string(&self) -> String {
        let condition = serialize_component_values(&self.condition);
        serialize_block_at_rule("container", &condition, &self.css_rules)
    }
}

impl ToCss for CssFontFaceRule {
    /// `@font-face { descriptors }`
    fn to_css_string(&self) -> String {
        serialize_descriptor_at_rule("font-face", "", &self.style)
    }
}

impl ToCss for CssPageRule {
    /// `@page [selector] { descriptors }`
    fn to_css_string(&self) -> String {
        let prelude = serialize_component_values(&self.selectors);
        serialize_descriptor_at_rule("page", &prelude, &self.style)
    }
}

impl ToCss for CssKeyframesRule {
    /// `@keyframes name { keyframe, ... }`
    fn to_css_string(&self) -> String {
        let name = self.name.as_deref().unwrap_or("");
        let mut s = format!("@keyframes {}", serialize_identifier(name));
        s.push_str(" {");
        for kf in &self.keyframes {
            s.push(' ');
            s.push_str(&kf.to_css_string());
        }
        s.push_str(" }");
        s
    }
}

impl ToCss for CssKeyframeRule {
    /// `from { declarations }`
    fn to_css_string(&self) -> String {
        let key_text = serialize_component_values(&self.key_text);
        let mut s = format!("{} {{", key_text.trim());
        if !self.style.is_empty() {
            s.push(' ');
            s.push_str(&self.style.to_css_string());
        }
        s.push_str(" }");
        s
    }
}

impl ToCss for CssCounterStyleRule {
    /// `@counter-style name { descriptors }`
    fn to_css_string(&self) -> String {
        serialize_descriptor_at_rule(
            "counter-style",
            &serialize_identifier(&self.name),
            &self.style,
        )
    }
}

impl ToCss for CssPropertyRule {
    /// `@property --name { descriptors }`
    fn to_css_string(&self) -> String {
        serialize_descriptor_at_rule("property", &serialize_identifier(&self.name), &self.style)
    }
}

impl ToCss for CssScopeRule {
    /// `@scope prelude { rules }`
    fn to_css_string(&self) -> String {
        let prelude = serialize_component_values(&self.prelude);
        serialize_block_at_rule("scope", &prelude, &self.css_rules)
    }
}

impl ToCss for OtherRule {
    /// `@name prelude;` 或 `@name prelude { declarations; rules }`
    fn to_css_string(&self) -> String {
        let prelude = serialize_component_values(&self.prelude);
        if self.child_rules.is_empty() && self.declarations.is_none() {
            // statement at-rule
            let mut s = format!("@{}", serialize_identifier(&self.name));
            if !prelude.trim().is_empty() {
                s.push(' ');
                s.push_str(&prelude);
            }
            s.push(';');
            s
        } else {
            // block at-rule
            let mut s = format!("@{}", serialize_identifier(&self.name));
            if !prelude.trim().is_empty() {
                s.push(' ');
                s.push_str(&prelude);
            }
            s.push_str(" {");
            // P1-5：block 形式输出 declarations（@font-face 等），否则
            // roundtrip 会丢失块内声明。
            if let Some(decls) = &self.declarations {
                for d in decls {
                    s.push(' ');
                    s.push_str(&d.to_css_string());
                    s.push(';');
                }
            }
            for r in &self.child_rules {
                s.push(' ');
                s.push_str(&r.to_css_string());
            }
            s.push_str(" }");
            s
        }
    }
}

// ── CssStyleSheet 序列化 ──────────────────────────────────────────

impl ToCss for CssStyleSheet {
    /// rules 用换行连接。
    fn to_css_string(&self) -> String {
        // PERF-6/7：直写 &mut String，避免中间 Vec<String> 分配。
        let mut s = String::new();
        for (i, r) in self.css_rules.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str(&r.to_css_string());
        }
        s
    }
}

// ── 辅助 ──────────────────────────────────────────────────────────

/// 序列化 descriptor at-rule：`@name prelude { descriptors }`。
///
/// 供 @font-face/@page/@counter-style/@property 使用（P2-14 类型化变体）。
fn serialize_descriptor_at_rule(name: &str, prelude: &str, style: &CssStyleDeclaration) -> String {
    let mut s = format!("@{}", serialize_identifier(name));
    if !prelude.trim().is_empty() {
        s.push(' ');
        s.push_str(prelude);
    }
    s.push_str(" {");
    if !style.is_empty() {
        s.push(' ');
        s.push_str(&style.to_css_string());
    }
    s.push_str(" }");
    s
}

/// 序列化 block at-rule：`@name prelude { rules }`。
fn serialize_block_at_rule(name: &str, prelude: &str, css_rules: &[CssRule]) -> String {
    let mut s = format!("@{}", serialize_identifier(name));
    if !prelude.trim().is_empty() {
        s.push(' ');
        s.push_str(prelude);
    }
    s.push_str(" {");
    for r in css_rules {
        s.push(' ');
        s.push_str(&r.to_css_string());
    }
    s.push_str(" }");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_simple_identifier() {
        assert_eq!(serialize_identifier("color"), "color");
        assert_eq!(serialize_identifier("font-size"), "font-size");
    }

    #[test]
    fn serialize_identifier_with_escape() {
        // 首字符是数字需要转义
        assert_eq!(serialize_identifier("1foo"), "\\31 foo");
        // 单独的 `-`
        assert_eq!(serialize_identifier("-"), "\\-");
    }

    #[test]
    fn serialize_string_basic() {
        assert_eq!(serialize_string("hello"), "\"hello\"");
    }

    #[test]
    fn serialize_string_with_quotes() {
        assert_eq!(serialize_string("he said \"hi\""), "\"he said \\\"hi\\\"\"");
    }

    #[test]
    fn serialize_string_with_backslash() {
        assert_eq!(serialize_string("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn serialize_url_basic() {
        assert_eq!(serialize_url("style.css"), "url(\"style.css\")");
    }

    #[test]
    fn format_number_integer() {
        assert_eq!(format_number(10.0), "10");
        assert_eq!(format_number(-5.0), "-5");
        assert_eq!(format_number(0.0), "0");
    }

    #[test]
    fn format_number_float() {
        assert_eq!(format_number(2.5), "2.5");
        assert_eq!(format_number(0.5), "0.5");
    }

    #[test]
    fn declaration_without_important() {
        let d = CssDeclaration::new("color", Vec::new(), false);
        assert_eq!(d.to_css_string(), "color: ");
    }

    #[test]
    fn declaration_with_important() {
        let d = CssDeclaration::new("color", Vec::new(), true);
        assert_eq!(d.to_css_string(), "color:  !important");
    }

    #[test]
    fn style_declaration_serialization() {
        let mut block = CssStyleDeclaration::new();
        block.push(CssDeclaration::new("a", Vec::new(), false));
        block.push(CssDeclaration::new("b", Vec::new(), false));
        assert_eq!(block.to_css_string(), "a: ; b: ;");
    }

    #[test]
    fn layer_statement_serialization() {
        let r = CssLayerStatementRule {
            names: vec!["base".to_string(), "theme".to_string()],
        };
        assert_eq!(r.to_css_string(), "@layer base, theme;");
    }

    #[test]
    fn empty_stylesheet_serializes_to_empty() {
        let ss = CssStyleSheet::new();
        assert_eq!(ss.to_css_string(), "");
    }
}
