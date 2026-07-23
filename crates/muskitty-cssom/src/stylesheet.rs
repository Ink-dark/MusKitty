//! §8.1 CSSStyleSheet。
//!
//! 规范源: `d:\csswg\cssom-1\Overview.md` §8.1 L639-1030
//!
//! 顶层容器，含 rules 列表 + 元数据。省略 `parent CSS style
//! sheet`/`owner node`/`owner CSS rule`/`origin-clean flag`/
//! `constructed flag` 等 DOM 集成或 JS API 相关字段（推迟）。

use crate::CssRule;

/// §8.1: A CSS style sheet.
#[derive(Debug, Clone)]
pub struct CssStyleSheet {
    /// §8.1 L700: location（绝对 URL；嵌入式为 `None`）。
    pub location: Option<String>,
    /// §8.1 L722: media，以 component value 列表表示。
    pub media: Vec<muskitty_css::parser::ComponentValue>,
    /// §8.1 L742: title。
    pub title: String,
    /// §8.1 L785: alternate flag。
    pub alternate: bool,
    /// §8.1 L800: disabled flag。
    pub disabled: bool,
    /// §8.1: rules 列表。
    pub css_rules: Vec<CssRule>,
}

impl Default for CssStyleSheet {
    fn default() -> Self {
        Self::new()
    }
}

impl CssStyleSheet {
    /// 创建一个空的 stylesheet。
    pub fn new() -> Self {
        Self {
            location: None,
            media: Vec::new(),
            title: String::new(),
            alternate: false,
            disabled: false,
            css_rules: Vec::new(),
        }
    }

    /// rules 数量。
    pub fn len(&self) -> usize {
        self.css_rules.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.css_rules.is_empty()
    }

    /// 遍历 rules。
    pub fn iter(&self) -> std::slice::Iter<'_, CssRule> {
        self.css_rules.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let ss = CssStyleSheet::default();
        assert!(ss.is_empty());
        assert_eq!(ss.len(), 0);
        assert!(ss.location.is_none());
        assert!(ss.media.is_empty());
        assert!(ss.title.is_empty());
        assert!(!ss.alternate);
        assert!(!ss.disabled);
    }

    #[test]
    fn new_equals_default() {
        assert_eq!(CssStyleSheet::new().len(), CssStyleSheet::default().len());
    }

    #[test]
    fn can_hold_rules() {
        let ss = CssStyleSheet {
            location: Some("file:///style.css".to_string()),
            title: "main".to_string(),
            alternate: true,
            disabled: false,
            media: Vec::new(),
            css_rules: vec![CssRule::Style(crate::CssStyleRule::new(Vec::new()))],
        };
        assert_eq!(ss.len(), 1);
        assert!(!ss.is_empty());
        assert_eq!(ss.location.as_deref(), Some("file:///style.css"));
        assert_eq!(ss.title, "main");
        assert!(ss.alternate);
    }

    #[test]
    fn iter_yields_rules() {
        let ss = CssStyleSheet {
            css_rules: vec![
                CssRule::Style(crate::CssStyleRule::new(Vec::new())),
                CssRule::Style(crate::CssStyleRule::new(Vec::new())),
            ],
            ..CssStyleSheet::new()
        };
        assert_eq!(ss.iter().count(), 2);
    }

    #[test]
    fn clone_preserves_state() {
        let ss = CssStyleSheet {
            title: "x".to_string(),
            css_rules: vec![CssRule::Style(crate::CssStyleRule::new(Vec::new()))],
            ..CssStyleSheet::new()
        };
        let cloned = ss.clone();
        assert_eq!(cloned.title, "x");
        assert_eq!(cloned.len(), 1);
    }
}
