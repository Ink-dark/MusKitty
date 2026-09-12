//! 绘制指令（RenderCommand）。
//!
//! `paint` 函数输出 `Vec<RenderCommand>`，后端 [`Backend`](crate::backend::Backend)
//! 消费这些指令栅格化为像素。当前仅 `Rect`，文本/裁剪推迟。

use crate::color::Color;

/// CSS `text-align` 的水平对齐（T-3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// 左对齐（`left` / `start` / 默认）。
    #[default]
    Left,
    /// 居中（`center`）。
    Center,
    /// 右对齐（`right` / `end`）。
    Right,
}

/// 单条绘制指令。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RenderCommand {
    /// 矩形填充（含可选四边边框）。
    ///
    /// `x` / `y` 为相对画布原点的绝对坐标（已累加父元素偏移），
    /// `width` / `height` 为元素的 border box 尺寸（content + padding +
    /// border；taffy 的布局结果即 border box）。
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
        /// 四边边框。`None` 表示无边框（四边均为 `None` 亦等价）。
        border: Option<Border>,
    },
    /// 文本绘制（T-2 / T-3）。
    ///
    /// glyph 细节（整形/光栅化）由后端用 cosmic-text 现算；此处承载
    /// 文本串 + 字体样式 + 颜色，位置为 text 布局盒左上角（画布坐标系）。
    ///
    /// M-3 batch 3：`text` 是**已应用 `text-transform`** 的内容（与 layout
    /// 测量所用文本一致——paint 与 layout 都调用 cascade
    /// `apply_text_transform`）；`line_height` 是 `line-height` 的使用值（px，
    /// cascade `used_line_height_px` 解析），后端须用它换行与排行，否则
    /// 绘制行位置与布局盒高不一致（T-3 的"汉字位移"教训）。
    Text {
        /// 左上角 X（px，画布坐标系）。
        x: f32,
        /// 左上角 Y（px，画布坐标系）。
        y: f32,
        /// 布局宽度（px），用于换行（T-3），与 layout 层 measure 的容器宽一致。
        width: f32,
        /// 文本内容（已应用 `text-transform`）。
        text: String,
        /// 字号（px）。
        font_size: f32,
        /// 行高（px，`line-height` 的使用值）。
        line_height: f32,
        /// 字体族名（CSS `font-family` 首个族名）。
        font_family: String,
        /// 字重（CSS `font-weight`，100-900）。
        font_weight: u16,
        /// 水平对齐（CSS `text-align`）。
        text_align: TextAlign,
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
    /// 轮廓绘制（M-3 batch 2，CSS UI Level 4 §4）。
    ///
    /// 轮廓绘制在元素 **border box 之外**（本命令的 `x`/`y`/`width`/`height`
    /// 即该 border box），不参与布局、不影响元素尺寸。语义上轮廓绘制在元素
    /// 及其后代之上，故 paint 在递归子节点**之后**发出本命令（与
    /// `overflow` 裁剪的 `EndClip` 之后），避免被后代覆盖。
    ///
    /// `outline-offset` 尚未注册，固定为 0（轮廓紧贴 border box 外缘）。
    Outline {
        /// 元素 border box 左上角 X（px，画布坐标系）。
        x: f32,
        /// 元素 border box 左上角 Y（px，画布坐标系）。
        y: f32,
        /// 元素 border box 宽度（px）。
        width: f32,
        /// 元素 border box 高度（px）。
        height: f32,
        /// 轮廓宽度（px）。
        outline_width: f32,
        /// 轮廓颜色（`auto` / `currentcolor` 已由 paint 解析）。
        color: Color,
        /// 轮廓样式（`none`/`hidden` 不会生成本命令）。
        style: BorderStyle,
    },
}

/// 边框描述（四边独立，M-3 batch 2）。
///
/// 每边为 `None` 表示该边不绘制（未声明 / `border-style: none`/`hidden` /
/// used width 为 0）。`border` 简写在 cascade 阶段展开为方向性长属性，
/// 四边的宽/色/样式因此可以各不相同（`border-left: 4px solid red` 只影响左边）。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Border {
    /// 上边框。
    pub top: Option<SideBorder>,
    /// 右边框。
    pub right: Option<SideBorder>,
    /// 下边框。
    pub bottom: Option<SideBorder>,
    /// 左边框。
    pub left: Option<SideBorder>,
}

impl Border {
    /// 四边同宽同色同样式。
    pub fn uniform(width: f32, color: Color, style: BorderStyle) -> Self {
        let side = SideBorder {
            width,
            color,
            style,
        };
        Self {
            top: Some(side),
            right: Some(side),
            bottom: Some(side),
            left: Some(side),
        }
    }

    /// 是否四边均无边框。
    pub fn is_empty(&self) -> bool {
        self.top.is_none() && self.right.is_none() && self.bottom.is_none() && self.left.is_none()
    }

    /// 各边宽度 `(top, right, bottom, left)`，无边框的边为 0。
    ///
    /// 供后端按边拼接矩形条使用（corner 归属由消费方决定）。
    pub fn widths(&self) -> (f32, f32, f32, f32) {
        let w = |s: Option<SideBorder>| s.map(|s| s.width.max(0.0)).unwrap_or(0.0);
        (w(self.top), w(self.right), w(self.bottom), w(self.left))
    }
}

/// 单边边框。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SideBorder {
    /// 边框宽度（px；`none`/`hidden` 的 used width = 0 已在上游判定）。
    pub width: f32,
    /// 边框颜色（`currentcolor` 已由 paint 解析为元素文字色）。
    pub color: Color,
    /// 边框样式。
    pub style: BorderStyle,
}

/// CSS border-style 关键字（CSS Backgrounds & Borders L3 §4.2 全集）。
///
/// `None`/`Hidden` 不绘制（§4.1：两者 used width 均为 0）。其余样式当前
/// 按 solid 近似绘制——虚线/点线需 dash 模式，`double` 需分线，明暗类
/// （groove/ridge/inset/outset）需按边定向明暗，均在 backend 的绘制实现处
/// 记录为已知简化。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    /// `none`：不绘制（默认）。
    #[default]
    None,
    /// `hidden`：不绘制。
    Hidden,
    /// `solid`：实线。
    Solid,
    /// `dashed`：虚线（当前按 solid 近似）。
    Dashed,
    /// `dotted`：点线（当前按 solid 近似）。
    Dotted,
    /// `double`：双线（当前按 solid 近似）。
    Double,
    /// `groove`：凹槽（当前按 solid 近似）。
    Groove,
    /// `ridge`：凸脊（当前按 solid 近似）。
    Ridge,
    /// `inset`：内嵌（当前按 solid 近似）。
    Inset,
    /// `outset`：外凸（当前按 solid 近似）。
    Outset,
}

impl BorderStyle {
    /// 该样式是否产生可见描边（`none`/`hidden` 为否）。
    pub fn is_painted(self) -> bool {
        !matches!(self, BorderStyle::None | BorderStyle::Hidden)
    }
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
    fn border_default_is_empty() {
        let b = Border::default();
        assert!(b.is_empty(), "默认 Border 四边皆无");
        assert_eq!(BorderStyle::default(), BorderStyle::None);
    }

    #[test]
    fn border_uniform_sets_all_sides() {
        let b = Border::uniform(2.0, Color::rgb(0, 0, 255), BorderStyle::Solid);
        assert!(!b.is_empty());
        for side in [b.top, b.right, b.bottom, b.left] {
            let side = side.expect("uniform sets every side");
            assert_eq!(side.width, 2.0);
            assert_eq!(side.color, Color::rgb(0, 0, 255));
            assert_eq!(side.style, BorderStyle::Solid);
        }
        assert_eq!(b.widths(), (2.0, 2.0, 2.0, 2.0));
    }

    #[test]
    fn border_single_side_widths() {
        // 仅左边 → widths() 只有 left 非零
        let b = Border {
            left: Some(SideBorder {
                width: 4.0,
                color: Color::BLACK,
                style: BorderStyle::Solid,
            }),
            ..Border::default()
        };
        assert_eq!(b.widths(), (0.0, 0.0, 0.0, 4.0));
        assert!(!b.is_empty());
    }

    #[test]
    fn border_style_painted_predicate() {
        assert!(!BorderStyle::None.is_painted());
        assert!(!BorderStyle::Hidden.is_painted());
        assert!(BorderStyle::Solid.is_painted());
        assert!(BorderStyle::Double.is_painted());
        assert!(BorderStyle::Groove.is_painted());
    }
}
