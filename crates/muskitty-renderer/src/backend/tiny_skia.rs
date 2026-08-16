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

use cosmic_text::{
    Attrs, Buffer, Command, Family, FontSystem, Metrics, Shaping, SwashCache, Weight,
};
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::backend::{Backend, RenderOutput};
use crate::color::Color;
use crate::command::{Border, BorderStyle, RenderCommand};

/// tiny-skia CPU 渲染后端。
///
/// `render` 后渲染结果通过 [`RenderOutput::Pixels`] 返回，PNG 编码走
/// [`encode_png`](Self::encode_png) / [`save_png`](Self::save_png)。
/// 内部 [`Pixmap`] 为私有实现，不暴露 tiny-skia 类型。
#[derive(Debug, Default)]
pub struct TinySkiaBackend {
    pixmap: Option<Pixmap>,
}

impl TinySkiaBackend {
    /// 创建空后端（尚未渲染）。
    pub fn new() -> Self {
        Self::default()
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

        // 裁剪栈（L-2）：Clip/EndClip 维护，栈顶作为后续绘制的 clip_mask。
        let canvas_w = width;
        let canvas_h = height;
        let mut clip_stack: Vec<Mask> = Vec::new();

        for cmd in commands {
            let clip = clip_stack.last();
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
                            pixmap.fill_rect(rect, &paint, Transform::identity(), clip);
                        }
                    }

                    // 绘制边框
                    if let Some(b) = border {
                        if b.width > 0.0 {
                            draw_border(&mut pixmap, *x, *y, *width, *height, b, clip);
                        }
                    }
                }
                RenderCommand::Text {
                    x,
                    y,
                    text,
                    font_size,
                    font_family,
                    font_weight,
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
                        font_family,
                        *font_weight,
                        *color,
                        font_system.as_mut().expect("font_system just initialized"),
                        swash_cache.as_mut().expect("swash_cache just initialized"),
                        clip,
                    );
                }
                RenderCommand::Clip {
                    x,
                    y,
                    width,
                    height,
                } => {
                    // 用矩形 path 裁剪：有栈顶则与栈顶相交，否则新建 mask 填充。
                    let rect = match Rect::from_xywh(*x, *y, *width, *height) {
                        Some(r) => r,
                        None => continue,
                    };
                    let path = PathBuilder::from_rect(rect);
                    let mask = match clip {
                        Some(top) => {
                            let mut m = top.clone();
                            m.intersect_path(
                                &path,
                                FillRule::Winding,
                                false,
                                Transform::identity(),
                            );
                            m
                        }
                        None => {
                            let mut m = Mask::new(canvas_w, canvas_h).expect("mask alloc");
                            m.fill_path(&path, FillRule::Winding, false, Transform::identity());
                            m
                        }
                    };
                    clip_stack.push(mask);
                }
                RenderCommand::EndClip => {
                    clip_stack.pop();
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
fn draw_border(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border: &Border,
    clip_mask: Option<&Mask>,
) {
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
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), clip_mask);
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
    font_family: &str,
    font_weight: u16,
    color: Color,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    clip_mask: Option<&Mask>,
) {
    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
    // 单行（不换行）。
    buffer.set_size(font_system, None, None);
    let attrs = Attrs::new()
        .family(family_from_css(font_family))
        .weight(Weight(font_weight));
    buffer.set_text(font_system, text, attrs, Shaping::Advanced);

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
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, clip_mask);
                }
            }
        }
    }
}

/// CSS 字体族名 → cosmic-text [`Family`]（与 layout 侧 text.rs 保持一致）。
fn family_from_css(name: &str) -> Family<'_> {
    let trimmed = name.trim();
    match trimmed.to_ascii_lowercase().as_str() {
        "serif" => Family::Serif,
        "sans-serif" => Family::SansSerif,
        "monospace" => Family::Monospace,
        "cursive" => Family::Cursive,
        "fantasy" => Family::Fantasy,
        _ => Family::Name(trimmed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::RenderCommand;
    use crate::Color;

    /// 渲染并取出 RGBA 像素数据（width, height, data）。
    fn render_pixels(
        backend: &mut TinySkiaBackend,
        cmds: &[RenderCommand],
        w: u32,
        h: u32,
    ) -> (u32, u32, Vec<u8>) {
        match backend.render(cmds, w, h) {
            RenderOutput::Pixels {
                width,
                height,
                data,
            } => (width, height, data),
            _ => panic!("expected Pixels"),
        }
    }

    /// 读取 (x, y) 处的 RGBA 像素（8-bit per channel）。
    fn pixel(data: &[u8], width: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
        let i = ((y * width + x) * 4) as usize;
        (data[i], data[i + 1], data[i + 2], data[i + 3])
    }

    #[test]
    fn render_text_produces_ink() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::Text {
            x: 10.0,
            y: 10.0,
            text: "Hello".to_string(),
            font_size: 24.0,
            font_family: "serif".to_string(),
            font_weight: 400,
            color: Color::rgb(0, 0, 0),
        }];
        let (width, height, data) = render_pixels(&mut backend, &cmds, 200, 50);

        // 统计非白像素（文字墨迹）；白画布（P3-5）下文字应为黑色像素。
        let mut ink = 0usize;
        for py in 0..height {
            for px in 0..width {
                let (r, g, b, _) = pixel(&data, width, px, py);
                if r < 200 || g < 200 || b < 200 {
                    ink += 1;
                }
            }
        }
        assert!(ink > 0, "text should produce non-white (ink) pixels");
    }

    #[test]
    fn clip_crops_subsequent_rect() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![
            RenderCommand::Clip {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
            RenderCommand::rect(0.0, 0.0, 100.0, 100.0, Color::rgb(255, 0, 0)),
            RenderCommand::EndClip,
        ];
        let (width, _, data) = render_pixels(&mut backend, &cmds, 100, 100);

        // clip 内（10,10）应为红色。
        let (r, _, _, _) = pixel(&data, width, 10, 10);
        assert_eq!(r, 255, "inside clip should be red");
        // clip 外（60,60）应保持白底（裁剪生效）。
        let (r, g, b, _) = pixel(&data, width, 60, 60);
        assert_eq!(
            (r, g, b),
            (255, 255, 255),
            "outside clip should remain white canvas"
        );
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
        let (width, height, data) = render_pixels(&mut backend, &cmds, 100, 50);
        assert_eq!(width, 100);
        assert_eq!(height, 50);

        // 左上角像素应为红色（不透明）
        let (r, g, b, a) = pixel(&data, width, 0, 0);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
        assert_eq!(a, 255);
    }

    #[test]
    fn render_empty_commands_produces_white_canvas() {
        // P3-5：画布默认填白，空指令也产出白底而非透明。
        let mut backend = TinySkiaBackend::new();
        let (width, _, data) = render_pixels(&mut backend, &[], 10, 10);
        let (r, g, b, a) = pixel(&data, width, 0, 0);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);
        assert_eq!(a, 255);
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
        let (width, _, data) = render_pixels(&mut backend, &cmds, 10, 10);
        // 零尺寸矩形不应绘制 → 画布保持白色（P3-5）
        let (r, _, _, a) = pixel(&data, width, 0, 0);
        assert_eq!(a, 255, "white canvas — zero-size rects skipped");
        assert_eq!(r, 255);
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
        let (width, _, data) = render_pixels(&mut backend, &cmds, 100, 100);

        // 边框外（左上角）应为白底画布（P3-5）
        let (r, _, _, a) = pixel(&data, width, 0, 0);
        assert_eq!(a, 255, "outside border should be white canvas");
        assert_eq!(r, 255, "white canvas");

        // 边框中心像素 (y=10) 应为蓝色
        // stroke 中心对齐到 (x+1, y+1)，所以 (50, 10) 应在边框顶部
        let (_, _, b, a) = pixel(&data, width, 50, 10);
        assert_eq!(a, 255, "border should be opaque");
        assert_eq!(b, 255, "border should be blue");
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
        let (width, _, data) = render_pixels(&mut backend, &cmds, 10, 10);
        let (r, g, _, a) = pixel(&data, width, 5, 5);
        // P3-5 白画布下，半透明红 source-over 混合到白底 →
        // (≈255, 127, 127) 粉色，alpha 变为不透明。
        assert_eq!(a, 255, "white canvas behind is opaque");
        assert!(r > 200, "red should dominate, got {}", r);
        assert!(
            g > 100 && g < 160,
            "green ~127 from white canvas, got {}",
            g
        );
    }
}
