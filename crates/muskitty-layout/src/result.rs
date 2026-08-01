//! 布局计算结果提取。
//!
//! [`LayoutResult`] 是 [`compute_layout`](crate::compute_layout) 的输出，
//! 按 DOM 节点指针地址索引每个元素的位置与尺寸。

use std::collections::HashMap;

/// 单个元素的布局结果。
///
/// 所有坐标均为相对父元素原点的偏移（px）。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct NodeLayout {
    /// 相对父元素原点的 X 坐标（px）。
    pub x: f32,
    /// 相对父元素原点的 Y 坐标（px）。
    pub y: f32,
    /// 计算后的宽度（px）。
    pub width: f32,
    /// 计算后的高度（px）。
    pub height: f32,
}

/// 整棵布局树的结果集合。
///
/// 通过 DOM 节点指针地址（`usize`）查询对应元素的 [`NodeLayout`]。
#[derive(Debug, Default)]
pub struct LayoutResult {
    /// DOM 节点指针地址 → 布局结果。
    pub nodes: HashMap<usize, NodeLayout>,
}

impl LayoutResult {
    /// 创建空结果集。
    pub fn new() -> Self {
        Self::default()
    }

    /// 按 DOM 节点指针地址查询布局结果。
    pub fn get(&self, node_addr: usize) -> Option<&NodeLayout> {
        self.nodes.get(&node_addr)
    }
}
