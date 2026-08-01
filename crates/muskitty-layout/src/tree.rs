//! 布局树类型定义。
//!
//! [`LayoutTree`] 包装 [`taffy::TaffyTree`]，维护 DOM 节点指针地址 →
//! taffy [`NodeId`] 的映射，供布局计算后按 DOM 节点查询结果。
//!
//! # 规范依据
//!
//! - CSS Display Module Level 3 §2 (Box Tree)
//! - CSS Box Model Module Level 3 §2 (Box Model)

use std::collections::HashMap;

use taffy::NodeId;
use taffy::TaffyTree;

/// 布局树。
///
/// 包装 taffy 的 [`TaffyTree`]，并维护 DOM 节点指针地址（`usize`）
/// 到 taffy [`NodeId`] 的映射。构建阶段由 [`build_layout_tree`](crate::build_layout_tree)
/// 填充；计算阶段由 [`compute_layout`](crate::compute_layout) 读取。
pub struct LayoutTree {
    /// taffy 的内部节点树（默认 context 类型为 `()`）。
    pub taffy: TaffyTree,
    /// DOM 节点指针地址 → taffy NodeId 映射。
    pub node_map: HashMap<usize, NodeId>,
    /// 根节点 ID（若 DOM 根为 Element 且未 display:none 则有值）。
    pub root: Option<NodeId>,
}

impl LayoutTree {
    /// 创建空布局树。
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            node_map: HashMap::new(),
            root: None,
        }
    }
}

impl Default for LayoutTree {
    fn default() -> Self {
        Self::new()
    }
}
