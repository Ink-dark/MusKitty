//! §5 Filtering — 收集 declared values。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §5 L814-844
//!
//! 遍历 stylesheet rules，对每条匹配元素的 CssStyleRule，收集其
//! declarations 作为 DeclaredValue。条件为 false 的 @media/@supports
//! 内的 rule 被跳过（本阶段简化为无条件收集，条件评估推迟）。
//! 同时收集元素 inline `style` 属性中的声明（§6.1 准则 4）。
//!
//! # PERF-1 选择器缓存
//!
//! [`prepare_sheets`] 在预处理阶段把每个 style rule 的选择器**一次**
//! 完成 serialize→parse→`SelectorList` 并缓存（含 specificity）；此后
//! 每个元素匹配直接复用缓存，零重复解析。`compute_styles` 走
//! prepare 一次 + [`collect_declared_values_prepared`] 逐元素复用。
//! 便捷入口 [`collect_declared_values`] 每次调用内部 prepare + collect
//! （保持旧 API，测试/单次场景使用）。

use crate::style::DeclaredValue;
use muskitty_css::parser::{parse_a_blocks_contents, ComponentValue, Rule};
use muskitty_cssom::{serialize_component_values, CssRule, CssStyleSheet, Origin};
use muskitty_selectors::matching::{matches, DomElement, Element as ElementTrait};
use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::SelectorList;
use muskitty_selectors::Specificity;

/// 预处理后的 stylesheet 集合（PERF-1）。
///
/// 每个 style rule 的选择器已解析并缓存；`LayerBlock`/条件组已扁平化，
/// 每条 [`PreparedRule`] 保持文档序（嵌套子 rules 紧随父规则之后），
/// 匹配遍历与旧版递归顺序一致。
pub struct PreparedSheets {
    rules: Vec<PreparedRule>,
}

/// 一条已解析的 style rule。
struct PreparedRule {
    /// 已缓存的选择器列表（PERF-1）。
    selector_list: SelectorList,
    /// 选择器 max specificity。
    specificity: Specificity,
    /// 来源 origin。
    origin: Origin,
    /// 所属 @layer 的全局序号（P1-3）；`None` = 未分层。
    layer_order: Option<usize>,
    /// 声明块（元素无关，prepare 时克隆）。
    declarations: Vec<PreparedDecl>,
}

/// 一条声明的数据（元素无关）。
struct PreparedDecl {
    name: String,
    value: Vec<ComponentValue>,
    important: bool,
}

/// 预处理 stylesheet 集：每个 style rule 的选择器只解析一次。
///
/// 遍历与旧版 `collect_from_rules` 的递归一致：style rule 收集、
/// 嵌套子 rules 递归、条件组（@media/@supports/@container/@layer）
/// 无条件穿过、import/namespace 跳过。选择器解析失败的 rule 跳过但
/// 仍递归其子 rules（与旧版行为一致）。
///
/// P1-3：跨全部 sheets 按文档序为每个 @layer 名分配全局序号（首次出现
/// 定序），风格规则携带其所属层的序号。
pub fn prepare_sheets(sheets: &[CssStyleSheet]) -> PreparedSheets {
    let mut rules = Vec::new();
    let mut layers = LayerTracker::new();
    for sheet in sheets {
        prepare_rules(&sheet.css_rules, sheet.origin, &mut layers, &mut rules);
    }
    PreparedSheets { rules }
}

/// @layer 全局序号分配器（P1-3）。
///
/// 按文档序首次出现的层名分配递增序号；匿名层每次出现分配新序号。
/// 嵌套层采用扁平近似（`docs/audit-2026-08-08-full-scan.md` P1-3）：内层
/// 块以其自身名/匿名身份获得独立序号，不拼接父层名。
struct LayerTracker {
    /// 已见层名 → 序号。
    by_name: std::collections::HashMap<String, usize>,
    /// 下一个待分配的序号。
    next: usize,
    /// 当前嵌套层序号栈（`None` 不入栈；栈空 = 顶层）。
    stack: Vec<Option<usize>>,
}

impl LayerTracker {
    fn new() -> Self {
        Self {
            by_name: std::collections::HashMap::new(),
            next: 0,
            stack: Vec::new(),
        }
    }

    /// 声明（或再次引用）一个层，返回其全局序号。首次出现分配新序号；
    /// 匿名层（`None`）总是分配新序号。
    fn declare(&mut self, name: Option<&str>) -> usize {
        match name {
            Some(n) => *self.by_name.entry(n.to_string()).or_insert_with(|| {
                let i = self.next;
                self.next += 1;
                i
            }),
            None => {
                let i = self.next;
                self.next += 1;
                i
            }
        }
    }

    /// 进入一个层块：登记序号并压栈。
    fn enter(&mut self, name: Option<&str>) {
        let idx = self.declare(name);
        self.stack.push(Some(idx));
    }

    /// 退出当前层块。
    fn exit(&mut self) {
        self.stack.pop();
    }

    /// 当前层序号（顶层 = `None`）。
    fn current(&self) -> Option<usize> {
        self.stack.last().copied().flatten()
    }
}

/// 递归遍历 rules，构建扁平化的 [`PreparedRule`] 列表。
fn prepare_rules(
    rules: &[CssRule],
    origin: Origin,
    layers: &mut LayerTracker,
    out: &mut Vec<PreparedRule>,
) {
    for rule in rules {
        match rule {
            CssRule::Style(style_rule) => {
                // §5 L829: "Its declaration's selector matches the element"
                let selector_str = serialize_component_values(&style_rule.selectors);
                if let Ok(selector_list) = parse_a_selector(&selector_str) {
                    let specificity = selector_list.specificity_max();
                    let declarations = style_rule
                        .style
                        .declarations
                        .iter()
                        .map(|d| PreparedDecl {
                            name: d.name.clone(),
                            value: d.value.clone(),
                            important: d.important,
                        })
                        .collect();
                    out.push(PreparedRule {
                        selector_list,
                        specificity,
                        origin,
                        layer_order: layers.current(),
                        declarations,
                    });
                }
                // 递归处理 CSS nesting 子 rules
                prepare_rules(&style_rule.css_rules, origin, layers, out);
            }
            CssRule::Media(r) => {
                // 简化：无条件收集（条件评估推迟）
                prepare_rules(&r.css_rules, origin, layers, out);
            }
            CssRule::Supports(r) => {
                prepare_rules(&r.css_rules, origin, layers, out);
            }
            CssRule::Container(r) => {
                prepare_rules(&r.css_rules, origin, layers, out);
            }
            CssRule::LayerBlock(r) => {
                // P1-3: 进入层块，层内规则携带该层序号
                layers.enter(r.name.as_deref());
                prepare_rules(&r.css_rules, origin, layers, out);
                layers.exit();
            }
            CssRule::LayerStatement(r) => {
                // P1-3: statement 形式声明层名（定序），无子规则
                for name in &r.names {
                    layers.declare(Some(name));
                }
            }
            CssRule::Other(r) => {
                prepare_rules(&r.child_rules, origin, layers, out);
            }
            // 非样式 rule 跳过
            CssRule::Import(_) | CssRule::Namespace(_) => {}
        }
    }
}

/// §5: 收集元素的所有 declared values。
///
/// 遍历所有 stylesheet，对每条匹配 `element` 的 style rule，
/// 收集其 declarations。递归处理嵌套 rules 和条件 group rules
/// （@media/@supports/@container/@layer）。
/// 最后收集元素 inline `style` 属性中的声明。
///
/// **简化**：条件 group rules 的条件评估推迟，当前无条件收集
/// 所有嵌套 rules。
///
/// 便捷入口：每次调用内部 [`prepare_sheets`]（选择器重解析）。热路径
/// （`compute_styles`）应改为 prepare 一次 + [`collect_declared_values_prepared`]。
pub fn collect_declared_values(
    element: &DomElement,
    sheets: &[CssStyleSheet],
) -> Vec<DeclaredValue> {
    let prepared = prepare_sheets(sheets);
    collect_declared_values_prepared(element, &prepared)
}

/// §5: 用已预处理的 sheets 收集元素 declared values（PERF-1）。
///
/// 遍历 `prepared` 中按文档序排列的 style rules，匹配即收集声明；
/// 匹配过程中零分配（选择器已缓存）。最后收集元素 inline `style`
/// 属性中的声明（§6.1 准则 4）。
pub fn collect_declared_values_prepared(
    element: &DomElement,
    prepared: &PreparedSheets,
) -> Vec<DeclaredValue> {
    let mut result = Vec::new();
    let mut order = 0usize;

    for rule in &prepared.rules {
        if matches(&rule.selector_list, element) {
            for decl in &rule.declarations {
                order += 1;
                result.push(DeclaredValue {
                    property: decl.name.clone(),
                    value: decl.value.clone(),
                    important: decl.important,
                    origin: rule.origin,
                    specificity: rule.specificity,
                    order,
                    from_style_attr: false,
                    layer_order: rule.layer_order,
                });
            }
        }
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
                    // inline style 不在任何 @layer 内
                    layer_order: None,
                });
            }
        }
    }
}
