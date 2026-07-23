//! §8.4 CSS Rules。
//!
//! 规范源: `d:\csswg\cssom-1\Overview.md` §8.4 L1593-2287
//!
//! `CssRule` 用 Rust enum 分发到各具体 rule 类型。rule 类型集合是
//! 规范枚举的固定集合，无需开放扩展，因此用 enum 比 trait object
//! 更符合值语义、pattern matching 更清晰。

use crate::CssStyleDeclaration;
use muskitty_css::parser::ComponentValue;

/// §8.4: A CSS rule. Enum 分发到具体 rule 类型。
#[derive(Debug, Clone)]
pub enum CssRule {
    /// §8.4 L1641: CSSStyleRule (type=1)
    Style(CssStyleRule),
    /// §8.4 L1679: CSSImportRule (type=3)
    Import(CssImportRule),
    /// §8.4 L1699: CSSMediaRule (type=4)
    Media(CssMediaRule),
    /// §8.4 L1798: CSSNamespaceRule (type=10)
    Namespace(CssNamespaceRule),
    /// §8.4: CSSSupportsRule (type=12)
    Supports(CssSupportsRule),
    /// §8.4: CSSLayerBlockRule (type=16)
    LayerBlock(CssLayerBlockRule),
    /// §8.4: CSSLayerStatementRule (type=17)
    LayerStatement(CssLayerStatementRule),
    /// §8.4: CSSContainerRule (type=15)
    Container(CssContainerRule),
    /// 未识别的 at-rule，保留 prelude + block 原样。
    Other(OtherRule),
}

impl CssRule {
    /// §8.4 L1611-1624: rule type 常量（用于 CSSRule.type IDL 属性）。
    pub fn type_id(&self) -> u16 {
        match self {
            CssRule::Style(_) => 1,
            CssRule::Import(_) => 3,
            CssRule::Media(_) => 4,
            CssRule::Namespace(_) => 10,
            CssRule::Supports(_) => 12,
            CssRule::Container(_) => 15,
            CssRule::LayerBlock(_) => 16,
            CssRule::LayerStatement(_) => 17,
            // 0 表示自定义/未标准化的 rule
            CssRule::Other(_) => 0,
        }
    }

    /// 该 rule 是否包含子 cssRules（Style/Media/Supports/Container/
    /// LayerBlock/Other 都可能含子规则）。
    pub fn has_child_rules(&self) -> bool {
        match self {
            CssRule::Style(r) => !r.css_rules.is_empty(),
            CssRule::Media(r) => !r.css_rules.is_empty(),
            CssRule::Supports(r) => !r.css_rules.is_empty(),
            CssRule::Container(r) => !r.css_rules.is_empty(),
            CssRule::LayerBlock(r) => !r.css_rules.is_empty(),
            CssRule::Other(r) => !r.child_rules.is_empty(),
            CssRule::Import(_) | CssRule::Namespace(_) | CssRule::LayerStatement(_) => false,
        }
    }
}

/// §8.4 L1641: CSSStyleRule。
#[derive(Debug, Clone)]
pub struct CssStyleRule {
    /// selector prelude（component value 列表，后续 selectors crate
    /// 负责解析与序列化）。
    pub selectors: Vec<ComponentValue>,
    /// 关联的声明块。
    pub style: CssStyleDeclaration,
    /// 嵌套子规则（CSS nesting）。
    pub css_rules: Vec<CssRule>,
}

impl CssStyleRule {
    /// 创建一个空的 style rule。
    pub fn new(selectors: Vec<ComponentValue>) -> Self {
        Self {
            selectors,
            style: CssStyleDeclaration::new(),
            css_rules: Vec::new(),
        }
    }
}

/// §8.4 L1679: CSSImportRule。
#[derive(Debug, Clone)]
pub struct CssImportRule {
    /// 被导入的 URL（href）。
    pub href: String,
    /// media query，以 component value 列表表示（简化处理，无
    /// MediaList 接口）。
    pub media: Vec<ComponentValue>,
}

/// §8.4 L1699: CSSMediaRule。
#[derive(Debug, Clone)]
pub struct CssMediaRule {
    /// media condition，以 component value 列表表示。
    pub condition: Vec<ComponentValue>,
    /// 子规则。
    pub css_rules: Vec<CssRule>,
}

/// §8.4 L1798: CSSNamespaceRule。
#[derive(Debug, Clone)]
pub struct CssNamespaceRule {
    /// 命名空间 URI。
    pub namespace_uri: String,
    /// 可选前缀（`None` 表示默认命名空间）。
    pub prefix: Option<String>,
}

/// §8.4: CSSSupportsRule。
#[derive(Debug, Clone)]
pub struct CssSupportsRule {
    /// supports condition，以 component value 列表表示。
    pub condition: Vec<ComponentValue>,
    /// 子规则。
    pub css_rules: Vec<CssRule>,
}

/// §8.4: CSSLayerBlockRule。
#[derive(Debug, Clone)]
pub struct CssLayerBlockRule {
    /// 层名（`None` 表示匿名层）。
    pub name: Option<String>,
    /// 子规则。
    pub css_rules: Vec<CssRule>,
}

/// §8.4: CSSLayerStatementRule。
#[derive(Debug, Clone)]
pub struct CssLayerStatementRule {
    /// 声明的层名列表（按出现顺序）。
    pub names: Vec<String>,
}

/// §8.4: CSSContainerRule。
#[derive(Debug, Clone)]
pub struct CssContainerRule {
    /// container condition，以 component value 列表表示。
    pub condition: Vec<ComponentValue>,
    /// 子规则。
    pub css_rules: Vec<CssRule>,
}

/// 未识别的 at-rule 的 fallback 容器。
#[derive(Debug, Clone)]
pub struct OtherRule {
    /// at-rule 名（不含 `@`）。
    pub name: String,
    /// prelude。
    pub prelude: Vec<ComponentValue>,
    /// block at-rule 的声明（`None` 表示 statement at-rule）。
    pub declarations: Option<Vec<crate::CssDeclaration>>,
    /// block at-rule 的子规则（`None` 表示 statement at-rule）。
    pub child_rules: Vec<CssRule>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_rule_new_empty() {
        let r = CssStyleRule::new(Vec::new());
        assert!(r.selectors.is_empty());
        assert!(r.style.is_empty());
        assert!(r.css_rules.is_empty());
    }

    #[test]
    fn type_id_for_each_variant() {
        assert_eq!(CssRule::Style(CssStyleRule::new(Vec::new())).type_id(), 1);
        assert_eq!(
            CssRule::Import(CssImportRule {
                href: String::new(),
                media: Vec::new(),
            })
            .type_id(),
            3
        );
        assert_eq!(
            CssRule::Media(CssMediaRule {
                condition: Vec::new(),
                css_rules: Vec::new(),
            })
            .type_id(),
            4
        );
        assert_eq!(
            CssRule::Namespace(CssNamespaceRule {
                namespace_uri: String::new(),
                prefix: None,
            })
            .type_id(),
            10
        );
        assert_eq!(
            CssRule::Supports(CssSupportsRule {
                condition: Vec::new(),
                css_rules: Vec::new(),
            })
            .type_id(),
            12
        );
        assert_eq!(
            CssRule::Container(CssContainerRule {
                condition: Vec::new(),
                css_rules: Vec::new(),
            })
            .type_id(),
            15
        );
        assert_eq!(
            CssRule::LayerBlock(CssLayerBlockRule {
                name: None,
                css_rules: Vec::new(),
            })
            .type_id(),
            16
        );
        assert_eq!(
            CssRule::LayerStatement(CssLayerStatementRule { names: Vec::new() }).type_id(),
            17
        );
        assert_eq!(
            CssRule::Other(OtherRule {
                name: String::new(),
                prelude: Vec::new(),
                declarations: None,
                child_rules: Vec::new(),
            })
            .type_id(),
            0
        );
    }

    #[test]
    fn has_child_rules_default_false() {
        assert!(!CssRule::Style(CssStyleRule::new(Vec::new())).has_child_rules());
        assert!(!CssRule::Import(CssImportRule {
            href: String::new(),
            media: Vec::new(),
        })
        .has_child_rules());
        assert!(!CssRule::Namespace(CssNamespaceRule {
            namespace_uri: String::new(),
            prefix: None,
        })
        .has_child_rules());
        assert!(
            !CssRule::LayerStatement(CssLayerStatementRule { names: Vec::new() }).has_child_rules()
        );
    }

    #[test]
    fn has_child_rules_with_nested() {
        let style_with_nested = CssRule::Style(CssStyleRule {
            selectors: Vec::new(),
            style: CssStyleDeclaration::new(),
            css_rules: vec![CssRule::Style(CssStyleRule::new(Vec::new()))],
        });
        assert!(style_with_nested.has_child_rules());

        let media_with_child = CssRule::Media(CssMediaRule {
            condition: Vec::new(),
            css_rules: vec![CssRule::Style(CssStyleRule::new(Vec::new()))],
        });
        assert!(media_with_child.has_child_rules());
    }

    #[test]
    fn import_rule_fields() {
        let r = CssImportRule {
            href: "style.css".to_string(),
            media: Vec::new(),
        };
        assert_eq!(r.href, "style.css");
    }

    #[test]
    fn namespace_rule_with_prefix() {
        let r = CssNamespaceRule {
            namespace_uri: "http://www.w3.org/2000/svg".to_string(),
            prefix: Some("svg".to_string()),
        };
        assert_eq!(r.prefix.as_deref(), Some("svg"));
    }

    #[test]
    fn namespace_rule_default_namespace() {
        let r = CssNamespaceRule {
            namespace_uri: "http://www.w3.org/2000/svg".to_string(),
            prefix: None,
        };
        assert!(r.prefix.is_none());
    }

    #[test]
    fn layer_block_anonymous() {
        let r = CssLayerBlockRule {
            name: None,
            css_rules: Vec::new(),
        };
        assert!(r.name.is_none());
    }

    #[test]
    fn layer_block_named() {
        let r = CssLayerBlockRule {
            name: Some("base".to_string()),
            css_rules: Vec::new(),
        };
        assert_eq!(r.name.as_deref(), Some("base"));
    }

    #[test]
    fn layer_statement_multiple_names() {
        let r = CssLayerStatementRule {
            names: vec![
                "base".to_string(),
                "theme".to_string(),
                "utilities".to_string(),
            ],
        };
        assert_eq!(r.names.len(), 3);
        assert_eq!(r.names[1], "theme");
    }

    #[test]
    fn other_rule_statement_form() {
        // statement at-rule: 无 declarations、无 child_rules
        let r = OtherRule {
            name: "font-face".to_string(),
            prelude: Vec::new(),
            declarations: None,
            child_rules: Vec::new(),
        };
        assert_eq!(r.name, "font-face");
        assert!(r.declarations.is_none());
    }

    #[test]
    fn other_rule_block_form() {
        let r = OtherRule {
            name: "font-feature-values".to_string(),
            prelude: Vec::new(),
            declarations: Some(Vec::new()),
            child_rules: Vec::new(),
        };
        assert!(r.declarations.is_some());
    }

    #[test]
    fn clone_preserves_structure() {
        let rule = CssRule::Media(CssMediaRule {
            condition: Vec::new(),
            css_rules: vec![CssRule::Style(CssStyleRule::new(Vec::new()))],
        });
        let cloned = rule.clone();
        assert!(cloned.has_child_rules());
    }
}
