//! ComputedStyle → taffy::Style 映射测试。
//!
//! 覆盖：display / 尺寸 / margin / padding / flexbox 属性 / box-sizing / gap。

use muskitty_cascade::{ComputedStyle, ComputedValue};
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::{Numeric, Token};
use muskitty_layout::style_map::map_style;
use taffy::prelude::*;
use taffy::style::{BoxSizing, Dimension, Display, FlexDirection, FlexWrap, LengthPercentage};

/// 构造 `Npx` 的 Resolved ComputedValue。
fn px(val: f64) -> ComputedValue {
    ComputedValue::Resolved(vec![ComponentValue::PreservedToken(Token::Dimension(
        Numeric {
            value: val,
            is_integer: false,
        },
        "px".to_string(),
    ))])
}

/// 构造 `N%` 的 Resolved ComputedValue。
fn pct(val: f64) -> ComputedValue {
    ComputedValue::Resolved(vec![ComponentValue::PreservedToken(Token::Percentage(
        Numeric {
            value: val,
            is_integer: false,
        },
    ))])
}

/// 构造纯数字的 Resolved ComputedValue（用于 flex-grow/flex-shrink）。
fn num(val: f64) -> ComputedValue {
    ComputedValue::Resolved(vec![ComponentValue::PreservedToken(Token::Number(
        Numeric {
            value: val,
            is_integer: false,
        },
    ))])
}

fn kw(s: &str) -> ComputedValue {
    ComputedValue::Keyword(s.to_string())
}

// —— display ——

#[test]
fn display_block_maps_to_block() {
    let mut cs = ComputedStyle::new();
    cs.set("display", kw("block"));
    let style = map_style(Some(&cs));
    assert_eq!(style.display, Display::Block);
}

#[test]
fn display_flex_maps_to_flex() {
    let mut cs = ComputedStyle::new();
    cs.set("display", kw("flex"));
    let style = map_style(Some(&cs));
    assert_eq!(style.display, Display::Flex);
}

#[test]
fn display_grid_maps_to_grid() {
    let mut cs = ComputedStyle::new();
    cs.set("display", kw("grid"));
    let style = map_style(Some(&cs));
    assert_eq!(style.display, Display::Grid);
}

#[test]
fn display_none_maps_to_none() {
    let mut cs = ComputedStyle::new();
    cs.set("display", kw("none"));
    let style = map_style(Some(&cs));
    assert_eq!(style.display, Display::None);
}

#[test]
fn no_display_defaults_to_block() {
    let cs = ComputedStyle::new();
    let style = map_style(Some(&cs));
    assert_eq!(style.display, Display::Block);
}

#[test]
fn no_computed_style_defaults_to_block() {
    let style = map_style(None);
    assert_eq!(style.display, Display::Block);
}

// —— width / height ——

#[test]
fn width_px_maps_to_length() {
    let mut cs = ComputedStyle::new();
    cs.set("width", px(200.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.size.width, Dimension::length(200.0));
}

#[test]
fn width_auto_maps_to_auto() {
    let mut cs = ComputedStyle::new();
    cs.set("width", kw("auto"));
    let style = map_style(Some(&cs));
    assert!(
        style.size.width.is_auto(),
        "width: auto 应映射为 Dimension::AUTO"
    );
}

#[test]
fn width_percentage_maps_to_percent() {
    let mut cs = ComputedStyle::new();
    cs.set("width", pct(50.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.size.width, Dimension::percent(0.5));
}

#[test]
fn height_px_maps_to_length() {
    let mut cs = ComputedStyle::new();
    cs.set("height", px(100.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.size.height, Dimension::length(100.0));
}

#[test]
fn min_max_width_mapped() {
    let mut cs = ComputedStyle::new();
    cs.set("min-width", px(50.0));
    cs.set("max-width", px(300.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.min_size.width, Dimension::length(50.0));
    assert_eq!(style.max_size.width, Dimension::length(300.0));
}

// —— margin ——

#[test]
fn margin_px_maps_correctly() {
    let mut cs = ComputedStyle::new();
    cs.set("margin-top", px(10.0));
    cs.set("margin-right", px(20.0));
    cs.set("margin-bottom", px(30.0));
    cs.set("margin-left", px(40.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.margin.top, LengthPercentageAuto::length(10.0));
    assert_eq!(style.margin.right, LengthPercentageAuto::length(20.0));
    assert_eq!(style.margin.bottom, LengthPercentageAuto::length(30.0));
    assert_eq!(style.margin.left, LengthPercentageAuto::length(40.0));
}

#[test]
fn margin_auto_maps_correctly() {
    let mut cs = ComputedStyle::new();
    cs.set("margin-left", kw("auto"));
    let style = map_style(Some(&cs));
    assert!(
        style.margin.left.is_auto(),
        "margin-left: auto 应映射为 AUTO"
    );
}

#[test]
fn margin_percentage_maps_correctly() {
    let mut cs = ComputedStyle::new();
    cs.set("margin-top", pct(10.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.margin.top, LengthPercentageAuto::percent(0.1));
}

// —— padding ——

#[test]
fn padding_px_maps_correctly() {
    let mut cs = ComputedStyle::new();
    cs.set("padding-top", px(5.0));
    cs.set("padding-left", px(15.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.padding.top, LengthPercentage::length(5.0));
    assert_eq!(style.padding.left, LengthPercentage::length(15.0));
}

#[test]
fn padding_defaults_to_zero() {
    let cs = ComputedStyle::new();
    let style = map_style(Some(&cs));
    assert_eq!(style.padding.top, LengthPercentage::ZERO);
}

// —— box-sizing ——

#[test]
fn box_sizing_content_box() {
    let mut cs = ComputedStyle::new();
    cs.set("box-sizing", kw("content-box"));
    let style = map_style(Some(&cs));
    assert_eq!(style.box_sizing, BoxSizing::ContentBox);
}

#[test]
fn box_sizing_border_box() {
    let mut cs = ComputedStyle::new();
    cs.set("box-sizing", kw("border-box"));
    let style = map_style(Some(&cs));
    assert_eq!(style.box_sizing, BoxSizing::BorderBox);
}

// —— flexbox 属性 ——

#[test]
fn flex_direction_row() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-direction", kw("row"));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_direction, FlexDirection::Row);
}

#[test]
fn flex_direction_column() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-direction", kw("column"));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_direction, FlexDirection::Column);
}

#[test]
fn flex_direction_row_reverse() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-direction", kw("row-reverse"));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_direction, FlexDirection::RowReverse);
}

#[test]
fn flex_wrap_wrap() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-wrap", kw("wrap"));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_wrap, FlexWrap::Wrap);
}

#[test]
fn flex_wrap_nowrap() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-wrap", kw("nowrap"));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_wrap, FlexWrap::NoWrap);
}

#[test]
fn justify_content_center() {
    let mut cs = ComputedStyle::new();
    cs.set("justify-content", kw("center"));
    let style = map_style(Some(&cs));
    assert_eq!(style.justify_content, Some(JustifyContent::CENTER));
}

#[test]
fn justify_content_space_between() {
    let mut cs = ComputedStyle::new();
    cs.set("justify-content", kw("space-between"));
    let style = map_style(Some(&cs));
    assert_eq!(style.justify_content, Some(JustifyContent::SPACE_BETWEEN));
}

#[test]
fn align_items_center() {
    let mut cs = ComputedStyle::new();
    cs.set("align-items", kw("center"));
    let style = map_style(Some(&cs));
    assert_eq!(style.align_items, Some(AlignItems::CENTER));
}

#[test]
fn align_items_stretch() {
    let mut cs = ComputedStyle::new();
    cs.set("align-items", kw("stretch"));
    let style = map_style(Some(&cs));
    assert_eq!(style.align_items, Some(AlignItems::STRETCH));
}

#[test]
fn align_self_auto_falls_back_to_none() {
    let mut cs = ComputedStyle::new();
    cs.set("align-self", kw("auto"));
    let style = map_style(Some(&cs));
    assert_eq!(
        style.align_self, None,
        "align-self: auto 应回退到 None（继承父 align-items）"
    );
}

#[test]
fn align_self_center() {
    let mut cs = ComputedStyle::new();
    cs.set("align-self", kw("center"));
    let style = map_style(Some(&cs));
    assert_eq!(style.align_self, Some(AlignSelf::CENTER));
}

// —— flex-grow / flex-shrink / flex-basis ——

#[test]
fn flex_grow_number() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-grow", num(2.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_grow, 2.0);
}

#[test]
fn flex_shrink_number() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-shrink", num(0.5));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_shrink, 0.5);
}

#[test]
fn flex_basis_px() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-basis", px(150.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.flex_basis, Dimension::length(150.0));
}

#[test]
fn flex_basis_auto() {
    let mut cs = ComputedStyle::new();
    cs.set("flex-basis", kw("auto"));
    let style = map_style(Some(&cs));
    assert!(
        style.flex_basis.is_auto(),
        "flex-basis: auto 应映射为 Dimension::AUTO"
    );
}

// —— gap ——

#[test]
fn gap_px_sets_both_axes() {
    let mut cs = ComputedStyle::new();
    cs.set("gap", px(10.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.gap.width, LengthPercentage::length(10.0));
    assert_eq!(style.gap.height, LengthPercentage::length(10.0));
}

#[test]
fn row_gap_and_column_gap_independent() {
    let mut cs = ComputedStyle::new();
    cs.set("row-gap", px(5.0));
    cs.set("column-gap", px(15.0));
    let style = map_style(Some(&cs));
    assert_eq!(style.gap.height, LengthPercentage::length(5.0));
    assert_eq!(style.gap.width, LengthPercentage::length(15.0));
}
