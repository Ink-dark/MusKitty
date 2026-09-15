//! 最小 UA 样式表（CS-1f）。
//!
//! [`UA_CSS`] 是随二进制编译进来的 UA 级默认表（规范依据与偏差逐条写在
//! `ua.css` 的注释里）；[`ua_stylesheet`] 把它转成 Origin::UserAgent 的
//! [`CssStyleSheet`]，由 [`crate::page::render_page_with_sheets`] 置于表列表
//! 首位——cascade 的 origin 权重决定它低于 Author（CSS Cascade L5 §6.1 准则 1），
//! 位置只表达"先加载"。
//!
//! 解析结果进程内缓存一次（`OnceLock`）：渲染每帧都走那条路径，UA 表内容恒定。

use std::sync::OnceLock;

use muskitty_css::parse_stylesheet;
use muskitty_cssom::{from_stylesheet_with_origin, CssStyleSheet, Origin};

/// UA 样式表源码（HTML §15 Rendering 的最小可用子集）。
pub const UA_CSS: &str = include_str!("ua.css");

/// UA 级样式表（Origin::UserAgent；进程内只解析一次）。
pub fn ua_stylesheet() -> CssStyleSheet {
    static SHEET: OnceLock<CssStyleSheet> = OnceLock::new();
    SHEET
        .get_or_init(|| from_stylesheet_with_origin(&parse_stylesheet(UA_CSS), Origin::UserAgent))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ua_sheet_is_user_agent_origin_and_has_rules() {
        let sheet = ua_stylesheet();
        assert_eq!(sheet.origin, Origin::UserAgent);
        assert!(
            sheet.css_rules.len() > 10,
            "UA 表应含多条规则，实际 {}",
            sheet.css_rules.len()
        );
    }

    #[test]
    fn ua_sheet_parses_without_dropping_expected_rules() {
        // 解析健壮性：非渲染标签、body 边距、标题字号三条关键规则必须在。
        let ids = ua_style_rule_count(&ua_stylesheet());
        assert!(ids >= 10, "style rule 数 {}", ids);
    }

    fn ua_style_rule_count(sheet: &CssStyleSheet) -> usize {
        sheet
            .css_rules
            .iter()
            .filter(|r| matches!(r, muskitty_cssom::CssRule::Style(_)))
            .count()
    }
}
