//! MusKitty Layout — CSS 布局引擎。
//!
//! 将 DOM 树 + ComputedStyle 转换为布局盒树，
//! 使用 taffy 引擎计算 Flexbox/Grid/Block 布局，
//! 输出每个元素的位置和尺寸。
//!
//! # 数据流
//!
//! ```text
//! DOM 树 + ComputedStyle per element
//!     │  build_layout_tree
//!     ▼
//! LayoutTree (taffy TaffyTree + NodeId 映射)
//!     │  compute_layout
//!     ▼
//! LayoutResult (per-element x/y/width/height)
//! ```
//!
//! # 规范依据
//!
//! - CSS Display Level 3: box tree / formatting context
//! - CSS Box Model Level 3: margin/border/padding/content
//! - CSS Flexbox Level 1: flex container/item
//! - CSS Grid Level 2: grid container/item

pub mod convert;
pub mod result;
pub mod style_map;
pub mod tree;

pub use convert::build_layout_tree;
pub use result::{LayoutResult, NodeLayout};
pub use tree::LayoutTree;

use taffy::geometry::Size;
use taffy::style::AvailableSpace;

/// 计算布局树，返回每个元素的布局结果。
///
/// `viewport_width` / `viewport_height` 为根容器的可用空间（px），
/// 作为 taffy 布局计算的 definite available space。
///
/// 调用后 [`LayoutTree::taffy`] 内部缓存了每个节点的 [`taffy::tree::Layout`]，
/// 本函数遍历 `node_map` 提取位置与尺寸到 [`LayoutResult`]。
pub fn compute_layout(
    tree: &mut LayoutTree,
    viewport_width: f32,
    viewport_height: f32,
) -> LayoutResult {
    let mut result = LayoutResult::new();

    if let Some(root) = tree.root {
        tree.taffy
            .compute_layout(
                root,
                Size {
                    width: AvailableSpace::Definite(viewport_width),
                    height: AvailableSpace::Definite(viewport_height),
                },
            )
            .expect("taffy compute_layout 失败：布局计算出错");

        for (&dom_addr, &taffy_node) in &tree.node_map {
            let layout = tree
                .taffy
                .layout(taffy_node)
                .expect("taffy layout 查询失败：节点不在布局树中");
            result.nodes.insert(
                dom_addr,
                NodeLayout {
                    x: layout.location.x,
                    y: layout.location.y,
                    width: layout.size.width,
                    height: layout.size.height,
                },
            );
        }
    }

    result
}
