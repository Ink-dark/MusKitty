//! chrome 绘制（tiny-skia + cosmic-text）。
//!
//! 把 [`ChromeRects`](crate::chrome::model::ChromeRects) + [`ChromeState`]
//! 画进 tiny-skia [`Pixmap`]。文本整形用 cosmic-text、字形轮廓经 swash
//! 转为 tiny-skia 路径填充（与 muskitty-renderer `draw_text` 同方案；
//! 本地化实现以避免扩大 renderer 公共 API）。tiny-skia / cosmic-text
//! 类型只在模块内部使用，不出现在公共签名（decoupling ADR）。

use cosmic_text::{Attrs, Buffer, Command, FontSystem, Metrics, Shaping, SwashCache};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::chrome::model::{ChromeHit, ChromeRects, ChromeState, Rect};

// Chromium light 基线配色（RGB）。
const STRIP_BG: [u8; 3] = [0xde, 0xe1, 0xe6];
const TAB_ACTIVE_BG: [u8; 3] = [0xff, 0xff, 0xff];
const TAB_HOVER_BG: [u8; 3] = [0xe8, 0xea, 0xed];
const TOOLBAR_BG: [u8; 3] = [0xff, 0xff, 0xff];
const TOOLBAR_BORDER: [u8; 3] = [0xcf, 0xd4, 0xda];
const ADDRESS_BG: [u8; 3] = [0xf1, 0xf3, 0xf4];
const ADDRESS_FOCUS_BORDER: [u8; 3] = [0x1a, 0x73, 0xe8];
const TEXT: [u8; 3] = [0x3c, 0x40, 0x43];
const TEXT_PLACEHOLDER: [u8; 3] = [0x80, 0x86, 0x8b];
const ICON: [u8; 3] = [0x5f, 0x63, 0x68];
const ICON_DISABLED: [u8; 3] = [0xc1, 0xc5, 0xca];

/// 字体资源（FontSystem 加载系统字体，跨帧复用）。
pub struct ChromeAssets {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl ChromeAssets {
    /// 构造（加载系统字体）。
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }
}

impl Default for ChromeAssets {
    fn default() -> Self {
        Self::new()
    }
}

fn rgb(c: [u8; 3]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], 255)
}

fn fill(pixmap: &mut Pixmap, rect: Rect, c: [u8; 3]) {
    pixmap.fill_rect(
        tiny_skia::Rect::from_xywh(rect.x, rect.y, rect.width, rect.height).expect("valid rect"),
        &Paint {
            shader: tiny_skia::Shader::SolidColor(rgb(c)),
            anti_alias: false,
            ..Paint::default()
        },
        Transform::identity(),
        None,
    );
}

/// 圆角矩形路径（`top_only` = 只圆上角，标签形状）。
fn rounded_rect_path(rect: Rect, radius: f32, top_only: bool) -> Option<tiny_skia::Path> {
    let r = radius
        .clamp(0.0, rect.width / 2.0)
        .clamp(0.0, rect.height / 2.0);
    let x = rect.x;
    let y = rect.y;
    let w = rect.width;
    let h = rect.height;
    let mut pb = PathBuilder::new();
    if top_only {
        pb.move_to(x, y + h);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h);
    } else {
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
    }
    pb.close();
    pb.finish()
}

/// 折线路径（折线顶点序列，用于描边图标）。
fn polyline_path(points: &[(f32, f32)], closed: bool) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let (first_x, first_y) = points.first()?;
    pb.move_to(*first_x, *first_y);
    for (x, y) in &points[1..] {
        pb.line_to(*x, *y);
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

fn stroke_path(pixmap: &mut Pixmap, path: &tiny_skia::Path, c: [u8; 3], width: f32) {
    let paint = Paint {
        shader: tiny_skia::Shader::SolidColor(rgb(c)),
        anti_alias: true,
        ..Paint::default()
    };
    let stroke = Stroke {
        width,
        ..Stroke::default()
    };
    pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn fill_path(pixmap: &mut Pixmap, path: &tiny_skia::Path, c: [u8; 3]) {
    let paint = Paint {
        shader: tiny_skia::Shader::SolidColor(rgb(c)),
        anti_alias: true,
        ..Paint::default()
    };
    pixmap.fill_path(path, &paint, FillRule::Winding, Transform::identity(), None);
}

/// 测量单行文本总宽（不换行——fit_text 需要全长；若按 max_w 换行，
/// 首行宽恒 ≤ max_w，永不触发截断）。
fn measure_text(assets: &mut ChromeAssets, text: &str, font_size: f32) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let mut buffer = Buffer::new(
        &mut assets.font_system,
        Metrics::new(font_size, font_size * 1.2),
    );
    buffer.set_text(
        &mut assets.font_system,
        text,
        Attrs::new(),
        Shaping::Advanced,
    );
    buffer
        .layout_runs()
        .next()
        .map(|run| run.line_w)
        .unwrap_or(0.0)
}

/// 截断文本至 `max_w` 内（超宽加 `…`；逐字符测量，标题/地址长度有限可接受）。
fn fit_text(assets: &mut ChromeAssets, text: &str, font_size: f32, max_w: f32) -> String {
    if measure_text(assets, text, font_size) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    for ch in chars {
        let candidate = format!("{out}{ch}\u{2026}");
        if measure_text(assets, &candidate, font_size) > max_w {
            break;
        }
        out.push(ch);
    }
    format!("{out}\u{2026}")
}

/// 画一行文本（左上角 `(x, y)`，物理 px；返回实际首行宽）。
#[allow(clippy::too_many_arguments)]
fn draw_text(
    pixmap: &mut Pixmap,
    assets: &mut ChromeAssets,
    x: f32,
    y: f32,
    max_w: f32,
    text: &str,
    font_size: f32,
    color: [u8; 3],
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(
        &mut assets.font_system,
        Metrics::new(font_size, line_height),
    );
    buffer.set_size(&mut assets.font_system, Some(max_w), None);
    buffer.set_text(
        &mut assets.font_system,
        text,
        Attrs::new(),
        Shaping::Advanced,
    );

    let paint = Paint {
        shader: tiny_skia::Shader::SolidColor(rgb(color)),
        anti_alias: true,
        ..Paint::default()
    };

    let mut width: f32 = 0.0;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
        for glyph in run.glyphs {
            let physical = glyph.physical((0.0, 0.0), 1.0);
            let gx = physical.x as f32 + x;
            let gy = physical.y as f32 + y + run.line_y;
            if let Some(commands) = assets
                .swash_cache
                .get_outline_commands(&mut assets.font_system, physical.cache_key)
            {
                let mut pb = PathBuilder::new();
                // swash outline 为 y-up（baseline 原点），画布 y 向下：
                // outline +y 在 baseline 上方，故取 `gy - p.y`（取反会得到
                // 上下翻转的字形，renderer W-1 曾踩过同一坑）。
                for cmd in commands {
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
                    pixmap.fill_path(
                        &path,
                        &paint,
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
    }
    width
}

/// 画关闭按钮 `×`（两条对角线）。
fn draw_close(pixmap: &mut Pixmap, rect: Rect, c: [u8; 3], width: f32) {
    let inset = rect.width * 0.28;
    let (x0, y0) = (rect.x + inset, rect.y + inset);
    let (x1, y1) = (rect.x + rect.width - inset, rect.y + rect.height - inset);
    let path = polyline_path(&[(x0, y0), (x1, y1)], false)
        .zip(polyline_path(&[(x1, y0), (x0, y1)], false));
    if let Some((a, b)) = path {
        stroke_path(pixmap, &a, c, width);
        stroke_path(pixmap, &b, c, width);
    }
}

/// 画新建按钮 `+`（横竖两杆）。
fn draw_plus(pixmap: &mut Pixmap, rect: Rect, c: [u8; 3]) {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    let arm = rect.width * 0.32;
    let w = (rect.width * 0.09).max(1.0);
    fill(pixmap, Rect::new(cx - arm, cy - w / 2.0, arm * 2.0, w), c);
    fill(pixmap, Rect::new(cx - w / 2.0, cy - arm, w, arm * 2.0), c);
}

/// 左/右箭头（方向 `dir`：-1 左，1 右）——三角形头 + 横杆。
fn draw_arrow(pixmap: &mut Pixmap, rect: Rect, dir: f32, c: [u8; 3]) {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    let s = rect.width * 0.28;
    let tip = (cx + dir * s, cy);
    let head = polyline_path(
        &[
            (tip.0, tip.1),
            (tip.0 - dir * s * 0.8, cy - s * 0.7),
            (tip.0 - dir * s * 0.8, cy + s * 0.7),
        ],
        true,
    );
    if let Some(head) = head {
        fill_path(pixmap, &head, c);
    }
    let stem = polyline_path(&[(cx - dir * s * 0.9, cy), (cx + dir * s * 0.5, cy)], false);
    if let Some(stem) = stem {
        stroke_path(pixmap, &stem, c, (rect.width * 0.08).max(1.0));
    }
}

/// 刷新图标：270° 圆弧 + 箭头（折线近似圆弧）。
fn draw_reload(pixmap: &mut Pixmap, rect: Rect, c: [u8; 3]) {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    let r = rect.width * 0.26;
    let mut pts = Vec::new();
    // -60° → 200°（度，y 向下坐标系中顺时针视觉）。
    let steps = 20;
    for i in 0..=steps {
        let deg = -60.0 + (260.0 * i as f32) / steps as f32;
        let rad = deg.to_radians();
        pts.push((cx + r * rad.cos(), cy + r * rad.sin()));
    }
    if let Some(arc) = polyline_path(&pts, false) {
        stroke_path(pixmap, &arc, c, (rect.width * 0.08).max(1.0));
    }
    // 箭头头部在弧起点（-60°）处。
    let rad = (-60.0f32).to_radians();
    let (ax, ay) = (cx + r * rad.cos(), cy + r * rad.sin());
    let a = rect.width * 0.14;
    let head = polyline_path(
        &[(ax, ay), (ax - a, ay - a * 0.4), (ax - a * 0.4, ay - a)],
        true,
    );
    if let Some(head) = head {
        fill_path(pixmap, &head, c);
    }
}

/// 绘制整条 chrome 到 `pixmap`（页面视口区域不动，由合成器先画页面）。
///
/// `tab_titles` 与 `active_tab` 来自标签集合（每标签标题 + 活动索引）。
pub fn paint_chrome(
    pixmap: &mut Pixmap,
    rects: &ChromeRects,
    state: &ChromeState,
    tab_titles: &[&str],
    active_tab: usize,
    assets: &mut ChromeAssets,
) {
    let s = rects.scale;

    // 标签条 + 工具栏背景；工具栏底部 1px 分隔线。
    fill(pixmap, rects.tab_strip, STRIP_BG);
    fill(pixmap, rects.toolbar, TOOLBAR_BG);
    let sep_y = rects.toolbar.y + rects.toolbar.height - s;
    fill(
        pixmap,
        Rect::new(rects.toolbar.x, sep_y, rects.toolbar.width, s),
        TOOLBAR_BORDER,
    );

    // 标签：活动白底圆上角；非活动透明（hover 灰底）。
    for (i, tab) in rects.tabs.iter().enumerate() {
        let active = i == active_tab;
        let hovered = state.hover == Some(ChromeHit::Tab(i));
        if active || hovered {
            if let Some(path) = rounded_rect_path(*tab, 8.0 * s, true) {
                fill_path(
                    pixmap,
                    &path,
                    if active { TAB_ACTIVE_BG } else { TAB_HOVER_BG },
                );
            }
        }
        // 标题：左侧留 10*s，右侧给关闭按钮让位。
        let title_pad = 10.0 * s;
        let max_w = (tab.width - title_pad * 2.0 - rects.tab_close_buttons[i].width).max(0.0);
        let font_size = 12.0 * s;
        let line_h = font_size * 1.2;
        let ty = tab.y + (tab.height - line_h) / 2.0;
        let title = fit_text(assets, tab_titles.get(i).unwrap_or(&""), font_size, max_w);
        draw_text(
            pixmap,
            assets,
            tab.x + title_pad,
            ty,
            max_w,
            &title,
            font_size,
            TEXT,
        );
        draw_close(pixmap, rects.tab_close_buttons[i], TEXT, (1.2 * s).max(1.0));
    }

    // 新建按钮 +。
    draw_plus(
        pixmap,
        rects.new_tab_button,
        if state.hover == Some(ChromeHit::NewTab) {
            TEXT
        } else {
            ICON
        },
    );

    // 导航按钮：后退/前进禁用态（历史栈未建），刷新可用。
    draw_arrow(pixmap, rects.back_button, -1.0, ICON_DISABLED);
    draw_arrow(pixmap, rects.forward_button, 1.0, ICON_DISABLED);
    draw_reload(
        pixmap,
        rects.reload_button,
        if state.hover == Some(ChromeHit::Reload) {
            TEXT
        } else {
            ICON
        },
    );

    // 地址栏：胶囊底 + 聚焦描边；文本或占位符；聚焦时末尾光标。
    let focus_border_w = 1.5 * s;
    if let Some(path) = rounded_rect_path(rects.address_bar, rects.address_bar.height / 2.0, false)
    {
        let bg = if state.address_focused {
            TAB_ACTIVE_BG
        } else {
            ADDRESS_BG
        };
        fill_path(pixmap, &path, bg);
        if state.address_focused {
            let inner = Rect::new(
                rects.address_bar.x + focus_border_w / 2.0,
                rects.address_bar.y + focus_border_w / 2.0,
                (rects.address_bar.width - focus_border_w).max(0.0),
                (rects.address_bar.height - focus_border_w).max(0.0),
            );
            if let Some(border) = rounded_rect_path(inner, inner.height / 2.0, false) {
                stroke_path(pixmap, &border, ADDRESS_FOCUS_BORDER, focus_border_w);
            }
        }
    }
    let text_pad = 12.0 * s;
    let font_size = 13.0 * s;
    let line_h = font_size * 1.2;
    let ty = rects.address_bar.y + (rects.address_bar.height - line_h) / 2.0;
    let max_w = rects.address_bar.width - text_pad * 2.0;
    if state.address_text.is_empty() && !state.address_focused {
        draw_text(
            pixmap,
            assets,
            rects.address_bar.x + text_pad,
            ty,
            max_w,
            "搜索或输入网址",
            font_size,
            TEXT_PLACEHOLDER,
        );
    } else {
        let text = fit_text(assets, &state.address_text, font_size, max_w);
        let width = draw_text(
            pixmap,
            assets,
            rects.address_bar.x + text_pad,
            ty,
            max_w,
            &text,
            font_size,
            TEXT,
        );
        if state.address_focused {
            // 光标：文本末尾右侧 1.5px 竖线（v1 光标恒在末尾）。
            let caret_w = (1.5 * s).max(1.0);
            fill(
                pixmap,
                Rect::new(
                    rects.address_bar.x + text_pad + width + 2.0 * s,
                    rects.address_bar.y + (rects.address_bar.height - line_h) / 2.0 + line_h * 0.15,
                    caret_w,
                    line_h * 0.7,
                ),
                TEXT,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::model::{layout_chrome, ChromeState};

    fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * pixmap.width() + x) * 4) as usize;
        let d = pixmap.data();
        [d[i], d[i + 1], d[i + 2], d[i + 3]]
    }

    fn has_ink_in(pixmap: &Pixmap, rect: Rect) -> bool {
        let x0 = (rect.x.max(0.0)) as u32;
        let y0 = (rect.y.max(0.0)) as u32;
        let x1 = ((rect.x + rect.width) as u32).min(pixmap.width());
        let y1 = ((rect.y + rect.height) as u32).min(pixmap.height());
        for y in y0..y1 {
            for x in x0..x1 {
                let p = pixel(pixmap, x, y);
                if p[0] < 220 && p[1] < 220 && p[2] < 220 {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn strip_and_toolbar_backgrounds_painted() {
        let rects = layout_chrome(800, 600, 1.0, 1, &ChromeState::default());
        let mut px = Pixmap::new(800, 600).unwrap();
        let mut assets = ChromeAssets::new();
        paint_chrome(
            &mut px,
            &rects,
            &ChromeState::default(),
            &["Demo"],
            0,
            &mut assets,
        );
        // 标签条 #dee1e6（取标签与 + 按钮之间的空白）。
        let x = (rects.new_tab_button.x - 30.0) as u32;
        assert_eq!(pixel(&px, x, 4), [0xde, 0xe1, 0xe6, 255]);
        // 工具栏白底（地址栏右缘之外的边距）。
        assert_eq!(
            pixel(&px, 798, rects.toolbar.y as u32 + 4),
            [255, 255, 255, 255]
        );
        // 工具栏底部 1px 分隔线。
        let sep_y = (rects.toolbar.y + rects.toolbar.height - 0.5) as u32;
        assert_eq!(pixel(&px, 400, sep_y), [0xcf, 0xd4, 0xda, 255]);
    }

    #[test]
    fn active_tab_white_and_title_ink() {
        let rects = layout_chrome(800, 600, 1.0, 2, &ChromeState::default());
        let mut px = Pixmap::new(800, 600).unwrap();
        let mut assets = ChromeAssets::new();
        paint_chrome(
            &mut px,
            &rects,
            &ChromeState::default(),
            &["Demo", "Second"],
            0,
            &mut assets,
        );
        // 活动标签（第 0 个）白底：取标签顶部（文字上方）。
        let t = rects.tabs[0];
        assert_eq!(
            pixel(&px, t.x as u32 + 4, t.y as u32 + 2),
            [255, 255, 255, 255]
        );
        // 非活动标签无底色（露标签条灰）。
        let t1 = rects.tabs[1];
        assert_eq!(
            pixel(&px, t1.x as u32 + 4, t1.y as u32 + 2),
            [0xde, 0xe1, 0xe6, 255]
        );
        // 标题与关闭按钮墨迹。
        assert!(has_ink_in(&px, rects.tabs[0]));
        assert!(has_ink_in(&px, rects.tabs[1]));
        assert!(has_ink_in(&px, rects.tab_close_buttons[0]));
    }

    #[test]
    fn address_bar_placeholder_and_focus_caret() {
        let rects = layout_chrome(800, 600, 1.0, 1, &ChromeState::default());
        let mut px = Pixmap::new(800, 600).unwrap();
        let mut assets = ChromeAssets::new();
        // 未聚焦空文本 → 占位符墨迹 + #f1f3f4 底。
        paint_chrome(
            &mut px,
            &rects,
            &ChromeState::default(),
            &["Demo"],
            0,
            &mut assets,
        );
        let a = rects.address_bar;
        assert_eq!(
            pixel(&px, a.x as u32 + 2, a.y as u32 + a.height as u32 / 2),
            [0xf1, 0xf3, 0xf4, 255]
        );
        assert!(has_ink_in(&px, a));
        // 聚焦 + 输入 → 白底、文本墨迹、光标。
        let state = ChromeState {
            address_focused: true,
            address_text: "hello".into(),
            ..ChromeState::default()
        };
        paint_chrome(&mut px, &rects, &state, &["Demo"], 0, &mut assets);
        assert_eq!(
            pixel(&px, a.x as u32 + 2, a.y as u32 + a.height as u32 / 2),
            [255, 255, 255, 255]
        );
        assert!(has_ink_in(&px, a));
    }

    #[test]
    fn nav_button_icons_have_ink() {
        let rects = layout_chrome(800, 600, 1.0, 1, &ChromeState::default());
        let mut px = Pixmap::new(800, 600).unwrap();
        let mut assets = ChromeAssets::new();
        paint_chrome(
            &mut px,
            &rects,
            &ChromeState::default(),
            &["Demo"],
            0,
            &mut assets,
        );
        assert!(has_ink_in(&px, rects.back_button));
        assert!(has_ink_in(&px, rects.forward_button));
        assert!(has_ink_in(&px, rects.reload_button));
        assert!(has_ink_in(&px, rects.new_tab_button));
    }

    #[test]
    fn fit_text_truncates_with_ellipsis() {
        let mut assets = ChromeAssets::new();
        let long = "MMMMMMMMMMMMMMMMMMMMMMMMMMMMMM";
        let fitted = fit_text(&mut assets, long, 12.0, 60.0);
        assert!(fitted.chars().count() < long.chars().count());
        assert!(fitted.ends_with('\u{2026}'));
        assert_eq!(fit_text(&mut assets, "ok", 12.0, 60.0), "ok");
        assert_eq!(fit_text(&mut assets, "", 12.0, 60.0), "");
    }
}
