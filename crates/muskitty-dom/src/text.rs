//! Text 节点数据。
//!
//! 参见 DOM Living Standard §7.1 (Interface Text)。

/// `Text` 节点的数据载体。
#[derive(Debug, Clone)]
pub struct TextData {
    /// 节点的文本内容。
    pub data: String,
}

impl TextData {
    pub fn new(data: &str) -> Self {
        Self {
            data: data.to_string(),
        }
    }
}
