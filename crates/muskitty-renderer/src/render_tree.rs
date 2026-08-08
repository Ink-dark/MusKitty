//! 渲染树（RenderTree）。
//!
//! 渲染树是 DOM + ComputedStyle + LayoutResult 的「投影」：保留
//! 需要绘制的元素节点（跳过 `display: none` 与非元素节点），携带
//! 绘制所需的样式信息与布局结果。
//!
//! 当前实现：`paint` 直接输出 `Vec<RenderCommand>`，RenderTree
//! 作为中间结构保留供后续复杂场景（z-order / 层叠上下文 / transform
//! 嵌套）使用。B-1 阶段 paint 直接生成命令，暂不构造 RenderTree。

use crate::color::Color;
use crate::command::{Border, BorderStyle};
use muskitty_cascade::ComputedStyle;
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;
use muskitty_layout::NodeLayout;

/// 渲染节点：一个需要绘制的元素 + 其样式与布局信息。
#[derive(Debug, Clone)]
pub struct RenderNode {
    /// DOM 节点指针地址（与 LayoutResult 的 key 一致）。
    pub node_addr: usize,
    /// 元素的布局结果（位置与尺寸）。
    pub layout: NodeLayout,
    /// 元素的 computed style（用于查询 background-color 等）。
    pub style: ComputedStyle,
    /// 子渲染节点。
    pub children: Vec<RenderNode>,
}

/// 渲染树（根节点）。
#[derive(Debug, Clone, Default)]
pub struct RenderTree {
    /// 根渲染节点（可能为空）。
    pub root: Option<RenderNode>,
}

impl RenderTree {
    /// 创建空渲染树。
    pub fn new() -> Self {
        Self::default()
    }
}

/// 从 ComputedStyle 提取 background-color。
///
/// 未设置或无法解析时返回 `None`（调用方按透明处理）。单态化（P2-20）后
/// 值统一为 token 序列，`parse_color` 同时覆盖命名色/hex/rgb 函数与
/// `transparent`（`parse_named_color` 内含），无需再按来源分支。
pub fn extract_background_color(style: &ComputedStyle) -> Option<Color> {
    let cv = style.get("background-color")?;
    crate::color::parse_color(cv.tokens())
}

/// 从 ComputedStyle 提取边框。
///
/// 读取 `border-width` / `border-style` / `border-color` 三个 longhand
/// 属性。`border` 简写需要 CSSOM 层展开，当前不支持（推迟）。
///
/// 返回 `None` 表示无边框（未设置 / 样式为 none / 宽度为 0）。
pub fn extract_border(style: &ComputedStyle) -> Option<Border> {
    // border-style（默认 none）
    let style_val = parse_border_style(style)?;
    if style_val == BorderStyle::None {
        return None;
    }

    // border-width（默认 0，当前仅解析 px）
    let width = parse_border_width(style)?;
    if width <= 0.0 {
        return None;
    }

    // border-color（默认 currentColor，当前回退到黑色）
    let color = parse_border_color(style).unwrap_or(Color::BLACK);

    Some(Border {
        width,
        color,
        style: style_val,
    })
}

/// 解析 `border-style` 关键字。
fn parse_border_style(style: &ComputedStyle) -> Option<BorderStyle> {
    let cv = style.get("border-style")?;
    let kw = cv.keyword()?;
    match kw.to_ascii_lowercase().as_str() {
        "none" => Some(BorderStyle::None),
        "solid" => Some(BorderStyle::Solid),
        "dashed" => Some(BorderStyle::Dashed),
        "dotted" => Some(BorderStyle::Dotted),
        _ => None,
    }
}

/// 解析 `border-width` 为 px 浮点值。
///
/// 当前仅支持 `<length>` 的 px 单位；其他单位（em/rem/pt）推迟。
fn parse_border_width(style: &ComputedStyle) -> Option<f32> {
    let cv = style.get("border-width")?;
    // 取首个 dimension token
    for v in cv.tokens() {
        if let ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) = v {
            if unit.eq_ignore_ascii_case("px") {
                return Some(numeric.value as f32);
            }
            // 非 px 单位推迟
        }
    }
    None
}

/// 解析 `border-color`。
fn parse_border_color(style: &ComputedStyle) -> Option<Color> {
    let cv = style.get("border-color")?;
    crate::color::parse_color(cv.tokens())
}
