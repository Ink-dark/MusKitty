//! §4.1 DeclaredValue + §4.4 ComputedValue + ComputedStyle。

use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;
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
    /// §6.1 准则 5: 所属 @layer 的全局序号（按首次出现分配；
    /// `None` = 未分层，即隐式层）。P1-3。
    pub layer_order: Option<usize>,
}

/// §4.4: Computed value（cascade 输出）。
///
/// 单态：三态（Keyword/Raw/Resolved）合并为统一 token 序列（P2-20）。
/// 关键字值即 `[Ident(s)]`；相对单位已解析为 px 的 Dimension；无法解析的
/// 原始值原样保留 component values。下游一律按 token 序列消费，不再区分
/// 值来源。
#[derive(Debug, Clone)]
pub struct ComputedValue(pub Vec<ComponentValue>);

impl ComputedValue {
    /// 从关键字构造（`[Ident(s)]`）。
    pub fn from_keyword(s: &str) -> Self {
        Self(vec![ComponentValue::PreservedToken(Token::Ident(
            s.to_string(),
        ))])
    }

    /// 从 component value 列表构造。
    pub fn from_tokens(tokens: Vec<ComponentValue>) -> Self {
        Self(tokens)
    }

    /// 底层 token 序列。
    pub fn tokens(&self) -> &[ComponentValue] {
        &self.0
    }

    /// 取首个 Ident 关键字（`None` 若无 ident token）。
    pub fn keyword(&self) -> Option<&str> {
        self.0.iter().find_map(|cv| match cv {
            ComponentValue::PreservedToken(Token::Ident(s)) => Some(s.as_str()),
            _ => None,
        })
    }
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
