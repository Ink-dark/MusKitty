//! §7.1/§7.2: Property registry — 属性元数据（初始值、继承标志）。
//!
//! 初始覆盖 ~20 个常用属性。后续可扩展为完整属性数据库。

/// 属性百分比参考值类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PercentageBasis {
    /// 不接受百分比。
    None,
    /// 参考父元素同属性值。
    ParentSameProperty,
    /// 参考父元素 width。
    ParentWidth,
    /// 参考父元素 height。
    ParentHeight,
    /// 参考父元素 font-size。
    ParentFontSize,
    /// 参考根元素 font-size（rem）。
    RootFontSize,
}

/// 属性元数据定义。
#[derive(Debug, Clone, Copy)]
pub struct PropertyDefinition {
    /// 属性名。
    pub name: &'static str,
    /// §7.1: 初始值。
    pub initial_value: &'static str,
    /// §7.2: 是否继承。
    pub inherited: bool,
    /// 百分比参考。
    pub percentages: PercentageBasis,
}

/// 内置属性表（~20 个常用属性）。
pub static BUILTIN_PROPERTIES: &[PropertyDefinition] = &[
    PropertyDefinition {
        name: "color",
        initial_value: "black",
        inherited: true,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "font-size",
        initial_value: "medium",
        inherited: true,
        percentages: PercentageBasis::ParentFontSize,
    },
    PropertyDefinition {
        name: "font-family",
        initial_value: "serif",
        inherited: true,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "font-weight",
        initial_value: "normal",
        inherited: true,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "line-height",
        initial_value: "normal",
        inherited: true,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "display",
        initial_value: "inline",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "margin-top",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "margin-right",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "margin-bottom",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "margin-left",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "padding-top",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "padding-right",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "padding-bottom",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "padding-left",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "width",
        initial_value: "auto",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "height",
        initial_value: "auto",
        inherited: false,
        percentages: PercentageBasis::ParentHeight,
    },
    PropertyDefinition {
        name: "background-color",
        initial_value: "transparent",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "visibility",
        initial_value: "visible",
        inherited: true,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "text-align",
        initial_value: "start",
        inherited: true,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "opacity",
        initial_value: "1",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    // —— Flexbox 属性（CSS Flexbox Level 1）——
    PropertyDefinition {
        name: "flex-direction",
        initial_value: "row",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "flex-wrap",
        initial_value: "nowrap",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "justify-content",
        initial_value: "normal",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "align-items",
        initial_value: "normal",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "align-self",
        initial_value: "auto",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "flex-grow",
        initial_value: "0",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "flex-shrink",
        initial_value: "1",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "flex-basis",
        initial_value: "auto",
        inherited: false,
        percentages: PercentageBasis::ParentWidth,
    },
    PropertyDefinition {
        name: "gap",
        initial_value: "normal",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "row-gap",
        initial_value: "normal",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "column-gap",
        initial_value: "normal",
        inherited: false,
        percentages: PercentageBasis::None,
    },
    PropertyDefinition {
        name: "box-sizing",
        initial_value: "content-box",
        inherited: false,
        percentages: PercentageBasis::None,
    },
];

/// 查找属性定义。返回 `None` 表示属性未注册。
pub fn lookup_property(name: &str) -> Option<&'static PropertyDefinition> {
    BUILTIN_PROPERTIES
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_property() {
        let def = lookup_property("color").unwrap();
        assert!(def.inherited);
        assert_eq!(def.initial_value, "black");
    }

    #[test]
    fn lookup_case_insensitive() {
        assert!(lookup_property("COLOR").is_some());
        assert!(lookup_property("Font-Size").is_some());
    }

    #[test]
    fn lookup_unknown_property() {
        assert!(lookup_property("nonexistent").is_none());
    }

    #[test]
    fn inherited_properties() {
        assert!(lookup_property("color").unwrap().inherited);
        assert!(lookup_property("font-size").unwrap().inherited);
        assert!(lookup_property("visibility").unwrap().inherited);
    }

    #[test]
    fn non_inherited_properties() {
        assert!(!lookup_property("display").unwrap().inherited);
        assert!(!lookup_property("width").unwrap().inherited);
        assert!(!lookup_property("background-color").unwrap().inherited);
    }

    #[test]
    fn percentage_basis() {
        assert_eq!(
            lookup_property("width").unwrap().percentages,
            PercentageBasis::ParentWidth
        );
        assert_eq!(
            lookup_property("font-size").unwrap().percentages,
            PercentageBasis::ParentFontSize
        );
        assert_eq!(
            lookup_property("color").unwrap().percentages,
            PercentageBasis::None
        );
    }

    #[test]
    fn builtin_property_count() {
        assert!(BUILTIN_PROPERTIES.len() >= 20);
    }
}
