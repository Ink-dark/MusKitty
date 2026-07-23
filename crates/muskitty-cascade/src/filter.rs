//! §5 Filtering — 收集 declared values。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §5 L814-844
//!
//! 遍历 stylesheet rules，对每条匹配元素的 CssStyleRule，收集其
//! declarations 作为 DeclaredValue。条件为 false 的 @media/@supports
//! 内的 rule 被跳过。
//!
//! CC-3 将填充完整实现。

use crate::style::DeclaredValue;
use muskitty_cssom::CssStyleSheet;
use muskitty_selectors::matching::DomElement;

/// §5: 收集元素的所有 declared values。
///
/// 遍历所有 stylesheet，对每条匹配 `element` 的 style rule，
/// 收集其 declarations。
///
/// **CC-3 占位**：当前返回空列表，待 CC-3 实现。
pub fn collect_declared_values(
    _element: &DomElement,
    _sheets: &[CssStyleSheet],
) -> Vec<DeclaredValue> {
    // CC-3 将实现完整逻辑
    Vec::new()
}
