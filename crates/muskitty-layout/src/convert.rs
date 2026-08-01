//! DOM + ComputedStyle → LayoutTree 转换。
//!
//! [`build_layout_tree`] 递归遍历 DOM 树，为每个未被 `display:none` 排除的
//! Element 节点创建 taffy 节点，构建 [`LayoutTree`]。
//!
//! # 规范依据
//!
//! - CSS Display Level 3 §2: box tree generation
//! - CSS Box Model Level 3 §2: box generation

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use muskitty_cascade::{ComputedStyle, ComputedValue};
use muskitty_dom::Node;
use taffy::NodeId;

use crate::style_map;
use crate::tree::LayoutTree;

/// DOM 节点指针地址 → ComputedStyle 的映射类型。
pub type StyleMap = HashMap<usize, ComputedStyle>;

/// 从 DOM 树 + ComputedStyle 表构建 [`LayoutTree`]。
///
/// 递归遍历 DOM：
/// - [`Element`](muskitty_dom::NodeKind::Element) 节点 → 映射 ComputedStyle 为
///   taffy [`Style`](taffy::style::Style) 并创建 taffy 节点。
/// - `display: none` 的元素及其子树被跳过（不生成布局盒）。
/// - Text / Comment / Document 等非元素节点跳过（文本测量推迟到后续批次）。
///
/// DOM 节点的 `Rc` 指针地址（`Rc::as_ptr(node) as usize`）作为 [`LayoutTree::node_map`]
/// 的 key，供布局结果查询时关联回 DOM 节点。
pub fn build_layout_tree(root: &Rc<RefCell<Node>>, styles: &StyleMap) -> LayoutTree {
    let mut tree = LayoutTree::new();
    if let Some(root_id) = build_node_recursive(&mut tree, root, styles) {
        tree.root = Some(root_id);
    }
    tree
}

/// 递归为 DOM 子树构建 taffy 节点。
///
/// 返回 `None` 表示该节点不生成布局盒（非元素或 display:none）。
fn build_node_recursive(
    tree: &mut LayoutTree,
    node: &Rc<RefCell<Node>>,
    styles: &StyleMap,
) -> Option<NodeId> {
    // 先收集节点信息并释放 node 的借用，避免在递归期间持有 RefCell 借用。
    let (addr, computed, children) = {
        let node_ref = node.borrow();
        let kind = &node_ref.kind;
        if !matches!(kind, muskitty_dom::NodeKind::Element(_)) {
            return None;
        }

        let addr = Rc::as_ptr(node) as usize;
        let computed = styles.get(&addr);

        // display: none → 跳过该节点及其整个子树。
        if let Some(cs) = computed {
            if let Some(ComputedValue::Keyword(kw)) = cs.get("display") {
                if kw.eq_ignore_ascii_case("none") {
                    return None;
                }
            }
        }

        // 克隆子节点 Rc 引用，释放 node_ref 借用后再递归。
        let children: Vec<Rc<RefCell<Node>>> = node_ref.child_nodes().to_vec();
        (addr, computed, children)
    };

    let taffy_style = style_map::map_style(computed);

    // 先递归处理子节点，收集已创建的 taffy NodeId 列表。
    let child_ids: Vec<NodeId> = children
        .iter()
        .filter_map(|child| build_node_recursive(tree, child, styles))
        .collect();

    // 根据是否有子节点选择创建方式。
    let taffy_node = if child_ids.is_empty() {
        tree.taffy
            .new_leaf(taffy_style)
            .expect("taffy new_leaf 失败：无法创建叶子布局节点")
    } else {
        tree.taffy
            .new_with_children(taffy_style, &child_ids)
            .expect("taffy new_with_children 失败：无法创建带子节点的布局节点")
    };

    tree.node_map.insert(addr, taffy_node);
    Some(taffy_node)
}
