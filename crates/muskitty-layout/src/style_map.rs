//! ComputedStyle → taffy::Style 映射。
//!
//! [`map_style`] 将 [`ComputedStyle`] 中的 display / 尺寸 / margin / padding /
//! flexbox 等属性映射为 taffy 的 [`Style`] 结构体，作为布局引擎的输入。
//!
//! # 规范依据
//!
//! - CSS Box Model Level 3 §2-§3: margin/border/padding/content、box-sizing
//! - CSS Flexbox Level 1 §4-§8: flex-direction/flex-wrap/justify/align/gap
//! - CSS Display Level 3 §2: display

use muskitty_cascade::{ComputedStyle, ComputedValue};
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;
use taffy::geometry::Rect;
use taffy::style::{
    AlignItems, BoxSizing, Dimension, Display, FlexDirection, FlexWrap, JustifyContent,
    LengthPercentage, LengthPercentageAuto, Style,
};
use taffy::style_helpers::{TaffyAuto, TaffyZero};

/// 将 [`ComputedStyle`] 映射为 taffy [`Style`]。
///
/// `computed` 为 `None` 时返回默认 Style（display 默认为 `Block`）。
/// 已识别的属性会被覆盖到 Style 对应字段；未识别的属性保持 taffy 默认值。
pub fn map_style(computed: Option<&ComputedStyle>) -> Style {
    // taffy 默认 display 为 Flex（启用 flexbox feature 时），但 HTML 块级元素
    // 的预期默认是 Block，因此显式覆盖为 Block。
    // 同时 taffy 默认 box_sizing 为 BorderBox，与 CSS Box Model §4.1 规定的初始值
    // content-box 不一致，这里纠正为 ContentBox。
    let mut style = Style {
        display: Display::Block,
        box_sizing: BoxSizing::ContentBox,
        ..Style::default()
    };

    let cs = match computed {
        Some(cs) => cs,
        None => return style,
    };

    // —— display ——
    // CSS Display Level 3 §2: display 属性映射。
    //
    // taffy 0.12 的 Display 枚举只有 Block/Flex/Grid/None，无 Inline 变体。
    // 处理策略：
    // - `block` / `flow` / `flow-root` → Block（block container）
    // - `flex` / `inline-flex` → Flex（inline-flex 的 inline-level 行为 taffy 不支持，
    //   仅保留 flex container 语义）
    // - `grid` / `inline-grid` → Grid（同上）
    // - `inline` / `inline-block` → Block（taffy 无 inline layout，作为 workaround）
    // - `none` → None（不生成 box，已在 build_layout_tree 中排除）
    // - `contents` → Block（TODO: §2.5 规定元素本身不生成 box 但保留子元素，
    //   需在 build_layout_tree 中特殊处理，当前先当 Block）
    // - `list-item` → Block（TODO: 需额外处理 marker box，当前当 Block）
    if let Some(kw) = get_keyword(cs, "display") {
        style.display = match kw.to_ascii_lowercase().as_str() {
            "flex" | "inline-flex" => Display::Flex,
            "grid" | "inline-grid" => Display::Grid,
            "block" | "flow" | "flow-root" | "inline" | "inline-block" | "contents"
            | "list-item" => Display::Block,
            "none" => Display::None,
            _ => Display::Block,
        };
    }

    // —— width / height ——
    if let Some(cv) = cs.get("width") {
        style.size.width = map_dimension(cv);
    }
    if let Some(cv) = cs.get("height") {
        style.size.height = map_dimension(cv);
    }

    // —— min-width / max-width / min-height / max-height ——
    if let Some(cv) = cs.get("min-width") {
        style.min_size.width = map_dimension(cv);
    }
    if let Some(cv) = cs.get("max-width") {
        style.max_size.width = map_dimension(cv);
    }
    if let Some(cv) = cs.get("min-height") {
        style.min_size.height = map_dimension(cv);
    }
    if let Some(cv) = cs.get("max-height") {
        style.max_size.height = map_dimension(cv);
    }

    // —— margin ——
    style.margin = Rect {
        top: map_length_percentage_auto(cs.get("margin-top")),
        right: map_length_percentage_auto(cs.get("margin-right")),
        bottom: map_length_percentage_auto(cs.get("margin-bottom")),
        left: map_length_percentage_auto(cs.get("margin-left")),
    };

    // —— padding ——
    style.padding = Rect {
        top: map_length_percentage(cs.get("padding-top")),
        right: map_length_percentage(cs.get("padding-right")),
        bottom: map_length_percentage(cs.get("padding-bottom")),
        left: map_length_percentage(cs.get("padding-left")),
    };

    // —— box-sizing ——
    // CSS Box Model Level 3 §4.1: 初始值为 content-box.
    // 未知值按 §7.1 回退到初始值（ContentBox），而非 taffy 默认的 BorderBox.
    if let Some(kw) = get_keyword(cs, "box-sizing") {
        style.box_sizing = match kw.to_ascii_lowercase().as_str() {
            "content-box" => BoxSizing::ContentBox,
            "border-box" => BoxSizing::BorderBox,
            _ => BoxSizing::ContentBox,
        };
    }

    // —— flex-direction ——
    if let Some(kw) = get_keyword(cs, "flex-direction") {
        style.flex_direction = match kw.to_ascii_lowercase().as_str() {
            "row" => FlexDirection::Row,
            "row-reverse" => FlexDirection::RowReverse,
            "column" => FlexDirection::Column,
            "column-reverse" => FlexDirection::ColumnReverse,
            _ => FlexDirection::Row,
        };
    }

    // —— flex-wrap ——
    if let Some(kw) = get_keyword(cs, "flex-wrap") {
        style.flex_wrap = match kw.to_ascii_lowercase().as_str() {
            "nowrap" => FlexWrap::NoWrap,
            "wrap" => FlexWrap::Wrap,
            "wrap-reverse" => FlexWrap::WrapReverse,
            _ => FlexWrap::NoWrap,
        };
    }

    // —— justify-content ——
    if let Some(kw) = get_keyword(cs, "justify-content") {
        style.justify_content = map_justify_content(&kw.to_ascii_lowercase());
    }

    // —— align-items ——
    if let Some(kw) = get_keyword(cs, "align-items") {
        style.align_items = map_align_items(&kw.to_ascii_lowercase());
    }

    // —— align-self ——
    if let Some(kw) = get_keyword(cs, "align-self") {
        // align-self: auto 表示继承父元素的 align-items，用 None 表示回退。
        if !kw.eq_ignore_ascii_case("auto") {
            style.align_self = map_align_items(&kw.to_ascii_lowercase());
        }
    }

    // —— flex-grow / flex-shrink ——
    // CSS Flexbox §7.2: "Negative values are invalid."
    // 负值被拒绝：
    // - flex-grow 保持初始值 0.0（CSS）= taffy 默认值 0.0
    // - flex-shrink 保持初始值 1.0（CSS）= taffy 默认值 1.0
    if let Some(cv) = cs.get("flex-grow") {
        if let Some(val) = extract_number_from_cv(cv) {
            if val >= 0.0 {
                style.flex_grow = val;
            }
        }
    }
    if let Some(cv) = cs.get("flex-shrink") {
        if let Some(val) = extract_number_from_cv(cv) {
            if val >= 0.0 {
                style.flex_shrink = val;
            }
        }
    }

    // —— flex-basis ——
    if let Some(cv) = cs.get("flex-basis") {
        style.flex_basis = map_dimension(cv);
    }

    // —— gap / row-gap / column-gap ——
    // CSS Box Alignment Level 3 §6.2: gap 简写语法 `gap: <row-gap> <column-gap>?`.
    // - 单值：同时设置 row-gap（height 轴）和 column-gap（width 轴）
    // - 双值：第一个设置 row-gap，第二个设置 column-gap
    // 单独的 row-gap / column-gap 声明覆盖对应轴（在 gap 之后声明时）。
    let mut gap = style.gap;
    if let Some(cv) = cs.get("gap") {
        let (row_gap, col_gap) = extract_gap_pair(cv);
        gap.height = row_gap;
        gap.width = col_gap;
    }
    if let Some(cv) = cs.get("column-gap") {
        gap.width = map_length_percentage(Some(cv));
    }
    if let Some(cv) = cs.get("row-gap") {
        gap.height = map_length_percentage(Some(cv));
    }
    style.gap = gap;

    style
}

// —— 辅助函数 ——

/// 从 ComputedStyle 中读取指定属性的 Keyword 值（小写化由调用方决定）。
///
/// 支持两种来源：
/// - `ComputedValue::Keyword(s)` — defaulting 产生的初始值
/// - `ComputedValue::Resolved/Raw([Ident(s)])` — cascade pipeline 产生的关键字
fn get_keyword(cs: &ComputedStyle, name: &str) -> Option<String> {
    match cs.get(name) {
        Some(ComputedValue::Keyword(kw)) => Some(kw.clone()),
        Some(ComputedValue::Resolved(cvs)) | Some(ComputedValue::Raw(cvs)) => {
            // 从 component values 中提取第一个 Ident token
            for cv in cvs {
                if let ComponentValue::PreservedToken(Token::Ident(s)) = cv {
                    return Some(s.clone());
                }
            }
            None
        }
        _ => None,
    }
}

/// `justify-content` 关键字 → [`JustifyContent`]。
fn map_justify_content(kw: &str) -> Option<JustifyContent> {
    Some(match kw {
        "flex-start" => JustifyContent::FLEX_START,
        "center" => JustifyContent::CENTER,
        "flex-end" => JustifyContent::FLEX_END,
        "space-between" => JustifyContent::SPACE_BETWEEN,
        "space-around" => JustifyContent::SPACE_AROUND,
        "space-evenly" => JustifyContent::SPACE_EVENLY,
        _ => return None,
    })
}

/// `align-items` / `align-self` 关键字 → [`AlignItems`]。
///
/// # 规范依据
///
/// - CSS Flexbox Level 1 §8.3: `align-items` 接受 `normal` 关键字
/// - CSS Box Alignment Level 3 §6.1: `normal` 在 flex 布局中行为等价于 `start`
/// - CSS Flexbox §8.3: `start` 对齐到 cross-axis 起始端，等价于 `flex-start`
///
/// 因此 `normal` 显式映射为 [`AlignItems::FLEX_START`]，避免落入 `_` 分支返回
/// `None` 后被 taffy 默认值 `STRETCH` 覆盖。
fn map_align_items(kw: &str) -> Option<AlignItems> {
    Some(match kw {
        "stretch" => AlignItems::STRETCH,
        "flex-start" | "start" | "normal" => AlignItems::FLEX_START,
        "center" => AlignItems::CENTER,
        "flex-end" | "end" => AlignItems::FLEX_END,
        "baseline" => AlignItems::BASELINE,
        _ => return None,
    })
}

/// 将 [`ComputedValue`] 映射为 [`Dimension`]（用于 width/height/min-max/flex-basis）。
///
/// - `auto` → [`Dimension::AUTO`]
/// - `px` 长度 → [`Dimension::length`]
/// - 百分比 → [`Dimension::percent`]（0.0-1.0 区间）
fn map_dimension(cv: &ComputedValue) -> Dimension {
    match cv {
        ComputedValue::Keyword(kw) if kw.eq_ignore_ascii_case("auto") => Dimension::AUTO,
        ComputedValue::Keyword(kw) if kw.eq_ignore_ascii_case("none") => Dimension::AUTO,
        ComputedValue::Resolved(cvs) | ComputedValue::Raw(cvs) => {
            if let Some(px) = extract_px(cvs) {
                Dimension::length(px)
            } else if let Some(pct) = extract_percent(cvs) {
                Dimension::percent(pct / 100.0)
            } else {
                Dimension::AUTO
            }
        }
        _ => Dimension::AUTO,
    }
}

/// 将 `Option<&ComputedValue>` 映射为 [`LengthPercentageAuto`]（用于 margin）。
///
/// CSS margin 的初始值为 `0`（非 `auto`），因此属性未设置（`None`）或
/// 值无法解析时返回 [`LengthPercentageAuto::ZERO`]。仅 `auto` 关键字
/// 映射为 [`LengthPercentageAuto::AUTO`]。
fn map_length_percentage_auto(cv: Option<&ComputedValue>) -> LengthPercentageAuto {
    match cv {
        Some(ComputedValue::Keyword(kw)) if kw.eq_ignore_ascii_case("auto") => {
            LengthPercentageAuto::AUTO
        }
        Some(ComputedValue::Resolved(cvs)) | Some(ComputedValue::Raw(cvs)) => {
            if let Some(px) = extract_px(cvs) {
                LengthPercentageAuto::length(px)
            } else if let Some(pct) = extract_percent(cvs) {
                LengthPercentageAuto::percent(pct / 100.0)
            } else {
                LengthPercentageAuto::ZERO
            }
        }
        _ => LengthPercentageAuto::ZERO,
    }
}

/// 将 `Option<&ComputedValue>` 映射为 [`LengthPercentage`]（用于 padding/gap）。
fn map_length_percentage(cv: Option<&ComputedValue>) -> LengthPercentage {
    match cv {
        Some(ComputedValue::Resolved(cvs)) | Some(ComputedValue::Raw(cvs)) => {
            if let Some(px) = extract_px(cvs) {
                LengthPercentage::length(px)
            } else if let Some(pct) = extract_percent(cvs) {
                LengthPercentage::percent(pct / 100.0)
            } else {
                LengthPercentage::ZERO
            }
        }
        _ => LengthPercentage::ZERO,
    }
}

/// 从 component value 列表中提取第一个 `px` 长度值。
fn extract_px(cvs: &[ComponentValue]) -> Option<f32> {
    for cv in cvs {
        if let ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) = cv {
            if unit.eq_ignore_ascii_case("px") {
                return Some(numeric.value as f32);
            }
        }
    }
    None
}

/// 从 component value 列表中提取所有长度值（px 或百分比），跳过空白。
///
/// 用于 `gap` 简写双值解析：返回所有非空白的有效长度值。
fn extract_all_lengths(cvs: &[ComponentValue]) -> Vec<f32> {
    let mut result = Vec::new();
    for cv in cvs {
        match cv {
            ComponentValue::PreservedToken(Token::Whitespace) => continue,
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit))
                if unit.eq_ignore_ascii_case("px") =>
            {
                result.push(numeric.value as f32);
            }
            ComponentValue::PreservedToken(Token::Percentage(numeric)) => {
                // gap 百分比在 taffy 中按 LengthPercentage::percent 处理，
                // 这里统一转为 0.0-1.0 区间返回 f32，由调用方决定如何使用。
                // 但当前 gap 不支持百分比，暂不处理（返回空让 fallback 生效）。
                let _ = numeric;
            }
            _ => {}
        }
    }
    result
}

/// 解析 `gap` 简写的 ComputedValue，返回 (row_gap, column_gap)。
///
/// CSS Box Alignment §6.2: `gap: <row-gap> <column-gap>?`
/// - 单值：row_gap == col_gap
/// - 双值：第一个为 row_gap，第二个为 col_gap
fn extract_gap_pair(cv: &ComputedValue) -> (LengthPercentage, LengthPercentage) {
    let cvs = match cv {
        ComputedValue::Resolved(cvs) | ComputedValue::Raw(cvs) => cvs,
        _ => return (LengthPercentage::ZERO, LengthPercentage::ZERO),
    };
    let lengths = extract_all_lengths(cvs);
    match lengths.len() {
        0 => (LengthPercentage::ZERO, LengthPercentage::ZERO),
        1 => {
            let lp = LengthPercentage::length(lengths[0]);
            (lp, lp)
        }
        _ => {
            // 双值或更多：第一个为 row_gap，第二个为 column_gap
            (
                LengthPercentage::length(lengths[0]),
                LengthPercentage::length(lengths[1]),
            )
        }
    }
}

/// 从 component value 列表中提取第一个百分比值（0.0-100.0）。
fn extract_percent(cvs: &[ComponentValue]) -> Option<f32> {
    for cv in cvs {
        if let ComponentValue::PreservedToken(Token::Percentage(numeric)) = cv {
            return Some(numeric.value as f32);
        }
    }
    None
}

/// 从 [`ComputedValue`] 中提取第一个数字值（用于 flex-grow/flex-shrink）。
fn extract_number_from_cv(cv: &ComputedValue) -> Option<f32> {
    let cvs = match cv {
        ComputedValue::Resolved(cvs) | ComputedValue::Raw(cvs) => cvs,
        _ => return None,
    };
    for cv in cvs {
        if let ComponentValue::PreservedToken(Token::Number(numeric)) = cv {
            return Some(numeric.value as f32);
        }
    }
    None
}
