//! §4.4 Computed Value — 相对单位解析、var() 求值。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §4.4 L500-555
//!
//! CC-6 将填充完整实现。

use crate::style::ComputedValue;
use muskitty_css::parser::ComponentValue;

/// §4.4: 将 specified value 转换为 computed value。
///
/// 解析相对单位（em/rem/vh/vw）、var() 替换、百分比等。
///
/// **CC-6 占位**：当前原样返回，待 CC-6 实现。
pub fn compute_value(
    _property: &str,
    specified: &[ComponentValue],
    _parent_font_size: f64,
    _root_font_size: f64,
) -> ComputedValue {
    // CC-6 将实现完整计算逻辑
    ComputedValue::Raw(specified.to_vec())
}
