//! DOM Core 类型的集成测试。
//!
//! 覆盖节点构造、树操作（append/insert/remove/replace）、
//! 遍历 API、text_content、descendants 以及错误路径。

use std::rc::Rc;

use muskitty_dom::{
    append_child, insert_before, remove_child, replace_child, set_text_content, Attribute,
    DomError, Namespace, Node, NodeType,
};

// —— 构造函数 ——

#[test]
fn new_document_metadata() {
    let doc = Node::new_document();
    let d = doc.borrow();
    assert_eq!(d.node_type, NodeType::Document);
    assert_eq!(d.node_name, "#document");
    assert!(!d.has_child_nodes());
    assert_eq!(d.child_count(), 0);
    assert!(d.parent_node().is_none());
    assert!(d.parent_element().is_none());
    assert!(d.first_child().is_none());
    assert!(d.last_child().is_none());
    // owner_document 指向自身
    let owner = d.owner_document.upgrade().unwrap();
    assert!(Rc::ptr_eq(&owner, &doc));
}

#[test]
fn new_element_html_normalizes_tag_name() {
    let doc = Node::new_document();
    let el = Node::new_element_html("DIV", vec![], &doc);
    let e = el.borrow();
    assert_eq!(e.node_type, NodeType::Element);
    assert_eq!(e.node_name, "DIV"); // HTML namespace: uppercase node_name
    assert_eq!(
        e.kind.as_element().unwrap().local_name,
        "div" // local_name 小写
    );
    assert_eq!(e.kind.as_element().unwrap().namespace, Namespace::Html);
    // owner_document 指向 doc
    let owner = e.owner_document.upgrade().unwrap();
    assert!(Rc::ptr_eq(&owner, &doc));
}

#[test]
fn new_element_html_preserves_attributes() {
    let doc = Node::new_document();
    let attrs = vec![
        Attribute::new("class", "container"),
        Attribute::new("id", "main"),
    ];
    let el = Node::new_element_html("div", attrs, &doc);
    let e = el.borrow();
    let elem = e.kind.as_element().unwrap();
    assert_eq!(elem.attributes.len(), 2);
    assert_eq!(elem.get_attribute("class"), Some("container"));
    assert_eq!(elem.get_attribute("CLASS"), Some("container")); // 大小写不敏感
    assert_eq!(elem.get_attribute("id"), Some("main"));
    assert_eq!(elem.get_attribute("missing"), None);
}

#[test]
fn new_text_node() {
    let doc = Node::new_document();
    let text = Node::new_text("hello world", &doc);
    let t = text.borrow();
    assert_eq!(t.node_type, NodeType::Text);
    assert_eq!(t.node_name, "#text");
    assert_eq!(t.kind.as_text().unwrap().data, "hello world");
    assert_eq!(t.text_content(), Some("hello world".to_string()));
}

#[test]
fn new_comment_node() {
    let doc = Node::new_document();
    let comment = Node::new_comment("a comment", &doc);
    let c = comment.borrow();
    assert_eq!(c.node_type, NodeType::Comment);
    assert_eq!(c.node_name, "#comment");
    assert_eq!(c.kind.as_comment().unwrap().data, "a comment");
    assert_eq!(c.text_content(), Some("a comment".to_string()));
}

#[test]
fn new_document_type_node() {
    let doc = Node::new_document();
    let dt = Node::new_document_type("html", "", "", &doc);
    let d = dt.borrow();
    assert_eq!(d.node_type, NodeType::DocumentType);
    assert_eq!(d.node_name, "html");
    let dt_data = d.kind.as_document_type().unwrap();
    assert_eq!(dt_data.name, "html");
    assert_eq!(dt_data.public_id, "");
    assert_eq!(dt_data.system_id, "");
}

#[test]
fn new_document_fragment_node() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let f = frag.borrow();
    assert_eq!(f.node_type, NodeType::DocumentFragment);
    assert_eq!(f.node_name, "#document-fragment");
}

#[test]
fn new_element_ns_svg() {
    let doc = Node::new_document();
    let el = Node::new_element_ns("svg".to_string(), Namespace::Svg, None, vec![], &doc);
    let e = el.borrow();
    // SVG namespace: node_name 保持原样（不大写）
    assert_eq!(e.node_name, "svg");
    assert_eq!(e.kind.as_element().unwrap().local_name, "svg");
    assert_eq!(e.kind.as_element().unwrap().namespace, Namespace::Svg);
}

// —— append_child ——

#[test]
fn append_child_basic() {
    let doc = Node::new_document();
    let html = Node::new_element_html("html", vec![], &doc);
    append_child(&doc, html.clone()).unwrap();

    assert_eq!(doc.borrow().child_count(), 1);
    assert!(doc.borrow().has_child_nodes());
    assert!(Rc::ptr_eq(&doc.borrow().first_child().unwrap(), &html));
    assert!(Rc::ptr_eq(&doc.borrow().last_child().unwrap(), &html));
    // 子节点的 parent 指向 doc
    assert!(Rc::ptr_eq(&html.borrow().parent_node().unwrap(), &doc));
}

#[test]
fn append_multiple_children_preserves_order() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let html = Node::new_element_html("html", vec![], &doc);
    let head = Node::new_element_html("head", vec![], &doc);
    let body = Node::new_element_html("body", vec![], &doc);
    append_child(&frag, html.clone()).unwrap();
    append_child(&frag, head.clone()).unwrap();
    append_child(&frag, body.clone()).unwrap();

    assert_eq!(frag.borrow().child_count(), 3);
    let children: Vec<Rc<_>> = frag.borrow().children.clone();
    assert!(Rc::ptr_eq(&children[0], &html));
    assert!(Rc::ptr_eq(&children[1], &head));
    assert!(Rc::ptr_eq(&children[2], &body));
}

#[test]
fn append_nested_tree() {
    let doc = Node::new_document();
    let html = Node::new_element_html("html", vec![], &doc);
    let head = Node::new_element_html("head", vec![], &doc);
    let title = Node::new_element_html("title", vec![], &doc);
    let text = Node::new_text("Title", &doc);

    append_child(&doc, html.clone()).unwrap();
    append_child(&html, head.clone()).unwrap();
    append_child(&head, title.clone()).unwrap();
    append_child(&title, text.clone()).unwrap();

    // doc -> html -> head -> title -> text
    assert!(Rc::ptr_eq(&doc.borrow().first_child().unwrap(), &html));
    assert!(Rc::ptr_eq(&html.borrow().first_child().unwrap(), &head));
    assert!(Rc::ptr_eq(&head.borrow().first_child().unwrap(), &title));
    assert!(Rc::ptr_eq(&title.borrow().first_child().unwrap(), &text));
}

// —— insert_before ——

#[test]
fn insert_before_with_reference() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);
    let c = Node::new_element_html("c", vec![], &doc);

    append_child(&frag, a.clone()).unwrap();
    append_child(&frag, c.clone()).unwrap();
    // 在 c 之前插入 b
    insert_before(&frag, b.clone(), Some(&c)).unwrap();

    assert_eq!(frag.borrow().child_count(), 3);
    let children: Vec<Rc<_>> = frag.borrow().children.clone();
    assert!(Rc::ptr_eq(&children[0], &a));
    assert!(Rc::ptr_eq(&children[1], &b));
    assert!(Rc::ptr_eq(&children[2], &c));
}

#[test]
fn insert_before_none_appends() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);

    append_child(&frag, a.clone()).unwrap();
    insert_before(&frag, b.clone(), None).unwrap();

    let children: Vec<Rc<_>> = frag.borrow().children.clone();
    assert!(Rc::ptr_eq(&children[0], &a));
    assert!(Rc::ptr_eq(&children[1], &b));
}

#[test]
fn insert_before_invalid_reference_errors() {
    let doc = Node::new_document();
    let a = Node::new_element_html("a", vec![], &doc);
    let not_a_child = Node::new_element_html("x", vec![], &doc);

    append_child(&doc, a.clone()).unwrap();
    let result = insert_before(
        &doc,
        Node::new_element_html("b", vec![], &doc),
        Some(&not_a_child),
    );
    assert!(matches!(result, Err(DomError::NotFound(_))));
}

// —— remove_child ——

#[test]
fn remove_child_basic() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);

    append_child(&frag, a.clone()).unwrap();
    append_child(&frag, b.clone()).unwrap();

    let removed = remove_child(&frag, &a).unwrap();
    assert!(Rc::ptr_eq(&removed, &a));
    assert_eq!(frag.borrow().child_count(), 1);
    assert!(Rc::ptr_eq(&frag.borrow().first_child().unwrap(), &b));
    // 被移除节点的 parent 已清空
    assert!(a.borrow().parent_node().is_none());
}

#[test]
fn remove_child_not_found_errors() {
    let doc = Node::new_document();
    let not_a_child = Node::new_element_html("x", vec![], &doc);

    let result = remove_child(&doc, &not_a_child);
    assert!(matches!(result, Err(DomError::NotFound(_))));
}

// —— replace_child ——

#[test]
fn replace_child_basic() {
    let doc = Node::new_document();
    let old = Node::new_element_html("old", vec![], &doc);
    let new = Node::new_element_html("new", vec![], &doc);

    append_child(&doc, old.clone()).unwrap();
    let returned = replace_child(&doc, new.clone(), &old).unwrap();

    assert!(Rc::ptr_eq(&returned, &old));
    assert_eq!(doc.borrow().child_count(), 1);
    assert!(Rc::ptr_eq(&doc.borrow().first_child().unwrap(), &new));
    // old 的 parent 已清空
    assert!(old.borrow().parent_node().is_none());
    // new 的 parent 指向 doc
    assert!(Rc::ptr_eq(&new.borrow().parent_node().unwrap(), &doc));
}

#[test]
fn replace_child_not_found_errors() {
    let doc = Node::new_document();
    let not_a_child = Node::new_element_html("x", vec![], &doc);
    let new = Node::new_element_html("new", vec![], &doc);

    let result = replace_child(&doc, new, &not_a_child);
    assert!(matches!(result, Err(DomError::NotFound(_))));
}

// —— 节点移动 ——

#[test]
fn moving_node_between_parents_detaches_from_old() {
    let doc = Node::new_document();
    let parent_a = Node::new_element_html("a", vec![], &doc);
    let parent_b = Node::new_element_html("b", vec![], &doc);
    let child = Node::new_element_html("child", vec![], &doc);

    // parent_a / parent_b 作为独立容器，不挂到 doc（Document 只允许一个 Element 子节点）
    append_child(&parent_a, child.clone()).unwrap();

    assert_eq!(parent_a.borrow().child_count(), 1);

    // 将 child 从 parent_a 移到 parent_b
    append_child(&parent_b, child.clone()).unwrap();

    // parent_a 不再有 child
    assert_eq!(parent_a.borrow().child_count(), 0);
    // parent_b 有 child
    assert_eq!(parent_b.borrow().child_count(), 1);
    assert!(Rc::ptr_eq(
        &parent_b.borrow().first_child().unwrap(),
        &child
    ));
    // child 的 parent 指向 parent_b
    assert!(Rc::ptr_eq(
        &child.borrow().parent_node().unwrap(),
        &parent_b
    ));
}

// —— 遍历 API ——

#[test]
fn first_last_child() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);
    let c = Node::new_element_html("c", vec![], &doc);

    append_child(&frag, a.clone()).unwrap();
    append_child(&frag, b.clone()).unwrap();
    append_child(&frag, c.clone()).unwrap();

    assert!(Rc::ptr_eq(&frag.borrow().first_child().unwrap(), &a));
    assert!(Rc::ptr_eq(&frag.borrow().last_child().unwrap(), &c));
}

#[test]
fn siblings_middle_child() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);
    let c = Node::new_element_html("c", vec![], &doc);

    append_child(&frag, a.clone()).unwrap();
    append_child(&frag, b.clone()).unwrap();
    append_child(&frag, c.clone()).unwrap();

    // b 的前一个是 a，后一个是 c
    assert!(Rc::ptr_eq(&b.borrow().previous_sibling().unwrap(), &a));
    assert!(Rc::ptr_eq(&b.borrow().next_sibling().unwrap(), &c));
}

#[test]
fn siblings_first_and_last() {
    let doc = Node::new_document();
    let frag = Node::new_document_fragment(&doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);

    append_child(&frag, a.clone()).unwrap();
    append_child(&frag, b.clone()).unwrap();

    // a 没有前一个
    assert!(a.borrow().previous_sibling().is_none());
    // b 没有后一个
    assert!(b.borrow().next_sibling().is_none());
}

#[test]
fn siblings_only_child() {
    let doc = Node::new_document();
    let a = Node::new_element_html("a", vec![], &doc);
    append_child(&doc, a.clone()).unwrap();

    assert!(a.borrow().previous_sibling().is_none());
    assert!(a.borrow().next_sibling().is_none());
}

#[test]
fn parent_element_vs_parent_node() {
    let doc = Node::new_document();
    let html = Node::new_element_html("html", vec![], &doc);
    let body = Node::new_element_html("body", vec![], &doc);
    let text = Node::new_text("x", &doc);

    append_child(&doc, html.clone()).unwrap();
    append_child(&html, body.clone()).unwrap();
    append_child(&body, text.clone()).unwrap();

    // body 的 parent_node 和 parent_element 都是 html（Element）
    let body_parent_node = body.borrow().parent_node().unwrap();
    let body_parent_el = body.borrow().parent_element().unwrap();
    assert!(Rc::ptr_eq(&body_parent_node, &html));
    assert!(Rc::ptr_eq(&body_parent_el, &html));

    // text 的 parent_node 是 body，parent_element 也是 body
    let text_parent_node = text.borrow().parent_node().unwrap();
    let text_parent_el = text.borrow().parent_element().unwrap();
    assert!(Rc::ptr_eq(&text_parent_node, &body));
    assert!(Rc::ptr_eq(&text_parent_el, &body));

    // html 的 parent_node 是 doc，parent_element 是 None（doc 不是 Element）
    let html_parent_node = html.borrow().parent_node().unwrap();
    assert!(Rc::ptr_eq(&html_parent_node, &doc));
    assert!(html.borrow().parent_element().is_none());
}

// —— text_content ——

#[test]
fn text_content_element_aggregates_descendants() {
    let doc = Node::new_document();
    let div = Node::new_element_html("div", vec![], &doc);
    let text1 = Node::new_text("Hello ", &doc);
    let span = Node::new_element_html("span", vec![], &doc);
    let text2 = Node::new_text("World", &doc);

    append_child(&doc, div.clone()).unwrap();
    append_child(&div, text1).unwrap();
    append_child(&div, span.clone()).unwrap();
    append_child(&span, text2).unwrap();

    assert_eq!(div.borrow().text_content(), Some("Hello World".to_string()));
}

#[test]
fn text_content_empty_element() {
    let doc = Node::new_document();
    let div = Node::new_element_html("div", vec![], &doc);
    assert_eq!(div.borrow().text_content(), Some("".to_string()));
}

#[test]
fn text_content_document_returns_none() {
    let doc = Node::new_document();
    assert_eq!(doc.borrow().text_content(), None);
}

#[test]
fn text_content_document_type_returns_none() {
    let doc = Node::new_document();
    let dt = Node::new_document_type("html", "", "", &doc);
    assert_eq!(dt.borrow().text_content(), None);
}

#[test]
fn set_text_content_replaces_children() {
    let doc = Node::new_document();
    let div = Node::new_element_html("div", vec![], &doc);
    let old_child = Node::new_element_html("span", vec![], &doc);
    append_child(&doc, div.clone()).unwrap();
    append_child(&div.clone(), old_child.clone()).unwrap();

    assert_eq!(div.borrow().child_count(), 1);

    set_text_content(&div, "new text");

    // 旧子节点被清空
    assert_eq!(div.borrow().child_count(), 1);
    let only_child = div.borrow().first_child().unwrap();
    assert_eq!(only_child.borrow().node_type, NodeType::Text);
    assert_eq!(
        only_child.borrow().text_content(),
        Some("new text".to_string())
    );
    // 旧子节点的 parent 已清空
    assert!(old_child.borrow().parent_node().is_none());
}

#[test]
fn set_text_content_empty_clears_children() {
    let doc = Node::new_document();
    let div = Node::new_element_html("div", vec![], &doc);
    let old_child = Node::new_text("old", &doc);
    append_child(&doc, div.clone()).unwrap();
    append_child(&div.clone(), old_child.clone()).unwrap();

    set_text_content(&div, "");
    assert_eq!(div.borrow().child_count(), 0);
    assert!(div.borrow().first_child().is_none());
    assert!(old_child.borrow().parent_node().is_none());
}

// —— descendants ——

#[test]
fn descendants_depth_first_order() {
    // 构造树：
    //     root
    //    / | \
    //   a  b  c
    //  / \
    // d   e
    let doc = Node::new_document();
    let root = Node::new_element_html("root", vec![], &doc);
    let a = Node::new_element_html("a", vec![], &doc);
    let b = Node::new_element_html("b", vec![], &doc);
    let c = Node::new_element_html("c", vec![], &doc);
    let d = Node::new_element_html("d", vec![], &doc);
    let e = Node::new_element_html("e", vec![], &doc);

    append_child(&root, a.clone()).unwrap();
    append_child(&root, b.clone()).unwrap();
    append_child(&root, c.clone()).unwrap();
    append_child(&a, d.clone()).unwrap();
    append_child(&a, e.clone()).unwrap();

    let names: Vec<String> = Node::descendants(&root)
        .map(|n| n.borrow().node_name.clone())
        .collect();

    // 深度优先，文档顺序：a, d, e, b, c
    assert_eq!(names, vec!["A", "D", "E", "B", "C"]);
}

#[test]
fn descendants_empty() {
    let doc = Node::new_document();
    let div = Node::new_element_html("div", vec![], &doc);
    assert_eq!(Node::descendants(&div).count(), 0);
}

// —— 错误路径 ——

#[test]
fn insert_self_as_child_errors() {
    let doc = Node::new_document();
    let div = Node::new_element_html("div", vec![], &doc);
    append_child(&doc, div.clone()).unwrap();

    let result = append_child(&div, div.clone());
    assert!(matches!(result, Err(DomError::HierarchyRequest(_))));
}

#[test]
fn insert_ancestor_as_child_errors() {
    let doc = Node::new_document();
    let parent = Node::new_element_html("parent", vec![], &doc);
    let child = Node::new_element_html("child", vec![], &doc);
    append_child(&doc, parent.clone()).unwrap();
    append_child(&parent, child.clone()).unwrap();

    // 试图把 parent 插入到 child 下
    let result = append_child(&child, parent.clone());
    assert!(matches!(result, Err(DomError::HierarchyRequest(_))));
}

#[test]
fn document_rejects_text_child() {
    let doc = Node::new_document();
    let text = Node::new_text("oops", &doc);
    let result = append_child(&doc, text);
    assert!(matches!(result, Err(DomError::HierarchyRequest(_))));
}

#[test]
fn document_rejects_second_element() {
    let doc = Node::new_document();
    let html1 = Node::new_element_html("html", vec![], &doc);
    let html2 = Node::new_element_html("html", vec![], &doc);
    append_child(&doc, html1).unwrap();
    let result = append_child(&doc, html2);
    assert!(matches!(result, Err(DomError::HierarchyRequest(_))));
}

#[test]
fn document_rejects_second_doctype() {
    let doc = Node::new_document();
    let dt1 = Node::new_document_type("html", "", "", &doc);
    let dt2 = Node::new_document_type("html", "", "", &doc);
    append_child(&doc, dt1).unwrap();
    let result = append_child(&doc, dt2);
    assert!(matches!(result, Err(DomError::HierarchyRequest(_))));
}

#[test]
fn document_accepts_doctype_then_element() {
    let doc = Node::new_document();
    let dt = Node::new_document_type("html", "", "", &doc);
    let html = Node::new_element_html("html", vec![], &doc);
    append_child(&doc, dt).unwrap();
    append_child(&doc, html).unwrap();
    assert_eq!(doc.borrow().child_count(), 2);
}

#[test]
fn document_accepts_comment() {
    let doc = Node::new_document();
    let comment = Node::new_comment("a doc comment", &doc);
    append_child(&doc, comment).unwrap();
    assert_eq!(doc.borrow().child_count(), 1);
}
