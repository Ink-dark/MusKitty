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
//! # 边框 / 轮廓绘制
//!
//! - CSS border box 外边缘 = `Rect { x, y, width, height }`（taffy 布局结果
//!   即 border box，见 layout `style_map`）
//! - 四边各自填充一条 border box 内缘的矩形条：上/下取全长，左/右纵向内缩
//!   上/下边宽度（corner 方块拼接，非浏览器的 miter 斜接）
//! - 轮廓绘制在 border box **之外**（`outline-offset` 固定 0）
//! - dashed / dotted / double / groove / ridge / inset / outset 当前按 solid
//!   渲染（各自需 dash 模式 / 分线 / 明暗合成）

use cosmic_text::{
    Attrs, Buffer, Command, Family, FontSystem, Metrics, Shaping, SwashCache, Weight,
};
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Transform};

use crate::backend::{Backend, RenderOutput};
use crate::color::Color;
use crate::command::{Border, BorderStyle, RenderCommand, TextAlign};

/// tiny-skia CPU 渲染后端。
///
/// `render` 后渲染结果通过 [`RenderOutput::Pixels`] 返回，PNG 编码走
/// [`encode_png`](Self::encode_png) / [`save_png`](Self::save_png)。
/// 内部 [`Pixmap`] 为私有实现，不暴露 tiny-skia 类型。
///
/// F-12（审计 R-M3）：文本光栅化上下文（cosmic-text `FontSystem` /
/// `SwashCache`）随 backend 跨 `render` 调用持久化——修复前每次 render
/// 重建并全量扫描系统字体（Windows 上数十至数百 ms），交互 resize 期间
/// 每帧重扫导致明显卡顿。字段私有，cosmic-text 类型不进入公共 API
/// （外部依赖解耦约定）。
#[derive(Default)]
pub struct TinySkiaBackend {
    pixmap: Option<Pixmap>,
    /// 首个 Text 命令到达时懒创建，跨 render 复用。
    font_system: Option<FontSystem>,
    /// 与 `font_system` 同生命周期；swash 字形缓存依赖其复用。
    swash_cache: Option<SwashCache>,
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
    fn render(
        &mut self,
        commands: &[RenderCommand],
        width: u32,
        height: u32,
        scale: f32,
    ) -> RenderOutput {
        // scale 为 HiDPI 因子（物理像素 ÷ 逻辑像素，W-2）。非正/NaN 回退 1.0。
        let scale = if scale > 0.0 { scale } else { 1.0 };

        // 物理分辨率 = round(逻辑 × scale)。Pixmap::new 返回 None 当 width/height
        // 为 0 或超过 i32::MAX/4 → 回退 1x1 像素以避免 panic。
        // F-11（审计 R-M1）：回退时**同步**钳制 phys_w/phys_h——修复前
        // `width=1, scale=0.4` 产出 0×0 的报告尺寸配 1×1 像素缓冲（长度
        // 自相矛盾），且首个 Clip 命令按 0×0 建 Mask → `.expect` panic。
        let mut phys_w = ((width as f32) * scale).round() as u32;
        let mut phys_h = ((height as f32) * scale).round() as u32;
        let mut pixmap = match Pixmap::new(phys_w, phys_h) {
            Some(p) => p,
            None => {
                phys_w = 1;
                phys_h = 1;
                Pixmap::new(1, 1).expect("1x1 pixmap always succeeds")
            }
        };

        // P3-5: 画布默认填白（根元素背景传播的简化近似，见审计文档）。
        // 元素指令按序覆盖其上；未覆盖区域呈现白色而非透明。
        // 白底覆盖整张物理画布，用物理尺寸 + identity（设备空间），无需缩放。
        if let Some(canvas) = Rect::from_xywh(0.0, 0.0, phys_w as f32, phys_h as f32) {
            let mut white = Paint::default();
            white.set_color_rgba8(255, 255, 255, 255);
            pixmap.fill_rect(canvas, &white, Transform::identity(), None);
        }

        // F-12：文本光栅化上下文持久化在 backend 上（`self.font_system` /
        // `self.swash_cache`），遇首个 Text 命令时懒创建，跨 render 复用。

        // 裁剪栈（L-2 / F-10 重构）：嵌套矩形裁剪的交集仍是矩形，故当前
        // 裁剪区用单个逻辑 rect 表示——push/pop 是 O(1) 纯数学；Mask 只
        // 保留一份懒缓存，仅当裁剪区（逻辑 rect）变化时才重建（O(画布)）。
        // 修复前每层 Clip 克隆整幅画布 Mask（4K ≈ 8 MB）并全画布求交，
        // N 层嵌套 overflow:hidden 即 O(N×画布) 内存/CPU——单页可 OOM/freeze
        // （审计 S-1）。`clip_saved` 存每层 push 前的裁剪区供 EndClip 恢复。
        //
        // RN-1：Mask 构建**延迟到绘制命令实际消费 clip 时**（Rect/Text
        // 分支调用 [`clip_mask_for`]）。修复前在命令循环顶同步：paint 层
        // 对有布局盒的 overflow 元素无条件生成 `[Clip, EndClip]` 空对，
        // 每个空对在 EndClip 迭代触发一次整画布 `Mask::new` + `fill_path`
        // ——10 万个兄弟空 `overflow:hidden` div × 1080p ≈ 200 GB 无效
        // memset（CPU 挂起）；空对现在零成本。
        let mut clip_saved: Vec<Option<Rect>> = Vec::new();
        let mut clip_rect: Option<Rect> = None;
        let mut clip_mask: Option<Mask> = None;
        let mut mask_built_for: Option<Rect> = None;

        // W-2：逻辑坐标 → 物理坐标的向量缩放（清晰非模糊放大）。所有 rect /
        // border / clip path 均走此变换；文本用 scale∘translate（见 draw_text）。
        let scale_xform = Transform::from_scale(scale, scale);

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
                    // RN-1：无背景且无边框的矩形不产生任何绘制，跳过
                    // （同时免去 Mask 构建）。
                    let has_border = border.as_ref().is_some_and(|b| !b.is_empty());
                    if background.is_none() && !has_border {
                        continue;
                    }
                    // RN-1：实际消费 clip 时才懒构建 Mask。
                    let clip = clip_mask_for(
                        &pixmap,
                        clip_rect,
                        &mut clip_mask,
                        &mut mask_built_for,
                        scale_xform,
                    );

                    // 填充背景
                    if let Some(bg) = background {
                        if let Some(rect) = Rect::from_xywh(*x, *y, *width, *height) {
                            let mut paint = Paint::default();
                            paint.set_color_rgba8(bg.r, bg.g, bg.b, bg.a);
                            paint.anti_alias = false;
                            pixmap.fill_rect(rect, &paint, scale_xform, clip);
                        }
                    }

                    // 绘制四边边框
                    if let Some(b) = border {
                        if !b.is_empty() {
                            draw_borders(&mut pixmap, *x, *y, *width, *height, b, scale, clip);
                        }
                    }
                }
                RenderCommand::Outline {
                    x,
                    y,
                    width,
                    height,
                    outline_width,
                    color,
                    style,
                } => {
                    if *width <= 0.0 || *height <= 0.0 || *outline_width <= 0.0 {
                        continue;
                    }
                    // RN-1：实际消费 clip 时才懒构建 Mask。
                    let clip = clip_mask_for(
                        &pixmap,
                        clip_rect,
                        &mut clip_mask,
                        &mut mask_built_for,
                        scale_xform,
                    );
                    draw_outline(
                        &mut pixmap,
                        *x,
                        *y,
                        *width,
                        *height,
                        *outline_width,
                        *color,
                        *style,
                        scale,
                        clip,
                    );
                }
                RenderCommand::Text {
                    x,
                    y,
                    width,
                    text,
                    font_size,
                    line_height,
                    font_family,
                    font_weight,
                    text_align,
                    color,
                } => {
                    let font_system = self.font_system.get_or_insert_with(FontSystem::new);
                    let swash_cache = self.swash_cache.get_or_insert_with(SwashCache::new);
                    // RN-1：实际消费 clip 时才懒构建 Mask。
                    let clip = clip_mask_for(
                        &pixmap,
                        clip_rect,
                        &mut clip_mask,
                        &mut mask_built_for,
                        scale_xform,
                    );
                    draw_text(
                        &mut pixmap,
                        *x,
                        *y,
                        *width,
                        text,
                        *font_size,
                        *line_height,
                        font_family,
                        *font_weight,
                        *text_align,
                        *color,
                        scale,
                        font_system,
                        swash_cache,
                        clip,
                    );
                }
                RenderCommand::Clip {
                    x,
                    y,
                    width,
                    height,
                } => {
                    // 用矩形 rect 表示裁剪：有当前裁剪区则求交（纯数学），
                    // 否则直接作为新裁剪区。mask 由懒同步统一重建。
                    let Some(rect) = Rect::from_xywh(*x, *y, *width, *height) else {
                        continue;
                    };
                    clip_saved.push(clip_rect);
                    clip_rect = Some(match clip_rect {
                        Some(cur) => intersect_rect(cur, rect),
                        None => rect,
                    });
                }
                RenderCommand::EndClip => {
                    clip_rect = clip_saved.pop().flatten();
                }
            }
        }

        self.pixmap = Some(pixmap);

        // P2-18：返回像素数据（RGBA，行长 = width*4）。`pixmap.data()` 保持
        // 引用有效直到赋值前，故先取引用再 move。
        let p = self.pixmap.as_ref().expect("pixmap just set");
        RenderOutput::Pixels {
            width: phys_w,
            height: phys_h,
            data: p.data().to_vec(),
        }
    }
}

/// 两个逻辑 rect 的交集（F-10）。
///
/// 空交集返回零面积退化 rect：`fill_path` 填出全零 mask → 后续绘制全部
/// 被裁掉（与旧实现"空交集 mask 不可见"效果一致）。
fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let degenerate = || Rect::from_xywh(0.0, 0.0, 0.0, 0.0).expect("degenerate rect");
    let left = a.left().max(b.left());
    let top = a.top().max(b.top());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    if right > left && bottom > top {
        Rect::from_xywh(left, top, right - left, bottom - top).unwrap_or_else(degenerate)
    } else {
        degenerate()
    }
}

/// RN-1：按需构建（或复用）当前裁剪区的 Mask。
///
/// 仅在绘制命令（Rect/Text）实际消费 clip 时调用：`built_for` 与当前
/// 裁剪区一致则直接复用缓存；变化才重建（O(画布)）；无裁剪区返回
/// `None`。`[Clip, EndClip]` 空对不再触发任何 Mask 分配。
fn clip_mask_for<'a>(
    pixmap: &Pixmap,
    clip_rect: Option<Rect>,
    clip_mask: &'a mut Option<Mask>,
    built_for: &mut Option<Rect>,
    scale_xform: Transform,
) -> Option<&'a Mask> {
    if *built_for != clip_rect {
        *clip_mask = clip_rect.map(|r| {
            let mut m = Mask::new(pixmap.width(), pixmap.height()).expect("mask alloc");
            m.fill_path(
                &PathBuilder::from_rect(r),
                FillRule::Winding,
                false,
                scale_xform,
            );
            m
        });
        *built_for = clip_rect;
    }
    clip_mask.as_ref()
}

/// 绘制四边边框（每个可见边填充一条 border box 内缘的矩形条）。
///
/// 与旧的 inset-rect stroke 相比（M-3 batch 2）：四边可以有各自的宽/色/样式。
/// corner 处理为**方块拼接**——上/下边取全长，左/右边纵向内缩上/下边宽度；
/// 浏览器对相邻不同宽度的边用 miter 斜接（梯形），此处为已记录近似。
/// dashed/dotted/double 与明暗类（groove/ridge/inset/outset）按 solid 近似。
#[allow(clippy::too_many_arguments)]
fn draw_borders(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border: &Border,
    scale: f32,
    clip_mask: Option<&Mask>,
) {
    let (top, right, bottom, left) = border.widths();
    // 左右边纵向内缩上/下边宽度（corner 归上/下所有）
    let inner_h = (height - top - bottom).max(0.0);

    let mut fill = |bx: f32, by: f32, bw: f32, bh: f32, side: crate::command::SideBorder| {
        if bw <= 0.0 || bh <= 0.0 || side.width <= 0.0 {
            return;
        }
        let Some(rect) = Rect::from_xywh(bx, by, bw, bh) else {
            return;
        };
        let mut paint = Paint::default();
        paint.set_color_rgba8(side.color.r, side.color.g, side.color.b, side.color.a);
        // 整数宽度走无抗锯齿填充（边缘落在像素边界，避免半透明毛边）；
        // 亚像素宽度（如 0.5px）开抗锯齿，否则会被吸附成 1px 或消失。
        paint.anti_alias = side.width.fract() != 0.0;
        pixmap.fill_rect(rect, &paint, Transform::from_scale(scale, scale), clip_mask);
    };

    if let Some(side) = border.top {
        fill(x, y, width, top, side);
    }
    if let Some(side) = border.bottom {
        fill(x, y + height - bottom, width, bottom, side);
    }
    if let Some(side) = border.left {
        fill(x, y + top, left, inner_h, side);
    }
    if let Some(side) = border.right {
        fill(x + width - right, y + top, right, inner_h, side);
    }
}

/// 绘制轮廓（CSS UI Level 4 §4，M-3 batch 2）：border box **外侧**四条矩形条。
///
/// `outline-offset` 未实现（固定 0），故轮廓紧贴 border box 外缘向外展开
/// `outline_width`。轮廓不参与布局（由 layout 层保证：map_style 不读 outline）。
#[allow(clippy::too_many_arguments)]
fn draw_outline(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    outline_width: f32,
    color: Color,
    style: BorderStyle,
    scale: f32,
    clip_mask: Option<&Mask>,
) {
    if !style.is_painted() || outline_width <= 0.0 {
        return;
    }
    let ow = outline_width;
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = ow.fract() != 0.0;
    let xform = Transform::from_scale(scale, scale);

    // 上/下取全长并向左右各外扩 ow；左/右纵向只覆盖 border box 高度，
    // 四条拼成闭合环（corner 归上/下所有，同 border 的方块拼接近似）。
    let strips = [
        (x - ow, y - ow, width + 2.0 * ow, ow),     // top
        (x - ow, y + height, width + 2.0 * ow, ow), // bottom
        (x - ow, y, ow, height),                    // left
        (x + width, y, ow, height),                 // right
    ];
    for (sx, sy, sw, sh) in strips {
        if let Some(rect) = Rect::from_xywh(sx, sy, sw, sh) {
            pixmap.fill_rect(rect, &paint, xform, clip_mask);
        }
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
    width: f32,
    text: &str,
    font_size: f32,
    line_height: f32,
    font_family: &str,
    font_weight: u16,
    text_align: TextAlign,
    color: Color,
    scale: f32,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    clip_mask: Option<&Mask>,
) {
    // M-3 batch 3：行高来自 Text 命令（cascade `used_line_height_px` 的使用值），
    // 不再用 `font_size * 1.2` —— 与 layout 测量的行高一致，否则绘制行位置
    // 与布局盒高对不上（T-3 的"汉字位移"教训）。
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
    // 按布局宽度换行（T-3）。
    buffer.set_size(font_system, Some(width), None);
    let attrs = Attrs::new()
        .family(family_from_css(font_family))
        .weight(Weight(font_weight));
    buffer.set_text(font_system, text, attrs, Shaping::Advanced);

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;

    // glyph 物理坐标相对 buffer 原点，经 Transform 平移到 Text 命令位置。
    // W-2：scale∘translate = from_row(s,0,0,s,s*x,s*y)——先缩放 glyph 相对
    // 偏移，再平移并缩放盒原点（逻辑 (x,y) → 物理 (s*x, s*y)）。
    let transform = Transform::from_row(scale, 0.0, 0.0, scale, scale * x, scale * y);

    for run in buffer.layout_runs() {
        // 行水平偏移（text-align，T-3）。
        let line_offset = match text_align {
            TextAlign::Left => 0.0,
            TextAlign::Center => (width - run.line_w) / 2.0,
            TextAlign::Right => width - run.line_w,
        };
        for glyph in run.glyphs {
            // physical() 返回行内局部物理坐标（含 baseline）；多行时需加
            // run.line_y（行顶偏移），否则各行叠在首行位置（T-3）。
            let physical = glyph.physical((0.0, 0.0), 1.0);
            let gx = physical.x as f32 + line_offset;
            let gy = physical.y as f32 + run.line_y;
            let cache_key = physical.cache_key;
            if let Some(commands) = swash_cache.get_outline_commands(font_system, cache_key) {
                let mut pb = PathBuilder::new();
                for cmd in commands {
                    // swash outline 是 y-up（baseline 原点，见 cosmic-text
                    // `with_pixels` 中 `y = -placement.top` 的取反），而画布
                    // y 向下：outline 的 +y 应落在 baseline 上方，故取 `gy - p.y`。
                    // 误加 `p.y` 会让每个字形垂直翻转（W-1 真窗口发现的 bug）。
                    match *cmd {
                        Command::MoveTo(p) => pb.move_to(gx + p.x, gy - p.y),
                        Command::LineTo(p) => pb.line_to(gx + p.x, gy - p.y),
                        Command::QuadTo(c, p) => pb.quad_to(gx + c.x, gy - c.y, gx + p.x, gy - p.y),
                        Command::CurveTo(c1, c2, p) => pb.cubic_to(
                            gx + c1.x,
                            gy - c1.y,
                            gx + c2.x,
                            gy - c2.y,
                            gx + p.x,
                            gy - p.y,
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
    use crate::command::{RenderCommand, SideBorder};
    use crate::Color;

    /// 渲染并取出 RGBA 像素数据（width, height, data）。
    fn render_pixels(
        backend: &mut TinySkiaBackend,
        cmds: &[RenderCommand],
        w: u32,
        h: u32,
        scale: f32,
    ) -> (u32, u32, Vec<u8>) {
        match backend.render(cmds, w, h, scale) {
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
    fn glyph_is_not_vertically_flipped() {
        // 回归测试（W-1 真窗口发现"文字全部反翻"）：swash outline 是 y-up
        // （baseline 原点），绘制须 `baseline_y - p.y`；若误加 `p.y` 则每个
        // 字形垂直翻转。用大写 "T" 判别：横杠在 cap 顶部、竖干伸到 baseline。
        // 正确渲染 → 顶部墨迹行平均宽度 > 底部；翻转后横杠落到底部 → 反之。
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::Text {
            x: 10.0,
            y: 20.0,
            width: 200.0,
            text: "T".to_string(),
            font_size: 64.0,
            line_height: 64.0 * 1.2,
            font_family: "serif".to_string(),
            font_weight: 400,
            text_align: TextAlign::Left,
            color: Color::rgb(0, 0, 0),
        }];
        // 画布足够高，让翻转后的字形（横杠落到 baseline 之下）完整可见。
        let (width, height, data) = render_pixels(&mut backend, &cmds, 200, 160, 1.0);

        // 每行墨迹的像素数与水平范围。
        let mut counts = vec![0usize; height as usize];
        let mut min_x = vec![i32::MAX; height as usize];
        let mut max_x = vec![i32::MIN; height as usize];
        for py in 0..height {
            for px in 0..width {
                let (r, g, b, _) = pixel(&data, width, px, py);
                if r < 200 || g < 200 || b < 200 {
                    let i = py as usize;
                    counts[i] += 1;
                    min_x[i] = min_x[i].min(px as i32);
                    max_x[i] = max_x[i].max(px as i32);
                }
            }
        }

        // 墨迹行的垂直范围。
        let ink_rows: Vec<usize> = counts
            .iter()
            .enumerate()
            .filter(|(_, c)| **c > 0)
            .map(|(i, _)| i)
            .collect();
        let top = *ink_rows.first().expect("glyph has ink");
        let bottom = *ink_rows.last().expect("glyph has ink");
        assert!(
            bottom - top > 4,
            "glyph should span several rows, got top={top} bottom={bottom}"
        );

        // 顶部四分之一 vs 底部四分之一墨迹行的平均宽度。
        let avg_width = |rows: &[usize]| -> f32 {
            rows.iter()
                .map(|&i| (max_x[i] - min_x[i]) as f32)
                .sum::<f32>()
                / rows.len() as f32
        };
        let top_w = avg_width(&ink_rows[..ink_rows.len() / 4]);
        let bottom_w = avg_width(&ink_rows[ink_rows.len() * 3 / 4..]);
        assert!(
            top_w > bottom_w,
            "capital T's horizontal bar must sit at the TOP: top-quarter avg width {top_w:.1} \
             > bottom-quarter {bottom_w:.1}; glyph is vertically flipped"
        );
    }

    #[test]
    fn render_text_produces_ink() {
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::Text {
            x: 10.0,
            y: 10.0,
            width: 200.0,
            text: "Hello".to_string(),
            font_size: 24.0,
            line_height: 24.0 * 1.2,
            font_family: "serif".to_string(),
            font_weight: 400,
            text_align: TextAlign::Left,
            color: Color::rgb(0, 0, 0),
        }];
        let (width, height, data) = render_pixels(&mut backend, &cmds, 200, 50, 1.0);

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
        let (width, _, data) = render_pixels(&mut backend, &cmds, 100, 100, 1.0);

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
        let (width, height, data) = render_pixels(&mut backend, &cmds, 100, 50, 1.0);
        assert_eq!(width, 100);
        assert_eq!(height, 50);

        // 左上角像素应为红色（不透明）
        let (r, g, b, a) = pixel(&data, width, 0, 0);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);
        assert_eq!(a, 255);
    }

    // —— M-3 batch 3: Text 命令的 line_height 决定多行位置 ——

    /// 渲染单条 Text 命令并返回「最底部墨迹行号」。
    fn last_ink_row(text: &str, font_size: f32, line_height: f32, width: f32) -> (u32, usize) {
        let cmds = vec![RenderCommand::Text {
            x: 0.0,
            y: 0.0,
            width,
            text: text.to_string(),
            font_size,
            line_height,
            font_family: "serif".to_string(),
            font_weight: 400,
            text_align: TextAlign::Left,
            color: Color::rgb(0, 0, 0),
        }];
        let mut backend = TinySkiaBackend::new();
        let (w, h, data) = render_pixels(&mut backend, &cmds, 60, 160, 1.0);
        // 自下而上找首个含墨迹的行（clippy 1.98 不再接受 filter(..).next_back()）
        let last = (0..h)
            .rev()
            .find(|&y| {
                (0..w).any(|x| {
                    let i = ((y * w + x) * 4) as usize;
                    data[i] < 200
                })
            })
            .unwrap_or(0) as usize;
        (w, last)
    }

    #[test]
    fn text_line_height_controls_multiline_positions() {
        // 同一文本在同一宽度下换行（行数由换行决定，与行高无关）；行高只
        // 影响各行纵向位置——行高 40 的第二行必须落在行高 10 的下方。
        // 这是 backend 侧对「line_height 来自 Text 命令」的直接断言（此前
        // 后端硬编码 `font_size * 1.2`，三处行高必然相同）。
        let text = "one two three four five six";
        let (_, small) = last_ink_row(text, 10.0, 10.0, 40.0);
        let (_, normal) = last_ink_row(text, 10.0, 12.0, 40.0);
        let (_, big) = last_ink_row(text, 10.0, 40.0, 40.0);
        assert!(
            small > 0 && normal > 0,
            "wrapped text must ink multiple rows (small={small}, normal={normal})"
        );
        assert!(
            big >= small + 20,
            "40px line-height must push the last line far below the 10px case: big={big} small={small}"
        );
        assert!(
            big > normal,
            "40px line-height must exceed the 12px (1.2em) default: big={big} normal={normal}"
        );
    }

    // —— F-10: 裁剪栈有界化 ——

    #[test]
    fn font_system_lazily_created_and_persisted() {
        // F-12（审计 R-M3）：文本上下文仅随首个 Text 命令创建，跨 render
        // 持久化（修复前每次 render 重建并重扫系统字体）。
        let mut backend = TinySkiaBackend::new();
        let cmds_rect = vec![RenderCommand::rect(
            0.0,
            0.0,
            10.0,
            10.0,
            Color::rgb(255, 0, 0),
        )];
        let _ = render_pixels(&mut backend, &cmds_rect, 10, 10, 1.0);
        assert!(
            backend.font_system.is_none(),
            "rect-only render must not create font context"
        );
        let cmds_text = vec![RenderCommand::Text {
            x: 1.0,
            y: 1.0,
            width: 40.0,
            text: "T".to_string(),
            font_size: 12.0,
            line_height: 12.0 * 1.2,
            font_family: "serif".to_string(),
            font_weight: 400,
            text_align: TextAlign::Left,
            color: Color::rgb(0, 0, 0),
        }];
        let _ = render_pixels(&mut backend, &cmds_text, 50, 20, 1.0);
        assert!(
            backend.font_system.is_some() && backend.swash_cache.is_some(),
            "text render must create font context"
        );
        // 后续 render（无 Text）不销毁已有上下文。
        let _ = render_pixels(&mut backend, &cmds_rect, 10, 10, 1.0);
        assert!(
            backend.font_system.is_some(),
            "font context must persist across renders"
        );
    }

    // —— F-10: 裁剪栈有界化 ——

    #[test]
    fn subpixel_scale_zero_phys_dims_stay_consistent() {
        // F-11（审计 R-M1）：width=1、scale=0.4 → round(0.4)=0。修复前
        // pixmap 回退 1×1 但报告尺寸仍为 0×0（与 4 字节缓冲自相矛盾），
        // 且首个 Clip 按 canvas 0×0 建 Mask → `.expect` panic。
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![
            RenderCommand::Clip {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            RenderCommand::rect(0.0, 0.0, 1.0, 1.0, Color::rgb(255, 0, 0)),
            RenderCommand::EndClip,
        ];
        let (w, h, data) = render_pixels(&mut backend, &cmds, 1, 1, 0.4);
        assert_eq!((w, h), (1, 1), "reported dims must match the buffer");
        assert_eq!(data.len(), 4);
    }

    #[test]
    fn thousand_nested_clips_complete_and_restore() {
        // 审计 S-1：1000 层嵌套 Clip。修复前每层克隆整幅画布 Mask 并全画布
        // 求交（内存 O(N×画布)，4K 画布千层 ≈ 8 GB）；修复后内存 O(画布)。
        // 本测试同时是隐式性能回归测试——每层整画布克隆/求交的回归会令
        // CI 超时。
        // 裁剪 rect (i, i, 100, 100)：i≥100 后交集为空（退化 rect）。
        let mut backend = TinySkiaBackend::new();
        let mut cmds = Vec::new();
        for i in 0..1000i32 {
            let o = i as f32;
            cmds.push(RenderCommand::Clip {
                x: o,
                y: o,
                width: 100.0,
                height: 100.0,
            });
        }
        // 空交集内画红 rect → 不可见。
        cmds.push(RenderCommand::rect(
            150.0,
            150.0,
            40.0,
            40.0,
            Color::rgb(255, 0, 0),
        ));
        for _ in 0..1000 {
            cmds.push(RenderCommand::EndClip);
        }
        // 栈恢复后画红 rect → 可见（EndClip 恢复语义保持）。
        cmds.push(RenderCommand::rect(
            10.0,
            10.0,
            40.0,
            40.0,
            Color::rgb(255, 0, 0),
        ));
        let (width, height, data) = render_pixels(&mut backend, &cmds, 200, 200, 1.0);
        assert_eq!((width, height), (200, 200));
        let count_red = |data: &[u8], width: u32| -> usize {
            let mut n = 0;
            for y in 0..height {
                for x in 0..width {
                    let (r, g, b, _) = pixel(data, width, x, y);
                    if r == 255 && g == 0 && b == 0 {
                        n += 1;
                    }
                }
            }
            n
        };
        // (150,150) 在空交集内被裁掉；(10,10) 在恢复后可见。40×40 = 1600。
        assert_eq!(
            count_red(&data, width),
            1600,
            "clipped-out rect must be invisible; post-restore rect must show"
        );
    }

    #[test]
    fn nested_shrinking_clips_intersect_geometrically() {
        // 50 层递缩裁剪（每层 x/y+10、w/h-10）→ 交集 (490,490,260,260)。
        // 交集内红 rect 可见，交集外保持白底。
        let mut backend = TinySkiaBackend::new();
        let mut cmds = Vec::new();
        for i in 0..50i32 {
            let o = (i * 10) as f32;
            let s = 800.0 - o;
            cmds.push(RenderCommand::Clip {
                x: o,
                y: o,
                width: s,
                height: s,
            });
        }
        cmds.push(RenderCommand::rect(
            500.0,
            500.0,
            40.0,
            40.0,
            Color::rgb(255, 0, 0),
        ));
        cmds.push(RenderCommand::rect(
            100.0,
            100.0,
            40.0,
            40.0,
            Color::rgb(0, 255, 0),
        ));
        for _ in 0..50 {
            cmds.push(RenderCommand::EndClip);
        }
        let (width, _, data) = render_pixels(&mut backend, &cmds, 800, 800, 1.0);
        let (r, g, b) = {
            let (r, g, b, _) = pixel(&data, width, 510, 510);
            (r, g, b)
        };
        assert_eq!(
            (r, g, b),
            (255, 0, 0),
            "rect inside the intersection must render"
        );
        let (r, g, b) = {
            let (r, g, b, _) = pixel(&data, width, 110, 110);
            (r, g, b)
        };
        assert_eq!(
            (r, g, b),
            (255, 255, 255),
            "rect outside the intersection must be clipped"
        );
    }

    #[test]
    fn render_empty_commands_produces_white_canvas() {
        // P3-5：画布默认填白，空指令也产出白底而非透明。
        let mut backend = TinySkiaBackend::new();
        let (width, _, data) = render_pixels(&mut backend, &[], 10, 10, 1.0);
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
        let (width, _, data) = render_pixels(&mut backend, &cmds, 10, 10, 1.0);
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
            border: Some(Border::uniform(
                2.0,
                Color::rgb(0, 0, 255),
                BorderStyle::Solid,
            )),
        }];
        let (width, _, data) = render_pixels(&mut backend, &cmds, 100, 100, 1.0);

        // 边框外（左上角）应为白底画布（P3-5）
        let (r, _, _, a) = pixel(&data, width, 0, 0);
        assert_eq!(a, 255, "outside border should be white canvas");
        assert_eq!(r, 255, "white canvas");

        // 边框占据 border box 内缘的矩形条：y ∈ [10,12) 为顶部边框
        let (_, _, b, a) = pixel(&data, width, 50, 10);
        assert_eq!(a, 255, "border should be opaque");
        assert_eq!(b, 255, "border should be blue");
        let (_, _, b, _) = pixel(&data, width, 50, 11);
        assert_eq!(b, 255, "2px-wide top border covers the second row too");
        // 内缩 2px 之后是盒子内部（无背景 → 白底）
        let (r, _, _, _) = pixel(&data, width, 50, 12);
        assert_eq!(r, 255, "interior without background stays white canvas");
    }

    #[test]
    fn render_directional_border_only_on_that_side() {
        // M-3 batch 2: 仅左边框 → 只有最左列条着色
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::Rect {
            x: 10.0,
            y: 10.0,
            width: 30.0,
            height: 20.0,
            background: None,
            border: Some(Border {
                left: Some(SideBorder {
                    width: 4.0,
                    color: Color::rgb(255, 0, 0),
                    style: BorderStyle::Solid,
                }),
                ..Border::default()
            }),
        }];
        let (width, _, data) = render_pixels(&mut backend, &cmds, 100, 100, 1.0);

        // 左 4px 条：x ∈ [10,14) 红
        assert_eq!(pixel(&data, width, 10, 20), (255, 0, 0, 255), "left border");
        assert_eq!(pixel(&data, width, 13, 20), (255, 0, 0, 255), "left border");
        // 顶部/右边/内部均无边框 → 白底
        assert_eq!(
            pixel(&data, width, 20, 10),
            (255, 255, 255, 255),
            "no top border"
        );
        assert_eq!(
            pixel(&data, width, 39, 20),
            (255, 255, 255, 255),
            "no right border"
        );
        assert_eq!(
            pixel(&data, width, 20, 20),
            (255, 255, 255, 255),
            "interior"
        );
    }

    #[test]
    fn render_outline_rings_outside_the_box() {
        // M-3 batch 2: outline 画在 border box 外侧（不占布局）
        let mut backend = TinySkiaBackend::new();
        let cmds = vec![RenderCommand::Outline {
            x: 20.0,
            y: 20.0,
            width: 40.0,
            height: 30.0,
            outline_width: 3.0,
            color: Color::rgb(0, 128, 0),
            style: BorderStyle::Solid,
        }];
        let (width, _, data) = render_pixels(&mut backend, &cmds, 100, 100, 1.0);

        // 上边框条：y ∈ [17,20) 绿（盒子上方）
        assert_eq!(pixel(&data, width, 40, 17), (0, 128, 0, 255), "outline top");
        assert_eq!(pixel(&data, width, 40, 19), (0, 128, 0, 255), "outline top");
        // 下边框条：y ∈ [50,53)
        assert_eq!(
            pixel(&data, width, 40, 50),
            (0, 128, 0, 255),
            "outline bottom"
        );
        // 左右条：x ∈ [17,20) 与 [60,63)
        assert_eq!(
            pixel(&data, width, 17, 30),
            (0, 128, 0, 255),
            "outline left"
        );
        assert_eq!(
            pixel(&data, width, 60, 30),
            (0, 128, 0, 255),
            "outline right"
        );
        // 盒子内部不受轮廓影响 → 白底
        assert_eq!(
            pixel(&data, width, 40, 30),
            (255, 255, 255, 255),
            "interior"
        );
        // 轮廓之外仍是白底
        assert_eq!(
            pixel(&data, width, 40, 16),
            (255, 255, 255, 255),
            "above outline"
        );
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
        backend.render(&cmds, 10, 10, 1.0);
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
        backend.render(&cmds, 4, 4, 1.0);

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
        let (width, _, data) = render_pixels(&mut backend, &cmds, 10, 10, 1.0);
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

    #[test]
    fn render_scale_doubles_resolution_and_scales_rects() {
        // W-2 退出条件：同组命令 scale=1 → w×h，scale=2 → 2w×2h；scale=2 的
        // 关键逻辑点颜色与 scale=1 对应物理点一致（向量缩放，非简单插值）。
        let cmds = vec![
            RenderCommand::rect(10.0, 10.0, 20.0, 20.0, Color::rgb(255, 0, 0)),
            RenderCommand::rect(60.0, 20.0, 10.0, 10.0, Color::rgb(0, 0, 255)),
        ];

        let mut backend = TinySkiaBackend::new();
        let (w1, h1, d1) = render_pixels(&mut backend, &cmds, 100, 50, 1.0);
        assert_eq!((w1, h1), (100, 50));

        let mut backend2 = TinySkiaBackend::new();
        let (w2, h2, d2) = render_pixels(&mut backend2, &cmds, 100, 50, 2.0);
        assert_eq!((w2, h2), (200, 100));

        // 逻辑点 (15,15) 红、(65,25) 蓝、(0,0) 白：scale=1 直接读，scale=2 读物理 (2p)。
        assert_eq!(pixel(&d1, w1, 15, 15), (255, 0, 0, 255));
        assert_eq!(pixel(&d2, w2, 30, 30), (255, 0, 0, 255));
        assert_eq!(pixel(&d1, w1, 65, 25), (0, 0, 255, 255));
        assert_eq!(pixel(&d2, w2, 130, 50), (0, 0, 255, 255));
        assert_eq!(pixel(&d1, w1, 0, 0), (255, 255, 255, 255));
        assert_eq!(pixel(&d2, w2, 0, 0), (255, 255, 255, 255));
    }

    #[test]
    fn render_scale_scales_border() {
        // 边框矩形条在逻辑空间填充后随 transform 缩放：scale=2 时逻辑 2px
        // 边框 → 物理 4px（scale=1 的 2px 宽一倍）。
        let cmds = vec![RenderCommand::Rect {
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 60.0,
            background: None,
            border: Some(Border::uniform(
                2.0,
                Color::rgb(0, 0, 255),
                BorderStyle::Solid,
            )),
        }];

        let mut backend = TinySkiaBackend::new();
        let (w2, _, d2) = render_pixels(&mut backend, &cmds, 100, 100, 2.0);
        assert_eq!(w2, 200);

        // 顶部边框物理覆盖 y ∈ [20,24)（逻辑 y=10 → 物理 20，高 2 逻辑 px → 4 物理 px）
        assert_eq!(
            pixel(&d2, w2, 100, 20),
            (0, 0, 255, 255),
            "border top edge at 2x"
        );
        assert_eq!(
            pixel(&d2, w2, 100, 23),
            (0, 0, 255, 255),
            "border top edge at 2x (last physical row)"
        );
        assert_eq!(
            pixel(&d2, w2, 100, 24),
            (255, 255, 255, 255),
            "interior without background stays white canvas"
        );
    }

    #[test]
    fn render_scale_nonpositive_falls_back_to_1() {
        // scale ≤ 0（含 NaN）回退 1.0：不 panic、输出与 1x 一致。
        let cmds = vec![RenderCommand::rect(
            0.0,
            0.0,
            10.0,
            10.0,
            Color::rgb(255, 0, 0),
        )];
        let mut backend = TinySkiaBackend::new();
        let (w, h, data) = render_pixels(&mut backend, &cmds, 100, 50, 0.0);
        assert_eq!((w, h), (100, 50));
        assert_eq!(pixel(&data, w, 5, 5), (255, 0, 0, 255));
    }
}
