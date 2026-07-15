//! DOM 树操作（mutation）算法。
//!
//! 实现 DOM Living Standard §4.2.6 的树修改算法：
//! `insert`、`append`、`replace`、`pre-remove` 等。
//! 这些函数是 `appendChild`/`insertBefore`/`removeChild`/`replaceChild`
//! 的底层实现。

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::error::DomError;
use crate::node::{Node, NodeType};

/// `Node.textContent` setter（DOM §4.4）。
///
/// 清空所有子节点；若 `value` 非空，则创建一个 Text 节点作为唯一子节点。
pub fn set_text_content(node: &Rc<RefCell<Node>>, value: &str) {
    // 清空所有子节点（解除它们的 parent 引用）
    let children: Vec<Rc<RefCell<Node>>> = node.borrow_mut().children.drain(..).collect();
    for c in children {
        c.borrow_mut().parent_node = Weak::new();
    }

    // 若 value 非空且节点有 owner_document，创建 Text 节点
    if !value.is_empty() {
        // 分离 borrow 到独立语句，确保 Ref 在 borrow_mut 前释放
        let doc = node.borrow().owner_document.upgrade();
        if let Some(doc) = doc {
            let text = Node::new_text(value, &doc);
            text.borrow_mut().parent_node = Rc::downgrade(node);
            node.borrow_mut().children.push(text);
        }
    }
}

/// `Node.appendChild(child)`。参见 DOM §4.2.6。
///
/// 将 `child` 追加为 `parent` 的最后一个子节点。
pub fn append_child(
    parent: &Rc<RefCell<Node>>,
    child: Rc<RefCell<Node>>,
) -> Result<Rc<RefCell<Node>>, DomError> {
    insert_before(parent, child, None)
}

/// `Node.insertBefore(node, child)`。参见 DOM §4.2.6。
///
/// 将 `node` 插入到 `reference` 之前；`reference` 为 `None` 时追加到末尾。
pub fn insert_before(
    parent: &Rc<RefCell<Node>>,
    node: Rc<RefCell<Node>>,
    reference: Option<&Rc<RefCell<Node>>>,
) -> Result<Rc<RefCell<Node>>, DomError> {
    ensure_pre_insert_validity(parent, &node, reference)?;

    // 定位 reference 在 parent.children 中的索引
    let ref_idx = match reference {
        Some(r) => parent
            .borrow()
            .children
            .iter()
            .position(|c| Rc::ptr_eq(c, r)),
        None => None,
    };

    // 若 node 已有父节点，先从原父节点移除
    // 分离 borrow 到独立语句，确保 Ref 在 remove_child_internal 的 borrow_mut 前释放
    let old_parent = node.borrow().parent_node.upgrade();
    if let Some(old_parent) = old_parent {
        remove_child_internal(&old_parent, &node);
    }

    // 插入
    let idx = ref_idx.unwrap_or_else(|| parent.borrow().children.len());
    node.borrow_mut().parent_node = Rc::downgrade(parent);
    parent.borrow_mut().children.insert(idx, node.clone());
    Ok(node)
}

/// `Node.removeChild(child)`。参见 DOM §4.2.6.
pub fn remove_child(
    parent: &Rc<RefCell<Node>>,
    child: &Rc<RefCell<Node>>,
) -> Result<Rc<RefCell<Node>>, DomError> {
    let idx = parent
        .borrow()
        .children
        .iter()
        .position(|c| Rc::ptr_eq(c, child))
        .ok_or_else(|| DomError::NotFound("node is not a child of parent".into()))?;

    let removed = parent.borrow_mut().children.remove(idx);
    removed.borrow_mut().parent_node = Weak::new();
    Ok(removed)
}

/// `Node.replaceChild(new, old)`。参见 DOM §4.2.6.
pub fn replace_child(
    parent: &Rc<RefCell<Node>>,
    new_child: Rc<RefCell<Node>>,
    old_child: &Rc<RefCell<Node>>,
) -> Result<Rc<RefCell<Node>>, DomError> {
    let idx = parent
        .borrow()
        .children
        .iter()
        .position(|c| Rc::ptr_eq(c, old_child))
        .ok_or_else(|| DomError::NotFound("old_child is not a child of parent".into()))?;

    // 若 new_child 已有父节点，先从原父移除
    // 分离 borrow 到独立语句，确保 Ref 在 remove_child_internal 的 borrow_mut 前释放
    let old_parent = new_child.borrow().parent_node.upgrade();
    if let Some(old_parent) = old_parent {
        remove_child_internal(&old_parent, &new_child);
    }

    // 替换
    let old = parent.borrow_mut().children[idx].clone();
    old.borrow_mut().parent_node = Weak::new();
    new_child.borrow_mut().parent_node = Rc::downgrade(parent);
    parent.borrow_mut().children[idx] = new_child;
    Ok(old)
}

/// 内部使用的移除（不报错，找不到则静默）。
fn remove_child_internal(parent: &Rc<RefCell<Node>>, child: &Rc<RefCell<Node>>) {
    let idx = parent
        .borrow()
        .children
        .iter()
        .position(|c| Rc::ptr_eq(c, child));
    if let Some(i) = idx {
        let removed = parent.borrow_mut().children.remove(i);
        removed.borrow_mut().parent_node = Weak::new();
    }
}

/// `ensure pre-insertion validity`。参见 DOM §4.2.6.
fn ensure_pre_insert_validity(
    parent: &Rc<RefCell<Node>>,
    node: &Rc<RefCell<Node>>,
    reference: Option<&Rc<RefCell<Node>>>,
) -> Result<(), DomError> {
    // 1. 若 node 是 parent 或 parent 的祖先，HierarchyRequestError
    if Rc::ptr_eq(node, parent) {
        return Err(DomError::HierarchyRequest(
            "cannot insert a node as its own child".into(),
        ));
    }
    let mut ancestor = parent.borrow().parent_node.upgrade();
    while let Some(a) = ancestor {
        if Rc::ptr_eq(&a, node) {
            return Err(DomError::HierarchyRequest(
                "cannot insert an ancestor as a child".into(),
            ));
        }
        ancestor = a.borrow().parent_node.upgrade();
    }

    // 2. 若 reference 不为 None 且不是 parent 的子节点，NotFoundError
    if let Some(r) = reference {
        if !parent.borrow().children.iter().any(|c| Rc::ptr_eq(c, r)) {
            return Err(DomError::NotFound(
                "reference is not a child of parent".into(),
            ));
        }
    }

    // 3. Document 父节点的特殊校验（DOM §4.2.6 步骤 5）
    if parent.borrow().node_type == NodeType::Document {
        ensure_document_child_validity(parent, node, reference)?;
    }

    Ok(())
}

/// Document 作为父节点时的额外校验。
///
/// 简化版：Document 只允许 Element/DocumentType/Comment/ProcessingInstruction
/// 作为子节点，且最多一个 Element 和一个 DocumentType。
/// 完整规范见 DOM §4.2.6 步骤 5。
fn ensure_document_child_validity(
    parent: &Rc<RefCell<Node>>,
    node: &Rc<RefCell<Node>>,
    reference: Option<&Rc<RefCell<Node>>>,
) -> Result<(), DomError> {
    let node_type = node.borrow().node_type;
    // Document 不允许 Document/DocumentFragment/Text 作为直接子节点
    match node_type {
        NodeType::Document | NodeType::DocumentFragment | NodeType::Text => {
            return Err(DomError::HierarchyRequest(format!(
                "Document cannot have a {:?} child",
                node_type
            )));
        }
        _ => {}
    }

    // 若 node 是 Element：检查是否已有 Element 子节点
    if node_type == NodeType::Element {
        let has_element = parent
            .borrow()
            .children
            .iter()
            .any(|c| c.borrow().node_type == NodeType::Element);
        if has_element {
            return Err(DomError::HierarchyRequest(
                "Document already has an element child".into(),
            ));
        }
    }

    // 若 node 是 DocumentType：检查是否已有 DocumentType 子节点
    if node_type == NodeType::DocumentType {
        let has_doctype = parent
            .borrow()
            .children
            .iter()
            .any(|c| c.borrow().node_type == NodeType::DocumentType);
        if has_doctype {
            return Err(DomError::HierarchyRequest(
                "Document already has a doctype child".into(),
            ));
        }
    }

    // reference 的位置校验（简化）：省略规范中关于 Element/DocumentType 顺序的复杂规则
    let _ = reference;
    Ok(())
}
