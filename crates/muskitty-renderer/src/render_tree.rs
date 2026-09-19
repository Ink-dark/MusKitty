//! 渲染样式提取工具。
//!
//! 从 [`ComputedStyle`] 提取绘制所需信息（background-color / 四边 border /
//! outline），供 `paint` 生成 [`RenderCommand`] 时查询。
//!
//! RenderTree / RenderNode 中间结构已移除（P2-17）：`paint` 直接输出
//! `Vec<RenderCommand>`。z-order / 层叠上下文 / transform 嵌套等复杂
//! 场景需要中间结构时再引入，当前无消费者。

use crate::color::Color;
use crate::command::{
    BackgroundPosition, BackgroundSize, Border, BorderRadius, BorderStyle, LengthOrPercent, Radius,
    RepeatStyle, SideBorder, TextAlign,
};
use muskitty_cascade::{ComputedStyle, ComputedValue};
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;

/// 从 ComputedStyle 解析 CSS `opacity`（M-3 batch 4，CSS Color L3 §5.1）。
///
/// 读取 `opacity` 的 Number token 并 clamp 到 `0..=1`（§5.1 的数值范围）。
/// 缺失 / 非法值 / 不可解析 → 返回 `1.0`（不透明，等效无操作）。
pub fn resolve_opacity(style: &ComputedStyle) -> f32 {
    let Some(cv) = style.get("opacity") else {
        return 1.0;
    };
    for v in cv.tokens() {
        if let ComponentValue::PreservedToken(Token::Number(n)) = v {
            // `n.value` 是 f64（CSS number token）；`opacity` 值域
            // clamp 到 [0,1] 后转 f32。
            return n.value.clamp(0.0, 1.0) as f32;
        }
    }
    1.0
}

/// 该元素自身 `visibility` 是否 `hidden`（CSS Visibility L3 §1）。
///
/// 缺失 / 非 hidden（`visible`/`collapse` 之外）→ `false`。继承语义由调用方
/// （paint）用参数传递：读取本元素 style 的关键字，若没有属性键则沿用
/// 继承值（ISO 上 cascade 已把 `visibility`（inherited）填充进每个元素的
/// computed style，故常见情形此处直接反映继承结果）。
pub fn is_visibility_hidden(style: &ComputedStyle) -> bool {
    style
        .get("visibility")
        .and_then(|cv| cv.keyword())
        .map(|k| k.eq_ignore_ascii_case("hidden"))
        .unwrap_or(false)
}

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

/// 从 ComputedStyle 提取 `background-image` 的 URL（BG-1，M-3 batch 5 前置）。
///
/// 支持子集（CSS Backgrounds L3 §3.1）：`url()` 的无引号形式（tokenizer 的
/// `Token::Url`）与带引号形式（`url("...")` 函数）。返回**原样** URL 字符串
/// （相对 URL 由调用方——chrome 侧——按文档 base 解析；renderer 不知道
/// base URL）。`none` / 缺失 / 渐变函数（暂不支持绘制）/ 其他值 → `None`
/// （跳过背景图，非致命）。
pub fn extract_background_image_url(style: &ComputedStyle) -> Option<String> {
    let cv = style.get("background-image")?;
    for t in cv.tokens() {
        match t {
            // 无引号形式：tokenizer §4.3.8 直接产出 Url token。
            ComponentValue::PreservedToken(Token::Url(u)) => return Some(u.clone()),
            // 带引号形式：`url(...)` 函数，参数为单个 String token。
            ComponentValue::Function(f) if f.name.eq_ignore_ascii_case("url") => {
                for inner in &f.value {
                    if let ComponentValue::PreservedToken(Token::String(s)) = inner {
                        return Some(s.clone());
                    }
                }
                return None;
            }
            // `none` 关键字与渐变函数等 → 无可绘制 URL。
            _ => {}
        }
    }
    None
}

/// 从 ComputedStyle 提取 `background-repeat` 平铺样式（BG-1 收尾）。
///
/// 支持子集（Backgrounds L3 §3.2）：`repeat`/`repeat-x`/`repeat-y`/
/// `no-repeat`。未知关键字 / 缺失 / 不可解析 → 回退 [`RepeatStyle::Repeat`]
/// （初始值），不致命。
pub fn extract_background_repeat(style: &ComputedStyle) -> RepeatStyle {
    let Some(cv) = style.get("background-repeat") else {
        return RepeatStyle::Repeat;
    };
    for t in cv.tokens() {
        if let ComponentValue::PreservedToken(Token::Ident(s)) = t {
            return match s.to_ascii_lowercase().as_str() {
                "repeat-x" => RepeatStyle::RepeatX,
                "repeat-y" => RepeatStyle::RepeatY,
                "no-repeat" => RepeatStyle::NoRepeat,
                // "repeat" 及任何未知 → Repeat（初始值）。
                _ => RepeatStyle::Repeat,
            };
        }
    }
    RepeatStyle::Repeat
}

/// 从 ComputedStyle 提取 `background-position` 起点偏移（BG-1 收尾）。
///
/// 支持子集（Backgrounds L3 §3.6）：`left`/`right`/`center`/`top`/
/// `bottom` 关键字或 `<length-percentage>`，至多两个分量（x y）。未提供 /
/// 无法解析 → 回退 `0% 0%`（初始值）。
pub fn extract_background_position(style: &ComputedStyle) -> BackgroundPosition {
    let default = BackgroundPosition::default();
    let Some(cv) = style.get("background-position") else {
        return default;
    };
    // 收集 position 数值分量（跳过非数值 token，如 whitespace）。
    let vals: Vec<&Token> = cv
        .tokens()
        .iter()
        .filter_map(|t| match t {
            ComponentValue::PreservedToken(tok) => Some(tok),
            _ => None,
        })
        .filter(|tok| {
            matches!(
                tok,
                Token::Ident(_) | Token::Dimension(..) | Token::Percentage(..)
            )
        })
        .take(2)
        .collect();
    match vals.as_slice() {
        [] => default,
        [single] => single_value_position(single),
        [x, y] => {
            // 双值：前者水平，后者垂直；任一轴归属不符 → 回退默认。
            let (Some(px), Some(py)) = (bg_axis_value(x), bg_axis_value(y)) else {
                return default;
            };
            BackgroundPosition { x: px, y: py }
        }
        _ => default,
    }
}

/// 单值 position：垂直关键字（top/bottom）作 y（x=center），其余作 x（y=center）。
fn single_value_position(tok: &Token) -> BackgroundPosition {
    if let Token::Ident(s) = tok {
        match s.to_ascii_lowercase().as_str() {
            "top" => {
                return BackgroundPosition {
                    x: LengthOrPercent::Percent(50.0),
                    y: LengthOrPercent::Percent(0.0),
                };
            }
            "bottom" => {
                return BackgroundPosition {
                    x: LengthOrPercent::Percent(50.0),
                    y: LengthOrPercent::Percent(100.0),
                };
            }
            _ => {}
        }
    }
    // 其余（left/right/center/长度/百分比）作水平值，垂直取 center。
    match bg_axis_value(tok) {
        Some(x) => BackgroundPosition {
            x,
            y: LengthOrPercent::Percent(50.0),
        },
        // 无法归属（如 registry 初始值整串 "0% 0%" 被合成为一个 Ident）→
        // 回退初始值 0% 0%（Backgrounds L3 §3.6）。
        None => BackgroundPosition::default(),
    }
}

/// 把单个 position 分量 token 解析为水平/垂直偏移；无法归属返回 `None`。
fn bg_axis_value(tok: &Token) -> Option<LengthOrPercent> {
    match tok {
        Token::Ident(s) => match s.to_ascii_lowercase().as_str() {
            "left" | "top" => Some(LengthOrPercent::Percent(0.0)),
            "right" | "bottom" => Some(LengthOrPercent::Percent(100.0)),
            "center" => Some(LengthOrPercent::Percent(50.0)),
            _ => None,
        },
        Token::Dimension(n, u) if u.eq_ignore_ascii_case("px") => {
            Some(LengthOrPercent::Px(n.value as f32))
        }
        Token::Percentage(p) => Some(LengthOrPercent::Percent(p.value as f32)),
        _ => None,
    }
}

/// 从 ComputedStyle 提取 `background-size` 尺寸（BG-1 收尾）。
///
/// 支持子集（Backgrounds L3 §3.9）：`auto` / `cover` / `contain` 或
/// `<length-percentage>{1,2}`（第二值 `auto` = [`BackgroundSize::Length`]
/// 的 `height: None`）。未知 / 无法解析 → 回退 [`BackgroundSize::Auto`]。
pub fn extract_background_size(style: &ComputedStyle) -> BackgroundSize {
    let Some(cv) = style.get("background-size") else {
        return BackgroundSize::Auto;
    };
    let vals: Vec<&Token> = cv
        .tokens()
        .iter()
        .filter_map(|t| match t {
            ComponentValue::PreservedToken(tok) => Some(tok),
            _ => None,
        })
        .filter(|tok| {
            matches!(
                tok,
                Token::Ident(..) | Token::Dimension(..) | Token::Percentage(..)
            )
        })
        .take(2)
        .collect();
    match vals.as_slice() {
        [] => BackgroundSize::Auto,
        [t] => match t {
            Token::Ident(s) if s.eq_ignore_ascii_case("cover") => BackgroundSize::Cover,
            Token::Ident(s) if s.eq_ignore_ascii_case("contain") => BackgroundSize::Contain,
            // "auto" 及任何长度/百分比 → Length（高度 auto）。
            Token::Ident(s) if s.eq_ignore_ascii_case("auto") => BackgroundSize::Auto,
            // 单个长度/百分比 → width 固定、height auto。
            _ => bg_size_len(t)
                .map(|width| BackgroundSize::Length {
                    width,
                    height: None,
                })
                .unwrap_or(BackgroundSize::Auto),
        },
        [w, h] => {
            // 双值：width + height；任一为 auto → 该轴按纵横比推导。
            match (bg_size_len(w), bg_size_len(h)) {
                // 两 auto → Auto（自然尺寸）。
                (None, None) => BackgroundSize::Auto,
                (Some(width), height) => BackgroundSize::Length { width, height },
                // width=auto 组合（height 给定了值）不在支持子集内 → 回退 Auto。
                (None, Some(_)) => BackgroundSize::Auto,
            }
        }
        _ => BackgroundSize::Auto,
    }
}

/// 把单个 size 分量解析为 [`LengthOrPercent`]；`auto` 或不可解析返回 `None`。
fn bg_size_len(tok: &Token) -> Option<LengthOrPercent> {
    match tok {
        Token::Dimension(n, u) if u.eq_ignore_ascii_case("px") => {
            Some(LengthOrPercent::Px(n.value as f32))
        }
        Token::Percentage(p) => Some(LengthOrPercent::Percent(p.value as f32)),
        _ => None,
    }
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

/// 从 ComputedStyle 提取四角圆角（M-3 batch 5，Backgrounds L3 §5.1）。
///
/// 四个 `border-<corner>-radius` 长属性由 cascade 的 `border-radius` 简写
/// 展开或长属性直接声明。每个属性值是 x 半径（及可选的 y 半径，来自 `/`
/// 简写）的 token 序列：
/// - px `Dimension` → 直接用；
/// - `Percentage` → 首值（x）按盒**宽**折算、次值（y）按盒**高**折算（`width`
///   /`height` 参数即调用方传入的盒子尺寸）；
/// - 裸 `0` → 0；
/// - 缺失 → 0，且 y 缺省时 = x（圆角）。
///
/// 只认 px 与百分比；其他单位（em 等）本次忽略（该分量视为缺失），并把半径
/// 钳制在盒半宽/半高内（§5.1 的 corner 重叠时收敛为半圆）。
pub fn extract_border_radius(style: &ComputedStyle, width: f32, height: f32) -> BorderRadius {
    let corner = |prop: &str, box_w: f32, box_h: f32| {
        let mut vals: Vec<f32> = Vec::with_capacity(2);
        if let Some(cv) = style.get(prop) {
            for t in cv.tokens() {
                if vals.len() >= 2 {
                    break;
                }
                match t {
                    ComponentValue::PreservedToken(Token::Dimension(n, unit))
                        if unit.eq_ignore_ascii_case("px") =>
                    {
                        vals.push(n.value as f32);
                    }
                    ComponentValue::PreservedToken(Token::Percentage(p)) => {
                        // 首值（x）相对宽，次值（y）相对高。
                        let basis = if vals.is_empty() { box_w } else { box_h };
                        vals.push(basis * (p.value as f32) / 100.0);
                    }
                    ComponentValue::PreservedToken(Token::Number(n)) if n.value == 0.0 => {
                        vals.push(0.0);
                    }
                    _ => {}
                }
            }
        }
        let x = vals.first().copied().unwrap_or(0.0);
        let y = vals.get(1).copied().unwrap_or(x);
        Radius {
            x: x.clamp(0.0, box_w / 2.0),
            y: y.clamp(0.0, box_h / 2.0),
        }
    };
    BorderRadius {
        top_left: corner("border-top-left-radius", width, height),
        top_right: corner("border-top-right-radius", width, height),
        bottom_right: corner("border-bottom-right-radius", width, height),
        bottom_left: corner("border-bottom-left-radius", width, height),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use muskitty_css::tokenizer::Numeric;

    /// 构造一个含单个 `opacity` Number token 的 computed value。
    fn opacity_style(op: f64) -> ComputedStyle {
        let mut s = ComputedStyle::new();
        s.set(
            "opacity",
            ComputedValue::from_tokens(vec![ComponentValue::PreservedToken(Token::Number(
                Numeric::new(op, false),
            ))]),
        );
        s
    }

    #[test]
    fn resolve_opacity_parses_number_and_clamps() {
        // 0.5 → 0.5（值域内直接透传）。
        assert_eq!(resolve_opacity(&opacity_style(0.5)), 0.5);
        // 0 → 0（整棵子树不可见）。
        assert_eq!(resolve_opacity(&opacity_style(0.0)), 0.0);
        // 1 → 1（不透明，等效无操作）。
        assert_eq!(resolve_opacity(&opacity_style(1.0)), 1.0);
        // 超范围 clamp 到 [0,1]。
        assert_eq!(resolve_opacity(&opacity_style(1.7)), 1.0);
        assert_eq!(resolve_opacity(&opacity_style(-0.3)), 0.0);
    }

    #[test]
    fn resolve_opacity_missing_or_illegal_falls_back_to_1() {
        // 缺失 `opacity` 键 → 1.0。
        assert_eq!(resolve_opacity(&ComputedStyle::new()), 1.0);
        // 非法值（非 Number token，如关键字）→ 1.0。
        let mut s = ComputedStyle::new();
        s.set("opacity", ComputedValue::from_keyword("hidden"));
        assert_eq!(resolve_opacity(&s), 1.0);
    }

    #[test]
    fn is_visibility_hidden_matches_only_hidden_keyword() {
        let hidden = {
            let mut s = ComputedStyle::new();
            s.set("visibility", ComputedValue::from_keyword("hidden"));
            s
        };
        assert!(is_visibility_hidden(&hidden), "hidden → true");

        let visible = {
            let mut s = ComputedStyle::new();
            s.set("visibility", ComputedValue::from_keyword("visible"));
            s
        };
        assert!(!is_visibility_hidden(&visible), "visible → false");

        let collapse = {
            let mut s = ComputedStyle::new();
            s.set("visibility", ComputedValue::from_keyword("collapse"));
            s
        };
        assert!(!is_visibility_hidden(&collapse), "collapse → false");

        // 缺失键 → false（继承语义由 paint 侧参数传递处理）。
        assert!(!is_visibility_hidden(&ComputedStyle::new()));
    }
}
