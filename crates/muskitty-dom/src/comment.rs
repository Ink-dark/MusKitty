//! Comment 节点数据。
//!
//! 参见 DOM Living Standard §7.3 (Interface Comment)。

/// `Comment` 节点的数据载体。
#[derive(Debug, Clone)]
pub struct CommentData {
    /// 注释内容。
    pub data: String,
}

impl CommentData {
    pub fn new(data: &str) -> Self {
        Self {
            data: data.to_string(),
        }
    }
}
