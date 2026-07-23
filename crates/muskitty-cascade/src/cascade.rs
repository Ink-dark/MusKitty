//! §6.1 Cascade 排序算法。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §6.1 L855-994
//!
//! 按 7 准则降序排序 declared values：
//! 1. Origin and Importance
//! 4. Element-Attached Styles (`style` attr)
//! 6. Specificity
//! 7. Order of Appearance
//!
//! CC-4 将填充完整实现。

use crate::style::DeclaredValue;

/// §6.1: 对 declared values 列表按 cascade 准则排序（降序）。
///
/// 排序后列表首项为 cascade 胜出者。
///
/// **CC-4 占位**：当前按原始顺序返回，待 CC-4 实现完整排序。
pub fn cascade_for_element(declared: Vec<DeclaredValue>) -> Vec<DeclaredValue> {
    // CC-4 将实现完整排序逻辑
    declared
}
