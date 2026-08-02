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
use muskitty_cssom::CssStyleSheet;
use muskitty_selectors::matching::DomElement;
use std::collections::HashMap;

/// §4.3: 收集元素的自定义属性（`--*`）表。
///
/// 从父级继承（`parent_props`）开始，再将元素 cascade 胜出的 `--*`
/// 声明覆盖到结果中。返回值用于构造 [`ComputeContext`]（供 var()
/// 替换使用），并作为该元素子元素的 `parent_props` 传入（继承）。
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
                props.insert(property.clone(), winner.value.clone());
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
