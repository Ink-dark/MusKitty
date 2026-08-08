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

use crate::command::RenderCommand;
use crate::render_tree::{extract_background_color, extract_border};
use muskitty_cascade::ComputedStyle;
use muskitty_dom::{Node, NodeKind};
use muskitty_layout::LayoutResult;
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
    paint_recursive(input.dom, input.styles, input.layout, &mut commands);
    commands
}

/// 递归遍历 DOM，累积绘制指令。
///
/// 保留 DOM 先序仅为了指令顺序与 style 查询；坐标一律读
/// [`NodeLayout::abs_x`] / [`NodeLayout::abs_y`]，不再传祖先偏移。
fn paint_recursive(
    node: &Rc<RefCell<Node>>,
    styles: &HashMap<usize, ComputedStyle>,
    layout: &LayoutResult,
    commands: &mut Vec<RenderCommand>,
) {
    let addr = Rc::as_ptr(node) as usize;

    // 仅 Element 节点生成绘制指令。
    if matches!(node.borrow().kind, NodeKind::Element(_)) {
        // 查询布局结果；display:none / contents / 非渲染标签不在布局树中
        // （或无盒），自然跳过。
        if let Some(node_layout) = layout.get(addr) {
            if let Some(style) = styles.get(&addr) {
                let bg = extract_background_color(style).filter(|c| !c.is_transparent());
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

    // 递归子节点（无论本节点是否绘制；display:contents 等无盒元素的后代
    // 仍需在 DOM 先序中遍历到）。
    let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
    for child in &children {
        paint_recursive(child, styles, layout, commands);
    }
}

// 单元测试见 tests/paint.rs（使用 muskitty-html5-parser 构造真实 DOM）。
