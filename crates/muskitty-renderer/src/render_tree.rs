//! 渲染样式提取工具。
//!
//! 从 [`ComputedStyle`] 提取绘制所需信息（background-color / 四边 border /
//! outline），供 `paint` 生成 [`RenderCommand`] 时查询。
//!
//! RenderTree / RenderNode 中间结构已移除（P2-17）：`paint` 直接输出
//! `Vec<RenderCommand>`。z-order / 层叠上下文 / transform 嵌套等复杂
//! 场景需要中间结构时再引入，当前无消费者。

use crate::color::Color;
use crate::command::{Border, BorderStyle, SideBorder, TextAlign};
use muskitty_cascade::{ComputedStyle, ComputedValue};
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;

/// 从 ComputedStyle 提取 background-color。
///
/// 未设置或无法解析时返回 `None`（调用方按透明处理）。单态化（P2-20）后
/// 值统一为 token 序列，`parse_color` 同时覆盖命名色/hex/rgb 函数与
/// `transparent`（`parse_named_color` 内含），无需再按来源分支。
pub fn extract_background_color(style: &ComputedStyle) -> Option<Color> {
    let cv = style.get("background-color")?;
    crate::color::parse_color(cv.tokens())
}

/// 从 ComputedStyle 提取文字颜色（`color` 属性）。
///
/// 未设置或无法解析时回退到默认黑色（CSS `color` 初始值 `canvastext`，
/// 当前按黑色近似）。
pub fn extract_text_color(style: &ComputedStyle) -> Color {
    style
        .get("color")
        .and_then(|cv| crate::color::parse_color(cv.tokens()))
        .unwrap_or(Color::BLACK)
}

/// 从 ComputedStyle 提取 font-size 的 px 值。
///
/// cascade 已把 font-size 归一化为 px Dimension（`normalize_font_size`），
/// 此处直接解析 `Token::Dimension(_, "px")`。无法解析时返回 `None`
/// （调用方回退到继承的 font-size 或默认 16px）。
pub fn resolve_font_size(style: &ComputedStyle) -> Option<f32> {
    let cv = style.get("font-size")?;
    for v in cv.tokens() {
        if let ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) = v {
            if unit.eq_ignore_ascii_case("px") {
                return Some(numeric.value as f32);
            }
        }
    }
    None
}

/// 从 ComputedStyle 提取 font-family（取首个字体族名，T-3）。
pub fn resolve_font_family(style: &ComputedStyle) -> Option<String> {
    let cv = style.get("font-family")?;
    cv.tokens().iter().find_map(|t| match t {
        ComponentValue::PreservedToken(Token::Ident(s)) => Some(s.clone()),
        ComponentValue::PreservedToken(Token::String(s)) => Some(s.clone()),
        _ => None,
    })
}

/// 从 ComputedStyle 提取 font-weight（`normal`=400、`bold`=700、数值直接，T-3）。
pub fn resolve_font_weight(style: &ComputedStyle) -> Option<u16> {
    let cv = style.get("font-weight")?;
    for t in cv.tokens() {
        match t {
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                return Some(if s.eq_ignore_ascii_case("bold") {
                    700
                } else {
                    400
                });
            }
            ComponentValue::PreservedToken(Token::Number(n)) => {
                return Some(n.value.clamp(1.0, 1000.0) as u16);
            }
            _ => {}
        }
    }
    None
}

/// 从 ComputedStyle 提取 `line-height` 的使用值（px，M-3 batch 3）。
///
/// 语义委托 cascade `text_props::used_line_height_px`（单一来源）：
/// `normal`/缺失 → 1.2 × font-size；数 → 倍数 × font-size；百分比已在
/// computed value 阶段转 px；非法值（负/NaN）回退 `normal`。
pub fn resolve_line_height(style: &ComputedStyle, font_size: f32) -> f32 {
    muskitty_cascade::used_line_height_px(style, font_size)
}

/// 按 `text-transform` 改写文本（M-3 batch 3）。
///
/// 委托 cascade `apply_text_transform`：与 layout 测量使用**同一**实现，
/// 保证绘制内容与测量内容一致（CSS Text L3 §2.1 的转换在布局前生效）。
pub fn apply_text_transform<'a>(text: &'a str, keyword: Option<&str>) -> std::borrow::Cow<'a, str> {
    muskitty_cascade::apply_text_transform(text, keyword)
}

/// 从 ComputedStyle 提取 text-align 水平对齐（T-3）。///
/// `center` → Center，`right`/`end` → Right，其余（`left`/`start`/`justify`/未知）→ Left。
pub fn resolve_text_align(style: &ComputedStyle) -> TextAlign {
    style
        .get("text-align")
        .and_then(|cv| cv.keyword())
        .map(|k| match k.to_ascii_lowercase().as_str() {
            "center" => TextAlign::Center,
            "right" | "end" => TextAlign::Right,
            _ => TextAlign::Left,
        })
        .unwrap_or(TextAlign::Left)
}

/// 从 ComputedStyle 提取边框（四边独立）。
///
/// 逐边读取 `border-<side>-{style,width,color}`（M-3 batch 2：cascade 的
/// `border`/`border-<side>`/`border-width|style|color` 简写已全部展开为
/// 方向性长属性，不再有统一的 `border-width` 等中间属性）。
///
/// - `border-<side>-style` 为 `none`/`hidden` 或缺失（初始值 none）→ 该边
///   跳过（§4.1：used width = 0）；
/// - 宽度取 px Dimension（cascade 已把 `thin`/`medium`/`thick` 归一化为 px）；
/// - 颜色为 `currentcolor` 时用调用方传入的 `current_color`（元素文字色）。
///
/// 四边均无 → `None`。
pub fn extract_border(style: &ComputedStyle, current_color: Color) -> Option<Border> {
    let border = Border {
        top: extract_side(
            style,
            "border-top-style",
            "border-top-width",
            "border-top-color",
            current_color,
        ),
        right: extract_side(
            style,
            "border-right-style",
            "border-right-width",
            "border-right-color",
            current_color,
        ),
        bottom: extract_side(
            style,
            "border-bottom-style",
            "border-bottom-width",
            "border-bottom-color",
            current_color,
        ),
        left: extract_side(
            style,
            "border-left-style",
            "border-left-width",
            "border-left-color",
            current_color,
        ),
    };
    if border.is_empty() {
        None
    } else {
        Some(border)
    }
}

/// 提取单边边框；该边不绘制（style none/hidden、宽度 ≤ 0 或缺失）时 `None`。
fn extract_side(
    style: &ComputedStyle,
    style_prop: &str,
    width_prop: &str,
    color_prop: &str,
    current_color: Color,
) -> Option<SideBorder> {
    let border_style = parse_border_style(style.get(style_prop)?.keyword()?);
    if !border_style.is_painted() {
        return None;
    }
    let width = parse_border_width(style.get(width_prop)?)?;
    if width <= 0.0 {
        return None;
    }
    let color = style
        .get(color_prop)
        .and_then(|cv| resolve_color(cv, current_color))
        .unwrap_or(current_color);
    Some(SideBorder {
        width,
        color,
        style: border_style,
    })
}

/// 从 ComputedStyle 提取轮廓（CSS UI Level 4 §4）。
///
/// 轮廓不参与布局，绘制在 border box 之外（由 backend 展开到盒子外侧）。
/// `outline-style` 为 `none`（初始值）或宽度 ≤ 0 → `None`。
/// `outline-style: auto`（UA 焦点环）按 solid 近似；`outline-color` 的
/// 初始值 `auto` 与 `currentcolor` 一样解析为元素文字色。
pub fn extract_outline(style: &ComputedStyle, current_color: Color) -> Option<SideBorder> {
    let kw = style.get("outline-style")?.keyword()?;
    // `auto` 是 outline-style 独有关键字（UA 焦点环），border-style 无此值
    let border_style = if kw.eq_ignore_ascii_case("auto") {
        BorderStyle::Solid
    } else {
        parse_border_style(kw)
    };
    if !border_style.is_painted() {
        return None;
    }
    let width = parse_border_width(style.get("outline-width")?)?;
    if width <= 0.0 {
        return None;
    }
    let color = style
        .get("outline-color")
        .and_then(|cv| resolve_color(cv, current_color))
        .unwrap_or(current_color);
    Some(SideBorder {
        width,
        color,
        style: border_style,
    })
}

/// 解析 border/outline 颜色值；`currentcolor` → `current_color`。
///
/// 无法解析（如 `outline-color: auto`）返回 `None`，调用方回退
/// `current_color`。
fn resolve_color(cv: &ComputedValue, current_color: Color) -> Option<Color> {
    if cv
        .keyword()
        .is_some_and(|kw| kw.eq_ignore_ascii_case("currentcolor"))
    {
        return Some(current_color);
    }
    crate::color::parse_color(cv.tokens())
}

/// 解析 border-style / outline-style 关键字（CSS Backgrounds & Borders L3 §4.2 全集）。
///
/// 未知关键字 → [`BorderStyle::None`]（不绘制），与 CSS 无效值回退初始值的
/// 效果一致。
fn parse_border_style(kw: &str) -> BorderStyle {
    match kw.to_ascii_lowercase().as_str() {
        "none" => BorderStyle::None,
        "hidden" => BorderStyle::Hidden,
        "solid" => BorderStyle::Solid,
        "dashed" => BorderStyle::Dashed,
        "dotted" => BorderStyle::Dotted,
        "double" => BorderStyle::Double,
        "groove" => BorderStyle::Groove,
        "ridge" => BorderStyle::Ridge,
        "inset" => BorderStyle::Inset,
        "outset" => BorderStyle::Outset,
        _ => BorderStyle::None,
    }
}

/// 解析边框宽度为 px 浮点值。
///
/// cascade 的 computed value 阶段已把 `thin`/`medium`/`thick` 归一化为 px
/// Dimension（`normalize_line_width`），故此处只需认 px Dimension 与
/// `<length>` 的裸 `0`。其他单位/无法解析的值 → `None`（不绘制）。
fn parse_border_width(cv: &ComputedValue) -> Option<f32> {
    for v in cv.tokens() {
        match v {
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit))
                if unit.eq_ignore_ascii_case("px") =>
            {
                return Some(numeric.value as f32);
            }
            ComponentValue::PreservedToken(Token::Number(numeric)) if numeric.value == 0.0 => {
                return Some(0.0);
            }
            _ => {}
        }
    }
    None
}
