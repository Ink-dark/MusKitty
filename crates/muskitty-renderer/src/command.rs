//! 绘制指令（RenderCommand）。
//!
//! `paint` 函数输出 `Vec<RenderCommand>`，后端 [`Backend`](crate::backend::Backend)
//! 消费这些指令栅格化为像素。当前仅 `Rect`，文本/裁剪推迟。

use crate::color::Color;

/// 单条绘制指令。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RenderCommand {
    /// 矩形填充（含可选边框）。
    ///
    /// `x` / `y` 为相对画布原点的绝对坐标（已累加父元素偏移），
    /// `width` / `height` 为元素的 content + padding + border 总尺寸。
    Rect {
        /// 左上角 X（px，画布坐标系）。
        x: f32,
        /// 左上角 Y（px，画布坐标系）。
        y: f32,
        /// 宽度（px）。
        width: f32,
        /// 高度（px）。
        height: f32,
        /// 背景填充色。`None` 表示不填充（透明）。
        background: Option<Color>,
        /// 边框。`None` 表示无边框。
        ///
        /// 注意：cascade 已注册 border-* 属性（P2-7），但 paint 阶段
        /// 尚未读取 computed 的边框生成绘制指令，该字段暂恒为 `None`，
        /// 供后续 Phase 4 扩展。
        border: Option<Border>,
    },
    /// 文本绘制（T-2 / T-3）。
    ///
    /// glyph 细节（整形/光栅化）由后端用 cosmic-text 现算；此处承载
    /// 文本串 + 字体样式 + 颜色，位置为 text 布局盒左上角（画布坐标系）。
    Text {
        /// 左上角 X（px，画布坐标系）。
        x: f32,
        /// 左上角 Y（px，画布坐标系）。
        y: f32,
        /// 布局宽度（px），用于换行（T-3），与 layout 层 measure 的容器宽一致。
        width: f32,
        /// 文本内容。
        text: String,
        /// 字号（px）。
        font_size: f32,
        /// 字体族名（CSS `font-family` 首个族名）。
        font_family: String,
        /// 字重（CSS `font-weight`，100-900）。
        font_weight: u16,
        /// 文字颜色。
        color: Color,
    },
    /// 开始裁剪（L-2）：后续指令裁剪到该矩形内，直到 [`RenderCommand::EndClip`]。
    Clip {
        /// 裁剪矩形左上角 X（px，画布坐标系）。
        x: f32,
        /// 裁剪矩形左上角 Y（px，画布坐标系）。
        y: f32,
        /// 裁剪矩形宽度（px）。
        width: f32,
        /// 裁剪矩形高度（px）。
        height: f32,
    },
    /// 结束裁剪（L-2）：恢复到最近 [`RenderCommand::Clip`] 之前的状态。
    EndClip,
}

/// 边框描述。
///
/// 当前为等宽四边；后续可扩展为逐边不同。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Border {
    /// 边框宽度（px）。
    pub width: f32,
    /// 边框颜色。
    pub color: Color,
    /// 边框样式。
    pub style: BorderStyle,
}

/// CSS border-style 关键字（CSS Backgrounds L3 §4.2）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    /// `none`：不绘制（默认）。
    #[default]
    None,
    /// `solid`：实线。
    Solid,
    /// `dashed`：虚线（推迟）。
    Dashed,
    /// `dotted`：点线（推迟）。
    Dotted,
}

impl RenderCommand {
    /// 构造一个纯背景填充矩形（无边框）。
    pub fn rect(x: f32, y: f32, width: f32, height: f32, background: Color) -> Self {
        RenderCommand::Rect {
            x,
            y,
            width,
            height,
            background: Some(background),
            border: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_command_construction() {
        let cmd = RenderCommand::rect(10.0, 20.0, 100.0, 50.0, Color::rgb(255, 0, 0));
        match cmd {
            RenderCommand::Rect {
                x,
                y,
                width,
                height,
                background,
                border,
            } => {
                assert_eq!(x, 10.0);
                assert_eq!(y, 20.0);
                assert_eq!(width, 100.0);
                assert_eq!(height, 50.0);
                assert_eq!(background, Some(Color::rgb(255, 0, 0)));
                assert_eq!(border, None);
            }
            _ => panic!("expected Rect"),
        }
    }

    #[test]
    fn border_default_is_none() {
        let b = Border::default();
        assert_eq!(b.style, BorderStyle::None);
        assert_eq!(b.width, 0.0);
    }
}
