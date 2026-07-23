//! §4.1 DeclaredValue + §4.4 ComputedValue + ComputedStyle。

use muskitty_css::parser::ComponentValue;
use muskitty_cssom::Origin;
use muskitty_selectors::Specificity;

/// §4.1: A declared value（cascade 输入项）。
///
/// 一条匹配元素的 CSS 声明，附带 cascade 排序所需的元数据。
#[derive(Debug, Clone)]
pub struct DeclaredValue {
    /// 属性名。
    pub property: String,
    /// 声明的值（component value 列表）。
    pub value: Vec<ComponentValue>,
    /// `!important` 标志。
    pub important: bool,
    /// §6.2: cascade origin。
    pub origin: Origin,
    /// §6.1 准则 6: 选择器特异性。
    pub specificity: Specificity,
    /// §6.1 准则 7: 文档序（全局递增）。
    pub order: usize,
    /// §6.1 准则 4: 是否来自 `style` 属性。
    pub from_style_attr: bool,
}

/// §4.4: Computed value（cascade 输出）。
#[derive(Debug, Clone)]
pub enum ComputedValue {
    /// 已解析为绝对值（相对单位已转换）。
    Resolved(Vec<ComponentValue>),
    /// 关键字值（如 "auto"、"none"）。
    Keyword(String),
    /// 未识别的属性值（原样保留 component values）。
    Raw(Vec<ComponentValue>),
}

/// 每元素的 computed style 表。
#[derive(Debug, Clone, Default)]
pub struct ComputedStyle {
    /// 属性名 → computed value。
    pub properties: std::collections::HashMap<String, ComputedValue>,
}

impl ComputedStyle {
    /// 创建空的 computed style。
    pub fn new() -> Self {
        Self::default()
    }

    /// 获取属性值。
    pub fn get(&self, name: &str) -> Option<&ComputedValue> {
        self.properties.get(name)
    }

    /// 设置属性值。
    pub fn set(&mut self, name: impl Into<String>, value: ComputedValue) {
        self.properties.insert(name.into(), value);
    }
}
