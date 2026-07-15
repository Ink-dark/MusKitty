//! DocumentType 节点数据。
//!
//! 参见 DOM Living Standard §10.1 (Interface DocumentType)。

/// `DocumentType` 节点的数据载体（如 `<!DOCTYPE html>`）。
#[derive(Debug, Clone)]
pub struct DocumentTypeData {
    /// DOCTYPE 名称（如 `"html"`）。
    pub name: String,
    /// PUBLIC 标识符，无则为空字符串。
    pub public_id: String,
    /// SYSTEM 标识符，无则为空字符串。
    pub system_id: String,
}

impl DocumentTypeData {
    pub fn new(name: &str, public_id: &str, system_id: &str) -> Self {
        Self {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        }
    }
}
