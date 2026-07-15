//! Document 节点数据。
//!
//! 参见 DOM Living Standard §5 (Documents)。

/// `Document` 节点的数据载体。
#[derive(Debug, Clone)]
pub struct DocumentData {
    /// 文档 URL，默认 `"about:blank"`。
    pub url: String,
    /// Content-Type，默认 `"text/html"`。
    pub content_type: String,
    /// 字符编码标签（如 `"UTF-8"`）。
    pub encoding: String,
}

impl Default for DocumentData {
    fn default() -> Self {
        Self {
            url: "about:blank".to_string(),
            content_type: "text/html".to_string(),
            encoding: "UTF-8".to_string(),
        }
    }
}

impl DocumentData {
    pub fn new() -> Self {
        Self::default()
    }
}
