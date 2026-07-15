//! Element 节点数据。
//!
//! 参见 DOM Living Standard §6 (Elements)。

use crate::attribute::{Attribute, Namespace};

/// `Element` 节点的数据载体。
#[derive(Debug, Clone)]
pub struct ElementData {
    /// 本地名（HTML namespace 下为小写，如 `"div"`）。
    pub local_name: String,
    /// 命名空间分类。
    pub namespace: Namespace,
    /// 命名空间 URI（与 `namespace` 对应）。
    pub namespace_uri: Option<String>,
    /// 限定名前缀（如 `svg:rect` 的 `svg`），HTML namespace 下通常为 `None`。
    pub prefix: Option<String>,
    /// 属性列表（按文档顺序）。
    pub attributes: Vec<Attribute>,
}

impl ElementData {
    /// 创建 HTML namespace 下的元素。
    pub fn new_html(local_name: &str, attributes: Vec<Attribute>) -> Self {
        Self {
            local_name: local_name.to_ascii_lowercase(),
            namespace: Namespace::Html,
            namespace_uri: Namespace::Html.uri().map(String::from),
            prefix: None,
            attributes,
        }
    }

    /// 创建指定 namespace 下的元素。
    pub fn with_namespace(
        local_name: String,
        namespace: Namespace,
        prefix: Option<String>,
        attributes: Vec<Attribute>,
    ) -> Self {
        Self {
            local_name,
            namespace_uri: namespace.uri().map(String::from),
            namespace,
            prefix,
            attributes,
        }
    }

    /// 返回元素的 `node_name`（HTML namespace 下为大写，否则原样）。
    /// 用于 Node.node_name。
    pub fn node_name(&self) -> String {
        match self.namespace {
            Namespace::Html => self.local_name.to_ascii_uppercase(),
            _ => self.local_name.clone(),
        }
    }

    /// 查找指定 local_name 的属性值（大小写不敏感，HTML namespace）。
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.local_name.eq_ignore_ascii_case(name))
            .map(|a| a.value.as_str())
    }
}
