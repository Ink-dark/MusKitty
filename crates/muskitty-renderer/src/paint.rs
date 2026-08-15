//! paint 函数：LayoutResult + ComputedStyle + DOM → RenderCommand[]。
//!
//! 遍历 DOM 树，对每个有布局结果的元素节点，查询其 `background-color`
//! 并生成对应的 [`RenderCommand::Rect`]。
//!
//! # 坐标系
//!
//! 坐标由 layout 层给出画布坐标系**绝对坐标**（[`NodeLayout::abs_x`] /
//! [`NodeLayout::abs_y`]，沿 taffy 树自根累加，P2-19）。paint 不再沿 DOM
//! 祖先累加偏移——`display: contents` splice 后 DOM 祖先链 ≠ taffy 父链，
//! DOM 累加会在未来 `position: absolute` / transform 下双重计数。

use crate::color::Color;
use crate::command::RenderCommand;
use crate::render_tree::{
    extract_background_color, extract_border, extract_text_color, resolve_font_size,
};
use muskitty_cascade::ComputedStyle;
use muskitty_dom::{Node, NodeKind};
use muskitty_layout::{LayoutResult, NodeLayout};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 绘制输入：DOM 根 + 每元素 computed style + 布局结果。
pub struct PaintInput<'a> {
    /// DOM 树根节点。
    pub dom: &'a Rc<RefCell<Node>>,
    /// DOM 节点指针地址 → ComputedStyle。
    pub styles: &'a HashMap<usize, ComputedStyle>,
    /// 布局计算结果。
    pub layout: &'a LayoutResult,
    /// 视口矩形 `(x, y, width, height)`。`None` = 不剔除。
    ///
    /// 完全位于视口外的元素矩形不生成绘制指令（P3-6）；与视口相交或
    /// 完全在内的保留。剔除只影响本节点指令，不影响子节点递归（后代
    /// 可能落在视口内）。
    pub viewport: Option<(f32, f32, f32, f32)>,
}

/// 执行绘制，生成绘制指令列表。
///
/// 遍历 DOM 树，对每个有布局结果且 `background-color` 非透明的元素
/// 生成一个 [`RenderCommand::Rect`]，坐标为画布绝对坐标。
///
/// 指令顺序为 DOM 先序遍历序（父先于子），后端按序绘制即可。
/// z-index / 层叠上下文排序推迟。
pub fn paint(input: &PaintInput) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    // PERF-11：跨递归层复用的子节点缓冲，避免每层 child_nodes().to_vec()
    // 新建临时 Vec。
    let mut children = Vec::new();
    paint_recursive(
        input.dom,
        input.styles,
        input.layout,
        input.viewport,
        &mut commands,
        &mut children,
        16.0, // 默认 font-size（medium = 16px）
        Color::BLACK,
    );
    commands
}

/// 递归遍历 DOM，累积绘制指令。
///
/// 保留 DOM 先序仅为了指令顺序与 style 查询；坐标一律读
/// [`NodeLayout::abs_x`] / [`NodeLayout::abs_y`]，不再传祖先偏移。
#[allow(clippy::too_many_arguments)]
fn paint_recursive(
    node: &Rc<RefCell<Node>>,
    styles: &HashMap<usize, ComputedStyle>,
    layout: &LayoutResult,
    viewport: Option<(f32, f32, f32, f32)>,
    commands: &mut Vec<RenderCommand>,
    children_scratch: &mut Vec<Rc<RefCell<Node>>>,
    inherited_font_size: f32,
    inherited_color: Color,
) {
    let addr = Rc::as_ptr(node) as usize;

    // 本节点的继承上下文：Element 从自身 style 解析 color/font-size，
    // 其余节点（Text/Comment/...）沿用继承值。
    let (font_size, color) = {
        let node_ref = node.borrow();
        match &node_ref.kind {
            NodeKind::Element(_) => {
                let fs = styles
                    .get(&addr)
                    .and_then(resolve_font_size)
                    .unwrap_or(inherited_font_size);
                let c = styles
                    .get(&addr)
                    .map(extract_text_color)
                    .unwrap_or(inherited_color);
                (fs, c)
            }
            _ => (inherited_font_size, inherited_color),
        }
    };

    // 按节点类型生成绘制指令。
    {
        let node_ref = node.borrow();
        match &node_ref.kind {
            // Text 节点 → Text 命令（T-2）。text 无自身 style，用继承上下文。
            // 纯空白文本（HTML 缩进/换行）不产生可见墨迹，跳过（white-space
            // 折叠的完整语义推迟到 T-3）。
            NodeKind::Text(text) if !text.data.trim().is_empty() => {
                if let Some(node_layout) = layout.get(addr).filter(|l| in_viewport(l, viewport)) {
                    commands.push(RenderCommand::Text {
                        x: node_layout.abs_x,
                        y: node_layout.abs_y,
                        text: text.data.clone(),
                        font_size,
                        color,
                    });
                }
            }
            // Element 节点 → Rect 命令（背景 + 边框）。
            NodeKind::Element(_) => {
                // 查询布局结果；display:none / contents / 非渲染标签不在布局
                // 树中（或无盒），自然跳过。
                if let Some(node_layout) = layout.get(addr) {
                    if in_viewport(node_layout, viewport) {
                        if let Some(style) = styles.get(&addr) {
                            let bg =
                                extract_background_color(style).filter(|c| !c.is_transparent());
                            let border = extract_border(style);

                            // 有背景或边框时生成绘制指令（绝对坐标）。
                            if bg.is_some() || border.is_some() {
                                commands.push(RenderCommand::Rect {
                                    x: node_layout.abs_x,
                                    y: node_layout.abs_y,
                                    width: node_layout.width,
                                    height: node_layout.height,
                                    background: bg,
                                    border,
                                });
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // 递归子节点（无论本节点是否绘制；display:contents 等无盒元素的后代
    // 仍需在 DOM 先序中遍历到）。PERF-11：子节点收集进复用缓冲，`take`
    // 为本次局部分片（递归期间缓冲保持空给子层复用）；递归结束后归还，
    // 让最深层的分配 capacity 被最外层持续复用。
    children_scratch.clear();
    children_scratch.extend(node.borrow().child_nodes().iter().cloned());
    let children = std::mem::take(children_scratch);
    for child in &children {
        paint_recursive(
            child,
            styles,
            layout,
            viewport,
            commands,
            children_scratch,
            font_size,
            color,
        );
    }
    *children_scratch = children;
}

/// P3-6: viewport culling —— 完全位于视口外的盒跳过绘制。
fn in_viewport(node_layout: &NodeLayout, viewport: Option<(f32, f32, f32, f32)>) -> bool {
    match viewport {
        Some((vx, vy, vw, vh)) => {
            !(node_layout.abs_x + node_layout.width <= vx
                || node_layout.abs_y + node_layout.height <= vy
                || node_layout.abs_x >= vx + vw
                || node_layout.abs_y >= vy + vh)
        }
        None => true,
    }
}

// 单元测试见 tests/paint.rs（使用 muskitty-html5-parser 构造真实 DOM）。
