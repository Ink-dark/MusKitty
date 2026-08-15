//! tiny-skia 后端：纯 Rust CPU 渲染，输出 PNG。
//!
//! 将 `RenderCommand::Rect` 转换为 tiny-skia 的 `fill_rect` /
//! `stroke_path` 操作，栅格化到 [`Pixmap`] 并支持 PNG 输出。
//!
//! # 颜色处理
//!
//! - [`Color`](crate::color::Color) 是 8-bit per channel 非预乘 RGBA，
//!   转换为 tiny-skia 的 [`tiny_skia::Color`]（也是非预乘）后由 fill_rect /
//!   stroke_path 内部完成预乘。
//!
//! # 边框绘制
//!
//! - CSS border-box 外边缘 = `Rect { x, y, width, height }`
//! - stroke 默认沿路径中心对齐（一半在内，一半在外）。为了让 stroke
//!   完全位于 border-box 内（不超出元素外缘），构造一个内缩
//!   `border.width / 2` 的 path 再描边。
//! - dashed / dotted 样式当前按 solid 渲染（推迟到 tiny-skia 的 dash
//!   支持接入）。

use cosmic_text::{Attrs, Buffer, Command, FontSystem, Metrics, Shaping, SwashCache};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::backend::{Backend, RenderOutput};
use crate::color::Color;
use crate::command::{Border, BorderStyle, RenderCommand};

/// tiny-skia CPU 渲染后端。
///
/// `render` 后渲染结果保存在内部 [`Pixmap`]，可通过 [`pixmap`](Self::pixmap) /
/// [`encode_png`](Self::encode_png) / [`save_png`](Self::save_png) 取出。
#[derive(Debug, Default)]
pub struct TinySkiaBackend {
    pixmap: Option<Pixmap>,
}

impl TinySkiaBackend {
    /// 创建空后端（尚未渲染）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 获取最后一次渲染产出的像素图（不可变引用）。
    pub fn pixmap(&self) -> Option<&Pixmap> {
        self.pixmap.as_ref()
    }

    /// 取出内部 Pixmap 的所有权。
    pub fn take_pixmap(&mut self) -> Option<Pixmap> {
        self.pixmap.take()
    }

    /// 将渲染结果编码为 PNG 字节流。
    ///
    /// 返回 `Err` 当尚未渲染或 PNG 编码失败。
    ///
    /// 错误类型用 `Box<dyn Error>` 而非 `png::EncodingError`，避免把
    /// `png` crate 暴露为 muskitty-renderer 的公共 API 依赖。
    pub fn encode_png(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let pixmap = self
            .pixmap
            .as_ref()
            .ok_or("backend has not been rendered yet")?;
        Ok(pixmap.encode_png()?)
    }

    /// 将渲染结果保存为 PNG 文件。
    pub fn save_png<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pixmap = self
            .pixmap
            .as_ref()
            .ok_or("backend has not been rendered yet")?;
        Ok(pixmap.save_png(path)?)
    }
}

impl Backend for TinySkiaBackend {
    fn render(&mut self, commands: &[RenderCommand], width: u32, height: u32) -> RenderOutput {
        // Pixmap::new 返回 None 当 width/height 为 0 或超过 i32::MAX/4。
        // 回退到 1x1 像素以避免 panic（这种情况下命令通常也无法绘制）。
        let mut pixmap = Pixmap::new(width, height)
            .unwrap_or_else(|| Pixmap::new(1, 1).expect("1x1 pixmap always succeeds"));

        // P3-5: 画布默认填白（根元素背景传播的简化近似，见审计文档）。
        // 元素指令按序覆盖其上；未覆盖区域呈现白色而非透明。
        if let Some(canvas) = Rect::from_xywh(0.0, 0.0, width as f32, height as f32) {
            let mut white = Paint::default();
            white.set_color_rgba8(255, 255, 255, 255);
            pixmap.fill_rect(canvas, &white, Transform::identity(), None);
        }

        // 文本光栅化上下文（懒创建：仅在遇首个 Text 命令时初始化，避免
        // 纯色块场景下扫描系统字体）。
        let mut font_system: Option<FontSystem> = None;
        let mut swash_cache: Option<SwashCache> = None;

        for cmd in commands {
            match cmd {
                RenderCommand::Rect {
                    x,
                    y,
                    width,
                    height,
                    background,
                    border,
                } => {
                    // 跳过零尺寸矩形
                    if *width <= 0.0 || *height <= 0.0 {
                        continue;
                    }

                    // 填充背景
                    if let Some(bg) = background {
                        if let Some(rect) = Rect::from_xywh(*x, *y, *width, *height) {
                            let mut paint = Paint::default();
                            paint.set_color_rgba8(bg.r, bg.g, bg.b, bg.a);
                            paint.anti_alias = false;
                            pixmap.fill_rect(rect, &paint, Transform::identity(), None);
                        }
                    }

                    // 绘制边框
                    if let Some(b) = border {
                        if b.width > 0.0 {
                            draw_border(&mut pixmap, *x, *y, *width, *height, b);
                        }
                    }
                }
                RenderCommand::Text {
                    x,
                    y,
                    text,
                    font_size,
                    color,
                } => {
                    if font_system.is_none() {
                        font_system = Some(FontSystem::new());
                        swash_cache = Some(SwashCache::new());
                    }
                    draw_text(
                        &mut pixmap,
                        *x,
                        *y,
                        text,
                        *font_size,
                        *color,
                        font_system.as_mut().expect("font_system just initialized"),
                        swash_cache.as_mut().expect("swash_cache just initialized"),
                    );
                }
            }
        }

        self.pixmap = Some(pixmap);

        // P2-18：返回像素数据（RGBA，行长 = width*4）。`pixmap.data()` 保持
        // 引用有效直到赋值前，故先取引用再 move。
        let p = self.pixmap.as_ref().expect("pixmap just set");
        RenderOutput::Pixels {
            width,
            height,
            data: p.data().to_vec(),
        }
    }
}

/// 绘制矩形边框（沿 border-box 内边缘描边）。
fn draw_border(pixmap: &mut Pixmap, x: f32, y: f32, width: f32, height: f32, border: &Border) {
    let half = border.width / 2.0;

    // 内缩半个边框宽度，使 stroke 完全位于 border-box 内
    let inner = Rect::from_xywh(
        x + half,
        y + half,
        width - border.width,
        height - border.width,
    );

    let rect = match inner {
        Some(r) => r,
        None => return, // 边框宽度大于元素尺寸 → 无法绘制
    };

    let path = PathBuilder::from_rect(rect);

    let mut paint = Paint::default();
    paint.set_color_rgba8(
        border.color.r,
        border.color.g,
        border.color.b,
        border.color.a,
    );
    paint.anti_alias = true;

    let stroke = Stroke {
        width: border.width,
        ..Default::default()
    };

    // dashed / dotted 当前按 solid 渲染（推迟 dash 模式接入）
    match border.style {
        BorderStyle::Solid | BorderStyle::Dashed | BorderStyle::Dotted => {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        BorderStyle::None => {} // 已在外层过滤
    }
}

/// 绘制文本（T-2）。
///
/// 用 cosmic-text 整形文本，swash 提取每个 glyph 的矢量 outline，
/// 转为 tiny-skia 路径填充。`x`/`y` 为 Text 命令的布局盒左上角
/// （画布坐标系）。
#[allow(clippy::too_many_arguments)]
fn draw_text(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    text: &str,
    font_size: f32,
    color: Color,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
) {
    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
    // 单行（不换行）。
    buffer.set_size(font_system, None, None);
    buffer.set_text(font_system, text, Attrs::new(), Shaping::Advanced);

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;

    // glyph 物理坐标相对 buffer 原点，经 Transform 平移到 Text 命令位置。
    let transform = Transform::from_translate(x, y);

    for run in buffer.layout_runs() {
        for glyph in run.glyphs {
            // physical() 返回像素对齐的物理坐标（含 baseline）+ cache key。
            let physical = glyph.physical((0.0, 0.0), 1.0);
            let gx = physical.x as f32;
            let gy = physical.y as f32;
            let cache_key = physical.cache_key;
            if let Some(commands) = swash_cache.get_outline_commands(font_system, cache_key) {
                let mut pb = PathBuilder::new();
                for cmd in commands {
                    match *cmd {
                        Command::MoveTo(p) => pb.move_to(gx + p.x, gy + p.y),
                        Command::LineTo(p) => pb.line_to(gx + p.x, gy + p.y),
                        Command::QuadTo(c, p) => pb.quad_to(gx + c.x, gy + c.y, gx + p.x, gy + p.y),
                        Command::CurveTo(c1, c2, p) => pb.cubic_to(
                            gx + c1.x,
                            gy + c1.y,
                            gx + c2.x,
                            gy + c2.y,
                            gx + p.x,
                            gy + p.y,
                        ),
                        Command::Close => pb.close(),
                    }
                }
                if let Some(path) = pb.finish() {
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::RenderCommand;
    use crate::Color;

    #[test]
    fn render_text_produces_ink() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::Text {
            x: 10.0,
            y: 10.0,
            text: "Hello".to_string(),
            font_size: 24.0,
            color: Color::rgb(0, 0, 0),
        }];
        backend.render(&cmds, 200, 50);
        let pixmap = backend.pixmap().expect("pixmap allocated");

        // 统计非白像素（文字墨迹）；白画布（P3-5）下文字应为黑色像素。
        let mut ink = 0usize;
        for py in 0..pixmap.height() {
            for px in 0..pixmap.width() {
                let p = pixmap.pixel(px, py).expect("pixel in range");
                if p.red() < 200 || p.green() < 200 || p.blue() < 200 {
                    ink += 1;
                }
            }
        }
        assert!(ink > 0, "text should produce non-white (ink) pixels");
    }

    #[test]
    fn render_single_red_rect() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::rect(
            0.0,
            0.0,
            100.0,
            50.0,
            Color::rgb(255, 0, 0),
        )];
        backend.render(&cmds, 100, 50);

        let pixmap = backend.pixmap().expect("pixmap should be allocated");
        assert_eq!(pixmap.width(), 100);
        assert_eq!(pixmap.height(), 50);

        // 左上角像素应为红色（不透明）
        let pixel = pixmap.pixel(0, 0).expect("pixel in range");
        assert_eq!(pixel.red(), 255);
        assert_eq!(pixel.green(), 0);
        assert_eq!(pixel.blue(), 0);
        assert_eq!(pixel.alpha(), 255);
    }

    #[test]
    fn render_empty_commands_produces_white_canvas() {
        // P3-5：画布默认填白，空指令也产出白底而非透明。
        let mut backend = TinySkiaBackend::new();
        backend.render(&[], 10, 10);
        let pixmap = backend.pixmap().expect("pixmap allocated");
        let pixel = pixmap.pixel(0, 0).expect("in range");
        assert_eq!(pixel.red(), 255);
        assert_eq!(pixel.green(), 255);
        assert_eq!(pixel.blue(), 255);
        assert_eq!(pixel.alpha(), 255);
    }

    #[test]
    fn render_skips_zero_size_rect() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![
            RenderCommand::Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 10.0,
                background: Some(Color::rgb(255, 0, 0)),
                border: None,
            },
            RenderCommand::Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 0.0,
                background: Some(Color::rgb(0, 255, 0)),
                border: None,
            },
        ];
        backend.render(&cmds, 10, 10);
        // 零尺寸矩形不应绘制 → 画布保持白色（P3-5）
        let pixmap = backend.pixmap().expect("pixmap allocated");
        let pixel = pixmap.pixel(0, 0).expect("in range");
        assert_eq!(pixel.alpha(), 255, "white canvas — zero-size rects skipped");
        assert_eq!(pixel.red(), 255);
    }

    #[test]
    fn render_with_border() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::Rect {
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 60.0,
            background: None,
            border: Some(Border {
                width: 2.0,
                color: Color::rgb(0, 0, 255),
                style: BorderStyle::Solid,
            }),
        }];
        backend.render(&cmds, 100, 100);

        let pixmap = backend.pixmap().expect("pixmap allocated");
        // 边框外（左上角）应为白底画布（P3-5）
        let outside = pixmap.pixel(0, 0).expect("in range");
        assert_eq!(
            outside.alpha(),
            255,
            "outside border should be white canvas"
        );
        assert_eq!(outside.red(), 255, "white canvas");

        // 边框中心像素 (y=10) 应为蓝色
        // stroke 中心对齐到 (x+1, y+1)，所以 (50, 10) 应在边框顶部
        let border_pixel = pixmap.pixel(50, 10).expect("in range");
        assert_eq!(border_pixel.alpha(), 255, "border should be opaque");
        assert_eq!(border_pixel.blue(), 255, "border should be blue");
    }

    #[test]
    fn encode_png_returns_valid_bytes() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::rect(
            0.0,
            0.0,
            10.0,
            10.0,
            Color::rgb(0, 255, 0),
        )];
        backend.render(&cmds, 10, 10);
        let png = backend.encode_png().expect("encode should succeed");
        // PNG magic header
        assert!(png.len() > 8);
        assert_eq!(
            &png[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
    }

    #[test]
    fn encode_png_without_render_returns_error() {
        let backend = TinySkiaBackend::new();
        assert!(backend.encode_png().is_err());
    }

    #[test]
    fn save_png_creates_file() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::rect(
            0.0,
            0.0,
            4.0,
            4.0,
            Color::rgb(255, 0, 0),
        )];
        backend.render(&cmds, 4, 4);

        let tmp = std::env::temp_dir().join("muskitty_tiny_skia_test.png");
        backend.save_png(&tmp).expect("save should succeed");
        assert!(tmp.exists(), "PNG file should exist");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn render_partial_transparency_alpha() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::rect(
            0.0,
            0.0,
            10.0,
            10.0,
            Color::rgba(255, 0, 0, 128),
        )];
        backend.render(&cmds, 10, 10);
        let pixmap = backend.pixmap().expect("pixmap allocated");
        let pixel = pixmap.pixel(5, 5).expect("in range");
        // P3-5 白画布下，半透明红 source-over 混合到白底 →
        // (≈255, 127, 127) 粉色，alpha 变为不透明。
        assert_eq!(pixel.alpha(), 255, "white canvas behind is opaque");
        assert!(
            pixel.red() > 200,
            "red should dominate, got {}",
            pixel.red()
        );
        assert!(
            pixel.green() > 100 && pixel.green() < 160,
            "green ~127 from white canvas, got {}",
            pixel.green()
        );
    }
}
