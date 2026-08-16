//! 渲染样式提取工具。
//!
//! 从 [`ComputedStyle`] 提取绘制所需信息（background-color / border），
//! 供 `paint` 生成 [`RenderCommand`] 时查询。
//!
//! RenderTree / RenderNode 中间结构已移除（P2-17）：`paint` 直接输出
//! `Vec<RenderCommand>`。z-order / 层叠上下文 / transform 嵌套等复杂
//! 场景需要中间结构时再引入，当前无消费者。

use crate::color::Color;
use crate::command::{Border, BorderStyle};
use muskitty_cascade::ComputedStyle;
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

/// 从 ComputedStyle 提取边框。
///
/// 读取 `border-width` / `border-style` / `border-color` 三个 longhand
/// 属性。`border` 简写需要 CSSOM 层展开，当前不支持（推迟）。
///
/// 返回 `None` 表示无边框（未设置 / 样式为 none / 宽度为 0）。
pub fn extract_border(style: &ComputedStyle) -> Option<Border> {
    // border-style（默认 none）
    let style_val = parse_border_style(style)?;
    if style_val == BorderStyle::None {
        return None;
    }

    // border-width（默认 0，当前仅解析 px）
    let width = parse_border_width(style)?;
    if width <= 0.0 {
        return None;
    }

    // border-color（默认 currentColor，当前回退到黑色）
    let color = parse_border_color(style).unwrap_or(Color::BLACK);

    Some(Border {
        width,
        color,
        style: style_val,
    })
}

/// 解析 `border-style` 关键字。
fn parse_border_style(style: &ComputedStyle) -> Option<BorderStyle> {
    let cv = style.get("border-style")?;
    let kw = cv.keyword()?;
    match kw.to_ascii_lowercase().as_str() {
        "none" => Some(BorderStyle::None),
        "solid" => Some(BorderStyle::Solid),
        "dashed" => Some(BorderStyle::Dashed),
        "dotted" => Some(BorderStyle::Dotted),
        _ => None,
    }
}

/// 解析 `border-width` 为 px 浮点值。
///
/// 当前仅支持 `<length>` 的 px 单位；其他单位（em/rem/pt）推迟。
fn parse_border_width(style: &ComputedStyle) -> Option<f32> {
    let cv = style.get("border-width")?;
    // 取首个 dimension token
    for v in cv.tokens() {
        if let ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) = v {
            if unit.eq_ignore_ascii_case("px") {
                return Some(numeric.value as f32);
            }
            // 非 px 单位推迟
        }
    }
    None
}

/// 解析 `border-color`。
fn parse_border_color(style: &ComputedStyle) -> Option<Color> {
    let cv = style.get("border-color")?;
    crate::color::parse_color(cv.tokens())
}
