//! Mock 后端：仅记录绘制指令，不栅格化。用于测试。
//!
//! 验证 paint 产出的 `RenderCommand` 序列是否符合预期。

use crate::backend::Backend;
use crate::command::RenderCommand;

/// 记录所有传入的绘制指令与画布尺寸。
#[derive(Debug, Default)]
pub struct MockBackend {
    /// 最后一次 render 调用的画布宽度。
    pub width: u32,
    /// 最后一次 render 调用的画布高度。
    pub height: u32,
    /// 收到的所有绘制指令（按传入顺序）。
    pub commands: Vec<RenderCommand>,
}

impl MockBackend {
    /// 创建空 mock 后端。
    pub fn new() -> Self {
        Self::default()
    }

    /// 收到的指令数。
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// 是否未收到任何指令。
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Backend for MockBackend {
    fn render(&mut self, commands: &[RenderCommand], width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.commands = commands.to_vec();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    #[test]
    fn mock_records_commands() {
        let mut backend = MockBackend::new();
        let cmds = vec![
            RenderCommand::rect(0.0, 0.0, 10.0, 10.0, Color::rgb(255, 0, 0)),
            RenderCommand::rect(20.0, 20.0, 30.0, 30.0, Color::rgb(0, 255, 0)),
        ];
        backend.render(&cmds, 800, 600);
        assert_eq!(backend.width, 800);
        assert_eq!(backend.height, 600);
        assert_eq!(backend.len(), 2);
        assert_eq!(backend.commands, cmds);
    }

    #[test]
    fn mock_empty() {
        let mut backend = MockBackend::new();
        backend.render(&[], 100, 100);
        assert!(backend.is_empty());
        assert_eq!(backend.width, 100);
    }
}
