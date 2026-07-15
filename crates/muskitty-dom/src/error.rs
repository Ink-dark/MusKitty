//! DOM 操作错误类型。
//!
//! 参见 DOM Living Standard §3.1 (Errors)。

/// DOM 树操作中可能抛出的错误。
///
/// 命名与规范中的 `DOMException` 名字对应。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomError {
    /// `HierarchyRequestError`：节点试图放在不合法的层级位置。
    HierarchyRequest(String),
    /// `WrongDocumentError`：节点属于另一个 Document。
    WrongDocument(String),
    /// `NotFoundError`：目标子节点不存在。
    NotFound(String),
    /// `InvalidCharacterError`：名称包含非法字符。
    InvalidCharacter(String),
}

impl std::fmt::Display for DomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomError::HierarchyRequest(msg) => {
                write!(f, "HierarchyRequestError: {msg}")
            }
            DomError::WrongDocument(msg) => write!(f, "WrongDocumentError: {msg}"),
            DomError::NotFound(msg) => write!(f, "NotFoundError: {msg}"),
            DomError::InvalidCharacter(msg) => {
                write!(f, "InvalidCharacterError: {msg}")
            }
        }
    }
}

impl std::error::Error for DomError {}
