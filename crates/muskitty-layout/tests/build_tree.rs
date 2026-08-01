//! DOM → LayoutTree 构建测试。
//!
//! 覆盖：单元素、嵌套元素、Text 节点跳过、display:none 排除。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use muskitty_cascade::{ComputedStyle, ComputedValue};
use muskitty_dom::{append_child, Node};
use muskitty_layout::build_layout_tree;

/// 创建 Document + 一个指定 tag 的 Element。
fn make_element(tag: &str, doc: &Rc<RefCell<Node>>) -> Rc<RefCell<Node>> {
    Node::new_element_html(tag, vec![], doc)
}

#[test]
fn single_element_builds_one_node() {
    let doc = Node::new_document();
    let root = make_element("div", &doc);

    let styles: HashMap<usize, ComputedStyle> = HashMap::new();
    let tree = build_layout_tree(&root, &styles);

    assert!(tree.root.is_some(), "根节点应被创建");
    assert_eq!(tree.node_map.len(), 1, "单个元素应映射到 1 个布局节点");
}

#[test]
fn nested_elements_build_tree() {
    // div > p > span
    let doc = Node::new_document();
    let root = make_element("div", &doc);
    let p = make_element("p", &doc);
    let span = make_element("span", &doc);
    append_child(&root, Rc::clone(&p)).unwrap();
    append_child(&p, Rc::clone(&span)).unwrap();

    let styles: HashMap<usize, ComputedStyle> = HashMap::new();
    let tree = build_layout_tree(&root, &styles);

    assert_eq!(tree.node_map.len(), 3, "三层嵌套应产生 3 个布局节点");
    assert!(tree.node_map.contains_key(&(Rc::as_ptr(&root) as usize)));
    assert!(tree.node_map.contains_key(&(Rc::as_ptr(&p) as usize)));
    assert!(tree.node_map.contains_key(&(Rc::as_ptr(&span) as usize)));
}

#[test]
fn text_nodes_skipped() {
    // div > "hello"
    let doc = Node::new_document();
    let root = make_element("div", &doc);
    let text = Node::new_text("hello", &doc);
    append_child(&root, text).unwrap();

    let styles: HashMap<usize, ComputedStyle> = HashMap::new();
    let tree = build_layout_tree(&root, &styles);

    assert_eq!(tree.node_map.len(), 1, "Text 节点不应创建布局节点");
}

#[test]
fn display_none_excluded() {
    // div > p[display:none]
    let doc = Node::new_document();
    let root = make_element("div", &doc);
    let p = make_element("p", &doc);
    append_child(&root, Rc::clone(&p)).unwrap();

    let mut styles: HashMap<usize, ComputedStyle> = HashMap::new();
    let p_addr = Rc::as_ptr(&p) as usize;
    let mut p_style = ComputedStyle::new();
    p_style.set("display", ComputedValue::Keyword("none".to_string()));
    styles.insert(p_addr, p_style);

    let tree = build_layout_tree(&root, &styles);

    assert_eq!(tree.node_map.len(), 1, "display:none 的 p 应被排除");
    assert!(
        !tree.node_map.contains_key(&p_addr),
        "p 的地址不应出现在 node_map 中"
    );
}

#[test]
fn display_none_excludes_entire_subtree() {
    // div > p[display:none] > span + em
    let doc = Node::new_document();
    let root = make_element("div", &doc);
    let p = make_element("p", &doc);
    let span = make_element("span", &doc);
    let em = make_element("em", &doc);
    append_child(&root, Rc::clone(&p)).unwrap();
    append_child(&p, Rc::clone(&span)).unwrap();
    append_child(&p, Rc::clone(&em)).unwrap();

    let mut styles: HashMap<usize, ComputedStyle> = HashMap::new();
    let mut p_style = ComputedStyle::new();
    p_style.set("display", ComputedValue::Keyword("none".to_string()));
    styles.insert(Rc::as_ptr(&p) as usize, p_style);

    let tree = build_layout_tree(&root, &styles);

    assert_eq!(tree.node_map.len(), 1, "display:none 的整个子树应被排除");
    assert!(!tree.node_map.contains_key(&(Rc::as_ptr(&span) as usize)));
    assert!(!tree.node_map.contains_key(&(Rc::as_ptr(&em) as usize)));
}

#[test]
fn empty_dom_produces_empty_tree() {
    // Document 节点（非 Element）→ 空树
    let doc = Node::new_document();
    let styles: HashMap<usize, ComputedStyle> = HashMap::new();
    let tree = build_layout_tree(&doc, &styles);

    assert!(tree.root.is_none(), "Document 节点不应产生布局根");
    assert_eq!(tree.node_map.len(), 0);
}
