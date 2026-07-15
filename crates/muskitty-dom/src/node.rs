//! DOM 节点核心类型。
//!
//! 参见 DOM Living Standard §4 (Nodes) 和 §4.4 (Interface Node)。
//!
//! 节点使用 `Rc<RefCell<Node>>` 共享所有权：父节点持有子节点的强引用，
//! 子节点持有父节点的弱引用（`Weak`），避免循环引用导致内存泄漏。

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::attribute::{Attribute, Namespace};
use crate::comment::CommentData;
use crate::document::DocumentData;
use crate::document_fragment::DocumentFragmentData;
use crate::document_type::DocumentTypeData;
use crate::element::ElementData;
use crate::text::TextData;

/// `Node.nodeType` 常量。参见 DOM Living Standard §4.4。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeType {
    /// `ELEMENT_NODE = 1`
    Element = 1,
    /// `TEXT_NODE = 3`
    Text = 3,
    /// `CDATA_SECTION_NODE = 4`
    CdataSection = 4,
    /// `PROCESSING_INSTRUCTION_NODE = 7`
    ProcessingInstruction = 7,
    /// `COMMENT_NODE = 8`
    Comment = 8,
    /// `DOCUMENT_NODE = 9`
    Document = 9,
    /// `DOCUMENT_TYPE_NODE = 10`
    DocumentType = 10,
    /// `DOCUMENT_FRAGMENT_NODE = 11`
    DocumentFragment = 11,
}

impl NodeType {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// 节点具体类型的数据载体。
pub enum NodeKind {
    Element(ElementData),
    Text(TextData),
    Comment(CommentData),
    Document(DocumentData),
    DocumentType(DocumentTypeData),
    DocumentFragment(DocumentFragmentData),
}

impl NodeKind {
    /// 若为 Element，返回其数据引用。
    pub fn as_element(&self) -> Option<&ElementData> {
        match self {
            NodeKind::Element(e) => Some(e),
            _ => None,
        }
    }

    /// 若为 Text，返回其数据引用。
    pub fn as_text(&self) -> Option<&TextData> {
        match self {
            NodeKind::Text(t) => Some(t),
            _ => None,
        }
    }

    /// 若为 Comment，返回其数据引用。
    pub fn as_comment(&self) -> Option<&CommentData> {
        match self {
            NodeKind::Comment(c) => Some(c),
            _ => None,
        }
    }

    /// 若为 Document，返回其数据引用。
    pub fn as_document(&self) -> Option<&DocumentData> {
        match self {
            NodeKind::Document(d) => Some(d),
            _ => None,
        }
    }

    /// 若为 DocumentType，返回其数据引用。
    pub fn as_document_type(&self) -> Option<&DocumentTypeData> {
        match self {
            NodeKind::DocumentType(d) => Some(d),
            _ => None,
        }
    }

    /// 若为 Element，返回其可变数据引用。
    pub fn as_element_mut(&mut self) -> Option<&mut ElementData> {
        match self {
            NodeKind::Element(e) => Some(e),
            _ => None,
        }
    }

    /// 若为 Text，返回其可变数据引用。
    pub fn as_text_mut(&mut self) -> Option<&mut TextData> {
        match self {
            NodeKind::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// DOM 节点，DOM 树的基本单元。
///
/// 实现对应 DOM Living Standard §4.4 `Node` 接口的核心字段与方法。
pub struct Node {
    /// `Node.nodeType`
    pub node_type: NodeType,
    /// `Node.nodeName`
    ///（Document=`"#document"`、Text=`"#text"`、Comment=`"#comment"`、
    /// DocumentFragment=`"#document-fragment"`、Element=限定名、DocumentType=name）
    pub node_name: String,
    /// 节点所属 Document（Document 节点指向自身）。
    pub owner_document: Weak<RefCell<Node>>,
    /// 父节点弱引用，无父则为空 `Weak`。
    pub parent_node: Weak<RefCell<Node>>,
    /// 子节点列表（按文档顺序）。
    pub children: Vec<Rc<RefCell<Node>>>,
    /// 具体类型数据。
    pub kind: NodeKind,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("node_type", &self.node_type)
            .field("node_name", &self.node_name)
            .field("child_count", &self.children.len())
            .finish()
    }
}

impl Node {
    /// 创建 Document 节点。`owner_document` 指向自身。
    /// 参见 DOM §5.3。
    pub fn new_document() -> Rc<RefCell<Node>> {
        let node = Rc::new(RefCell::new(Node {
            node_type: NodeType::Document,
            node_name: "#document".to_string(),
            owner_document: Weak::new(),
            parent_node: Weak::new(),
            children: Vec::new(),
            kind: NodeKind::Document(DocumentData::new()),
        }));
        // Document 的 owner_document 指向自身
        node.borrow_mut().owner_document = Rc::downgrade(&node);
        node
    }

    /// 创建 HTML namespace 下的 Element 节点。
    pub fn new_element_html(
        tag_name: &str,
        attributes: Vec<Attribute>,
        owner_document: &Rc<RefCell<Node>>,
    ) -> Rc<RefCell<Node>> {
        let element = ElementData::new_html(tag_name, attributes);
        Rc::new(RefCell::new(Node {
            node_type: NodeType::Element,
            node_name: element.node_name(),
            owner_document: Rc::downgrade(owner_document),
            parent_node: Weak::new(),
            children: Vec::new(),
            kind: NodeKind::Element(element),
        }))
    }

    /// 创建指定 namespace 下的 Element 节点。
    pub fn new_element_ns(
        local_name: String,
        namespace: Namespace,
        prefix: Option<String>,
        attributes: Vec<Attribute>,
        owner_document: &Rc<RefCell<Node>>,
    ) -> Rc<RefCell<Node>> {
        let element = ElementData::with_namespace(local_name, namespace, prefix, attributes);
        Rc::new(RefCell::new(Node {
            node_type: NodeType::Element,
            node_name: element.node_name(),
            owner_document: Rc::downgrade(owner_document),
            parent_node: Weak::new(),
            children: Vec::new(),
            kind: NodeKind::Element(element),
        }))
    }

    /// 创建 Text 节点。
    pub fn new_text(data: &str, owner_document: &Rc<RefCell<Node>>) -> Rc<RefCell<Node>> {
        Rc::new(RefCell::new(Node {
            node_type: NodeType::Text,
            node_name: "#text".to_string(),
            owner_document: Rc::downgrade(owner_document),
            parent_node: Weak::new(),
            children: Vec::new(),
            kind: NodeKind::Text(TextData::new(data)),
        }))
    }

    /// 创建 Comment 节点。
    pub fn new_comment(data: &str, owner_document: &Rc<RefCell<Node>>) -> Rc<RefCell<Node>> {
        Rc::new(RefCell::new(Node {
            node_type: NodeType::Comment,
            node_name: "#comment".to_string(),
            owner_document: Rc::downgrade(owner_document),
            parent_node: Weak::new(),
            children: Vec::new(),
            kind: NodeKind::Comment(CommentData::new(data)),
        }))
    }

    /// 创建 DocumentType 节点。
    pub fn new_document_type(
        name: &str,
        public_id: &str,
        system_id: &str,
        owner_document: &Rc<RefCell<Node>>,
    ) -> Rc<RefCell<Node>> {
        Rc::new(RefCell::new(Node {
            node_type: NodeType::DocumentType,
            node_name: name.to_string(),
            owner_document: Rc::downgrade(owner_document),
            parent_node: Weak::new(),
            children: Vec::new(),
            kind: NodeKind::DocumentType(DocumentTypeData::new(name, public_id, system_id)),
        }))
    }

    /// 创建 DocumentFragment 节点。
    pub fn new_document_fragment(owner_document: &Rc<RefCell<Node>>) -> Rc<RefCell<Node>> {
        Rc::new(RefCell::new(Node {
            node_type: NodeType::DocumentFragment,
            node_name: "#document-fragment".to_string(),
            owner_document: Rc::downgrade(owner_document),
            parent_node: Weak::new(),
            children: Vec::new(),
            kind: NodeKind::DocumentFragment(DocumentFragmentData),
        }))
    }

    // —— 只读遍历 API（DOM §4.4） ——

    /// `Node.parentNode`
    pub fn parent_node(&self) -> Option<Rc<RefCell<Node>>> {
        self.parent_node.upgrade()
    }

    /// `Node.parentElement`
    pub fn parent_element(&self) -> Option<Rc<RefCell<Node>>> {
        let parent = self.parent_node.upgrade()?;
        if parent.borrow().node_type == NodeType::Element {
            Some(parent)
        } else {
            None
        }
    }

    /// `Node.firstChild`
    pub fn first_child(&self) -> Option<Rc<RefCell<Node>>> {
        self.children.first().cloned()
    }

    /// `Node.lastChild`
    pub fn last_child(&self) -> Option<Rc<RefCell<Node>>> {
        self.children.last().cloned()
    }

    /// `Node.hasChildNodes`
    pub fn has_child_nodes(&self) -> bool {
        !self.children.is_empty()
    }

    /// 子节点数量。
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// `Node.previousSibling`。
    ///
    /// 通过比较 `Node` 内存地址在父节点 children 中定位自身。
    pub fn previous_sibling(&self) -> Option<Rc<RefCell<Node>>> {
        let parent = self.parent_node.upgrade()?;
        let parent_ref = parent.borrow();
        let self_ptr = self as *const Node;
        let idx = parent_ref.children.iter().position(|c| {
            let c_borrowed = c.borrow();
            (&*c_borrowed as *const Node) == self_ptr
        })?;
        if idx == 0 {
            None
        } else {
            Some(parent_ref.children[idx - 1].clone())
        }
    }

    /// `Node.nextSibling`。
    ///
    /// 通过比较 `Node` 内存地址在父节点 children 中定位自身。
    pub fn next_sibling(&self) -> Option<Rc<RefCell<Node>>> {
        let parent = self.parent_node.upgrade()?;
        let parent_ref = parent.borrow();
        let self_ptr = self as *const Node;
        let idx = parent_ref.children.iter().position(|c| {
            let c_borrowed = c.borrow();
            (&*c_borrowed as *const Node) == self_ptr
        })?;
        if idx + 1 >= parent_ref.children.len() {
            None
        } else {
            Some(parent_ref.children[idx + 1].clone())
        }
    }

    /// `Node.textContent` getter（DOM §4.4）。
    ///
    /// - DocumentFragment / Element：聚合所有后代 Text 节点内容。
    /// - Text / Comment：返回自身 data。
    /// - 其他类型：返回 `None`。
    pub fn text_content(&self) -> Option<String> {
        match self.node_type {
            NodeType::DocumentFragment | NodeType::Element => {
                let mut s = String::new();
                for child in &self.children {
                    if let Some(t) = child.borrow().text_content() {
                        s.push_str(&t);
                    }
                }
                Some(s)
            }
            NodeType::Text | NodeType::Comment => match &self.kind {
                NodeKind::Text(t) => Some(t.data.clone()),
                NodeKind::Comment(c) => Some(c.data.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    /// 返回后代节点的深度优先迭代器（不含自身）。
    pub fn descendants(node: &Rc<RefCell<Node>>) -> Descendants {
        let children: Vec<Rc<RefCell<Node>>> =
            node.borrow().children.iter().rev().cloned().collect();
        Descendants { stack: children }
    }
}

/// 后代节点深度优先迭代器。
pub struct Descendants {
    stack: Vec<Rc<RefCell<Node>>>,
}

impl Iterator for Descendants {
    type Item = Rc<RefCell<Node>>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        // 子节点逆序入栈以保持文档顺序（深度优先）
        let children: Vec<Rc<RefCell<Node>>> =
            node.borrow().children.iter().rev().cloned().collect();
        self.stack.extend(children);
        Some(node)
    }
}
