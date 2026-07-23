//! §7 Defaulting — initial/inherit/unset 关键字处理。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §7 L1505-1707
//!
//! CC-5 将填充完整实现。

use crate::style::ComputedValue;
use muskitty_css::parser::ComponentValue;

/// §7.3: 应用 defaulting，将 cascaded value 转换为 specified value。
///
/// - `initial` → 属性初始值
/// - `inherit` → 父元素 computed value
/// - `unset` → 继承属性当 `inherit`，非继承属性当 `initial`
///
/// **CC-5 占位**：当前原样返回，待 CC-5 实现。
pub fn apply_defaulting(
    _property: &str,
    cascaded: Option<&[ComponentValue]>,
    _parent_computed: Option<&ComputedValue>,
) -> ComputedValue {
    // CC-5 将实现完整 defaulting 逻辑
    match cascaded {
        Some(cvs) => ComputedValue::Raw(cvs.to_vec()),
        None => ComputedValue::Keyword("initial".to_string()),
    }
}
