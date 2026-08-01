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
use muskitty_cascade::ComputedStyle;
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
/// 未设置或无法解析时返回 `None`（调用方按透明处理）。
pub fn extract_background_color(style: &ComputedStyle) -> Option<Color> {
    let cv = style.get("background-color")?;
    match cv {
        muskitty_cascade::ComputedValue::Resolved(values)
        | muskitty_cascade::ComputedValue::Raw(values) => crate::color::parse_color(values),
        muskitty_cascade::ComputedValue::Keyword(kw) => {
            // 关键字值：可能是 "transparent" 或命名颜色
            if kw.eq_ignore_ascii_case("transparent") {
                Some(Color::TRANSPARENT)
            } else {
                crate::color::parse_named_color(kw)
            }
        }
    }
}
