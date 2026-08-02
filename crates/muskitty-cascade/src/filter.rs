//! §5 Filtering — 收集 declared values。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §5 L814-844
//!
//! 遍历 stylesheet rules，对每条匹配元素的 CssStyleRule，收集其
//! declarations 作为 DeclaredValue。条件为 false 的 @media/@supports
//! 内的 rule 被跳过（本阶段简化为无条件收集，条件评估推迟）。
//! 同时收集元素 inline `style` 属性中的声明（§6.1 准则 4）。

use crate::style::DeclaredValue;
use muskitty_css::parser::{parse_a_blocks_contents, Rule};
use muskitty_cssom::{serialize_component_values, CssRule, CssStyleSheet, Origin};
use muskitty_selectors::matching::{matches, DomElement, Element as ElementTrait};
use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::Specificity;

/// §5: 收集元素的所有 declared values。
///
/// 遍历所有 stylesheet，对每条匹配 `element` 的 style rule，
/// 收集其 declarations。递归处理嵌套 rules 和条件 group rules
/// （@media/@supports/@container/@layer）。
/// 最后收集元素 inline `style` 属性中的声明。
///
/// **简化**：条件 group rules 的条件评估推迟，当前无条件收集
/// 所有嵌套 rules。
pub fn collect_declared_values(
    element: &DomElement,
    sheets: &[CssStyleSheet],
) -> Vec<DeclaredValue> {
    let mut result = Vec::new();
    let mut order = 0usize;

    for sheet in sheets {
        collect_from_rules(
            &sheet.css_rules,
            sheet.origin,
            element,
            &mut order,
            &mut result,
        );
    }

    // §6.1 准则 4: 收集 inline style 属性的声明
    collect_from_style_attr(element, &mut order, &mut result);

    result
}

/// 从元素 inline `style` 属性收集声明。
///
/// inline style 声明的 specificity 为 (1,0,0,0)（最高优先级），
/// origin 为 Author，from_style_attr = true。
fn collect_from_style_attr(
    element: &DomElement,
    order: &mut usize,
    result: &mut Vec<DeclaredValue>,
) {
    let style_str = match element.get_attribute("style") {
        Some(s) => s,
        None => return,
    };

    let block_contents = parse_a_blocks_contents(&style_str);
    // §6.1 准则 4: inline style 通过 from_style_attr 标志单独排序，
    // specificity 本身为 (0,0,0)（准则 3 不会额外加权）
    let specificity = Specificity::new(0, 0, 0);

    for rule in &block_contents.rules {
        if let Rule::Declarations(decls) = rule {
            for decl in decls {
                *order += 1;
                result.push(DeclaredValue {
                    property: decl.name.clone(),
                    value: decl.value.clone(),
                    important: decl.important,
                    origin: Origin::Author,
                    specificity,
                    order: *order,
                    from_style_attr: true,
                });
            }
        }
    }
}

/// 递归遍历 rules，收集匹配元素的 declared values。
fn collect_from_rules(
    rules: &[CssRule],
    origin: Origin,
    element: &DomElement,
    order: &mut usize,
    result: &mut Vec<DeclaredValue>,
) {
    for rule in rules {
        match rule {
            CssRule::Style(style_rule) => {
                // §5 L829: "Its declaration's selector matches the element"
                let selector_str = serialize_component_values(&style_rule.selectors);
                if let Ok(selector_list) = parse_a_selector(&selector_str) {
                    if matches(&selector_list, element) {
                        let specificity = selector_list.specificity_max();
                        for decl in &style_rule.style.declarations {
                            *order += 1;
                            result.push(DeclaredValue {
                                property: decl.name.clone(),
                                value: decl.value.clone(),
                                important: decl.important,
                                origin,
                                specificity,
                                order: *order,
                                from_style_attr: false,
                            });
                        }
                    }
                }
                // 递归处理 CSS nesting 子 rules
                collect_from_rules(&style_rule.css_rules, origin, element, order, result);
            }
            CssRule::Media(r) => {
                // 简化：无条件收集（条件评估推迟）
                collect_from_rules(&r.css_rules, origin, element, order, result);
            }
            CssRule::Supports(r) => {
                collect_from_rules(&r.css_rules, origin, element, order, result);
            }
            CssRule::Container(r) => {
                collect_from_rules(&r.css_rules, origin, element, order, result);
            }
            CssRule::LayerBlock(r) => {
                collect_from_rules(&r.css_rules, origin, element, order, result);
            }
            CssRule::Other(r) => {
                collect_from_rules(&r.child_rules, origin, element, order, result);
            }
            // 非样式 rule 跳过
            CssRule::Import(_) | CssRule::Namespace(_) | CssRule::LayerStatement(_) => {}
        }
    }
}
