//! §4.3 Computed Value — 自定义属性收集。
//!
//! 规范源: CSS Cascading Level 4 §4.3 "Computed Value"
//!
//! 自定义属性声明参与 cascade，其值从 cascade 结果中收集并供 var()
//! 替换使用。CSS 自定义属性是继承属性：子元素未声明时继承父级收集
//! 到的 custom properties。
//!
//! 参考实现：Servo `components/style/cascade.rs::compute_style` 在
//! cascade 完成后从已 cascaded 的声明中提取 `--*`。

use crate::cascade::{cascade_for_element, cascade_winner};
use crate::filter::collect_declared_values;
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;
use muskitty_cssom::CssStyleSheet;
use muskitty_selectors::matching::DomElement;
use std::collections::HashMap;

/// 是否为 CSS-wide 关键字值（`initial`/`inherit`/`unset`/`revert`）。
///
/// css-variables-1 §2：这些关键字不作为自定义属性值被保留，而是触发其在
/// 自定义属性上的正常行为。收集时跳过（P2-4），避免 `var()` 替换出字面量
/// 关键字：
/// - `--x: inherit` → 继承父链同名属性（若不跳过会写入字面量 "inherit"）；
/// - `--x: initial` → 不写入 props。注意规范要求 `initial` 使该属性为
///   guaranteed-invalid 的**初始值**（不继承父链）；本 pipeline 为
///   Author-only，简化为"不写入"，若父链已声明同名属性会继承之 —— 这是
///   已知近似（见计划 B4 范围边界）。
pub(crate) fn is_css_wide_keyword(value: &[ComponentValue]) -> bool {
    let mut idents = 0;
    let mut keyword: Option<&str> = None;
    for cv in value {
        match cv {
            ComponentValue::PreservedToken(Token::Whitespace) => continue,
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                idents += 1;
                if keyword.is_none() {
                    keyword = Some(s.as_str());
                }
            }
            _ => return false,
        }
    }
    idents == 1
        && keyword.is_some_and(|k| {
            k.eq_ignore_ascii_case("initial")
                || k.eq_ignore_ascii_case("inherit")
                || k.eq_ignore_ascii_case("unset")
                || k.eq_ignore_ascii_case("revert")
        })
}

/// §4.3: 收集元素的自定义属性（`--*`）表。
///
/// 从父级继承（`parent_props`）开始，再将元素 cascade 胜出的 `--*`
/// 声明覆盖到结果中。返回值用于构造 [`ComputeContext`]（供 var()
/// 替换使用），并作为该元素子元素的 `parent_props` 传入（继承）。
///
/// 注意：本函数为原语路径（每元素单独级联）；整树路径应使用
/// [`crate::style_tree::compute_styles`]（单次级联 + 零克隆链式继承）。
///
/// [`ComputeContext`]: crate::compute::ComputeContext
pub fn collect_custom_properties(
    element: &DomElement,
    sheets: &[CssStyleSheet],
    parent_props: &HashMap<String, Vec<ComponentValue>>,
) -> HashMap<String, Vec<ComponentValue>> {
    let mut props = parent_props.clone();
    let declared = collect_declared_values(element, sheets);
    let groups = cascade_for_element(declared);
    for (property, group) in &groups {
        // 仅收集自定义属性（`--*`），普通属性不进入 custom properties 表。
        if property.starts_with("--") {
            if let Some(winner) = cascade_winner(group) {
                // P2-4：CSS-wide 关键字（initial/inherit/unset/revert）不写入，
                // 避免 var() 替换出字面量关键字。
                if !is_css_wide_keyword(&winner.value) {
                    props.insert(property.clone(), winner.value.clone());
                }
            }
        }
    }
    props
}

#[cfg(test)]
mod tests {
    use super::*;
    use muskitty_cssom::{from_stylesheet, Origin};
    use muskitty_dom::{Node, NodeKind};
    use muskitty_selectors::matching::Element as _;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn parse_dom(html: &str) -> Rc<RefCell<Node>> {
        muskitty_html5_parser::parse(html)
    }

    fn author_sheet(css: &str) -> CssStyleSheet {
        let parsed = muskitty_css::parse_stylesheet(css);
        let mut s = from_stylesheet(&parsed);
        s.origin = Origin::Author;
        s
    }

    fn find_element(
        node: &Rc<RefCell<Node>>,
        predicate: &dyn Fn(&DomElement) -> bool,
    ) -> Option<DomElement> {
        if matches!(&node.borrow().kind, NodeKind::Element(_)) {
            let el = DomElement::new(Rc::clone(node));
            if predicate(&el) {
                return Some(el);
            }
        }
        for child in node.borrow().child_nodes() {
            if let Some(found) = find_element(child, predicate) {
                return Some(found);
            }
        }
        None
    }

    fn element_with_id(node: &Rc<RefCell<Node>>, id: &str) -> Option<DomElement> {
        find_element(node, &|el| el.get_attribute("id").as_deref() == Some(id))
    }

    #[test]
    fn collects_custom_property_from_root() {
        let dom = parse_dom(r#"<html><body><div id="a"></div></body></html>"#);
        let sheets = [author_sheet(":root { --brand: red; }")];
        let empty = HashMap::new();
        let root = find_element(&dom, &|el| el.local_name() == "html").expect("html root");
        let props = collect_custom_properties(&root, &sheets, &empty);
        // :root 规则匹配 html → --brand 被收集
        assert_eq!(props.len(), 1);
        assert!(props.contains_key("--brand"));

        // 子元素继承根级 custom properties
        let el = element_with_id(&dom, "a").expect("div#a");
        let inherited = collect_custom_properties(&el, &sheets, &props);
        assert!(inherited.contains_key("--brand"));
    }

    #[test]
    fn child_inherits_and_override() {
        let dom = parse_dom(
            r#"<html><body>
                <div id="child" style="--brand: blue">
                    <span id="grand"></span>
                </div>
            </body></html>"#,
        );
        let sheets = [author_sheet(":root { --brand: red; }")];
        let empty = HashMap::new();
        let child = element_with_id(&dom, "child").expect("div#child");
        let child_props = collect_custom_properties(&child, &sheets, &empty);
        // 子元素声明覆盖根级 → blue
        assert_eq!(child_props.get("--brand").unwrap().len(), 1);

        let grand = element_with_id(&dom, "grand").expect("span#grand");
        let grand_props = collect_custom_properties(&grand, &sheets, &child_props);
        // 孙元素未声明 → 继承父级 blue
        assert_eq!(grand_props.get("--brand").unwrap().len(), 1);
    }
}
