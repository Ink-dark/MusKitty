//! ProcessingInstruction 节点数据。
//!
//! 参见 DOM Living Standard §7.4 (Interface ProcessingInstruction)。

/// `ProcessingInstruction` 节点的数据载体。
#[derive(Debug, Clone)]
pub struct ProcessingInstructionData {
    /// PI 的 target（`ProcessingInstruction.target`）。
    pub target: String,
    /// PI 的 data（`ProcessingInstruction.data` / `CharacterData.data`）。
    pub data: String,
}

impl ProcessingInstructionData {
    pub fn new(target: &str, data: &str) -> Self {
        Self {
            target: target.to_string(),
            data: data.to_string(),
        }
    }
}
