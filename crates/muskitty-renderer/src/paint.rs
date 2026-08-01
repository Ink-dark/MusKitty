//! paint 函数：LayoutResult + ComputedStyle + DOM → RenderCommand[]。
//!
//! 遍历 DOM 树，对每个有布局结果的元素节点，查询其 `background-color`
//! 并生成对应的 [`RenderCommand::Rect`]。
//!
//! # 坐标系
//!
//! `LayoutResult` 中每个 `NodeLayout` 的 `x/y` 是相对父元素原点的偏移。
//! paint 阶段递归累加父元素偏移，把所有坐标转换到画布坐标系（原点在
//! 视口左上角）。

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
/// 生成一个 [`RenderCommand::Rect`]。
///
/// 指令顺序为 DOM 先序遍历序（父先于子），后端按序绘制即可。
/// z-index / 层叠上下文排序推迟。
pub fn paint(input: &PaintInput) -> Vec<RenderCommand> {
    let mut commands = Vec::new();
    paint_recursive(
        input.dom,
        input.styles,
        input.layout,
        0.0,
        0.0,
        &mut commands,
    );
    commands
}

/// 递归遍历 DOM，累积绘制指令。
///
/// `offset_x` / `offset_y` 为当前元素相对画布原点的累计偏移。
fn paint_recursive(
    node: &Rc<RefCell<Node>>,
    styles: &HashMap<usize, ComputedStyle>,
    layout: &LayoutResult,
    offset_x: f32,
    offset_y: f32,
    commands: &mut Vec<RenderCommand>,
) {
    let addr = Rc::as_ptr(node) as usize;

    // 查询布局结果；display:none / 非元素节点不在布局树中，自然跳过。
    let node_layout = match layout.get(addr) {
        Some(l) => l,
        None => {
            // 仍需递归子节点（Document / 非布局容器可能有无布局的子元素
            // 仍需遍历找到可绘制的后代）
            let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
            for child in &children {
                paint_recursive(child, styles, layout, offset_x, offset_y, commands);
            }
            return;
        }
    };

    // 仅 Element 节点生成绘制指令。
    let is_element = matches!(node.borrow().kind, NodeKind::Element(_));
    if is_element {
        // 画布坐标 = 累计偏移 + 当前节点相对父的偏移
        let canvas_x = offset_x + node_layout.x;
        let canvas_y = offset_y + node_layout.y;

        // 查询 computed style，提取 background-color 与 border
        if let Some(style) = styles.get(&addr) {
            let bg = extract_background_color(style).filter(|c| !c.is_transparent());
            let border = extract_border(style);

            // 有背景或边框时生成绘制指令
            if bg.is_some() || border.is_some() {
                commands.push(RenderCommand::Rect {
                    x: canvas_x,
                    y: canvas_y,
                    width: node_layout.width,
                    height: node_layout.height,
                    background: bg,
                    border,
                });
            }
        }

        // 递归子节点时，把当前节点的画布坐标作为新的偏移基准
        let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
        for child in &children {
            paint_recursive(child, styles, layout, canvas_x, canvas_y, commands);
        }
    } else {
        // 非元素节点（Text/Comment 等）：不绘制，但递归子节点保持偏移不变
        let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
        for child in &children {
            paint_recursive(child, styles, layout, offset_x, offset_y, commands);
        }
    }
}

// 单元测试见 tests/paint.rs（使用 muskitty-html5-parser 构造真实 DOM）。
