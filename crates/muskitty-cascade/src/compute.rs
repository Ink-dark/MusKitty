//! §4.4 Computed Value — 相对单位解析、var() 求值、百分比解析。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §4.4 L500-555
//!
//! 将 specified value 转换为 computed value：
//! - 相对长度单位（em/rem/vh/vw/vmin/vmax）→ px
//! - `var()` 替换（递归求值 fallback）
//! - font-size 百分比 → px（其他属性的百分比推迟到 layout 阶段）

use crate::registry::{lookup_property, PercentageBasis};
use crate::style::ComputedValue;
use muskitty_css::parser::{ComponentValue, Function};
use muskitty_css::tokenizer::{Numeric, Token};
use std::collections::{HashMap, HashSet};

/// §4.4: Computed value 计算上下文。
///
/// 提供相对单位解析、var() 替换所需的上下文数据。
pub struct ComputeContext<'a> {
    /// 父元素 font-size（px），用于 em 解析。
    pub parent_font_size: f64,
    /// 根元素 font-size（px），用于 rem 解析。
    pub root_font_size: f64,
    /// 视口宽度（px），用于 vw/vmin/vmax 解析。
    pub viewport_width: f64,
    /// 视口高度（px），用于 vh/vmin/vmax 解析。
    pub viewport_height: f64,
    /// 自定义属性表（name → value），用于 var() 替换。
    pub custom_properties: &'a HashMap<String, Vec<ComponentValue>>,
}

impl<'a> ComputeContext<'a> {
    /// 创建默认上下文（font-size 16px, viewport 1920x1080, 空自定义属性）。
    pub fn new(custom_properties: &'a HashMap<String, Vec<ComponentValue>>) -> Self {
        Self {
            parent_font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            custom_properties,
        }
    }
}

/// §4.4: 将 specified value 转换为 computed value。
///
/// 处理：
/// - 相对长度单位（em/rem/vh/vw/vmin/vmax）→ px
/// - `var()` 替换（递归求值 fallback）
/// - font-size 百分比 → px
///
/// 其他属性的百分比保持原样（推迟到 layout 阶段解析）。
pub fn compute_value(
    property: &str,
    specified: &[ComponentValue],
    ctx: &ComputeContext,
) -> ComputedValue {
    // §3 CSS Variables: 每条计算路径使用独立的 visited 集合检测 var()
    // 循环引用（--a → --b → --a 等），避免无限递归导致栈溢出。
    let mut visited: HashSet<String> = HashSet::new();
    let resolved: Vec<ComponentValue> = specified
        .iter()
        .flat_map(|cv| resolve_component_value(cv, property, ctx, &mut visited))
        .collect();

    ComputedValue::Resolved(resolved)
}

/// 递归解析单个 component value。
///
/// 返回 `Vec<ComponentValue>` 是因为 `var()` 替换可能展开为多个值。
fn resolve_component_value(
    cv: &ComponentValue,
    property: &str,
    ctx: &ComputeContext,
    visited: &mut HashSet<String>,
) -> Vec<ComponentValue> {
    match cv {
        // 相对长度单位解析
        ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
            resolve_dimension(numeric, unit, property, ctx)
        }
        // 百分比解析（仅 font-size 等需要在此阶段解析）
        ComponentValue::PreservedToken(Token::Percentage(numeric)) => {
            resolve_percentage(numeric, property, ctx)
        }
        // var() 替换
        ComponentValue::Function(func) if func.name.eq_ignore_ascii_case("var") => {
            resolve_var(func, ctx, property, visited)
        }
        // 其他函数（如 calc()）— 递归解析参数
        ComponentValue::Function(func) => {
            let resolved_args: Vec<ComponentValue> = func
                .value
                .iter()
                .flat_map(|arg| resolve_component_value(arg, property, ctx, visited))
                .collect();
            vec![ComponentValue::Function(Function {
                name: func.name.clone(),
                value: resolved_args,
            })]
        }
        // 其他 token 原样保留
        other => vec![other.clone()],
    }
}

/// 解析相对长度维度（em/rem/vh/vw/vmin/vmax → px）。
///
/// 绝对单位（px/pt/pc/in/cm/mm）原样保留。
fn resolve_dimension(
    numeric: &Numeric,
    unit: &str,
    _property: &str,
    ctx: &ComputeContext,
) -> Vec<ComponentValue> {
    let value = numeric.value;
    let resolved = match unit.to_ascii_lowercase().as_str() {
        // 字体相对单位
        "em" => Some(value * ctx.parent_font_size),
        "rem" => Some(value * ctx.root_font_size),
        // 视口相对单位
        "vh" => Some(value * ctx.viewport_height / 100.0),
        "vw" => Some(value * ctx.viewport_width / 100.0),
        "vmin" => Some(value * ctx.viewport_width.min(ctx.viewport_height) / 100.0),
        "vmax" => Some(value * ctx.viewport_width.max(ctx.viewport_height) / 100.0),
        // 绝对单位 — 不转换
        _ => None,
    };

    match resolved {
        Some(px) => vec![ComponentValue::PreservedToken(Token::Dimension(
            Numeric {
                value: px,
                is_integer: false,
            },
            "px".to_string(),
        ))],
        None => vec![ComponentValue::PreservedToken(Token::Dimension(
            numeric.clone(),
            unit.to_string(),
        ))],
    }
}

/// 解析百分比。
///
/// 仅 font-size（PercentageBasis::ParentFontSize）和
/// ParentSameProperty（如果父值是绝对长度）在此阶段解析。
/// 其他百分比保持原样（推迟到 layout）。
fn resolve_percentage(
    numeric: &Numeric,
    property: &str,
    ctx: &ComputeContext,
) -> Vec<ComponentValue> {
    let basis = lookup_property(property).map(|d| d.percentages);

    match basis {
        Some(PercentageBasis::ParentFontSize) => {
            // font-size: 120% → 1.2 * parent_font_size px
            let px = numeric.value / 100.0 * ctx.parent_font_size;
            vec![ComponentValue::PreservedToken(Token::Dimension(
                Numeric {
                    value: px,
                    is_integer: false,
                },
                "px".to_string(),
            ))]
        }
        Some(PercentageBasis::RootFontSize) => {
            let px = numeric.value / 100.0 * ctx.root_font_size;
            vec![ComponentValue::PreservedToken(Token::Dimension(
                Numeric {
                    value: px,
                    is_integer: false,
                },
                "px".to_string(),
            ))]
        }
        // 其他百分比基准（ParentWidth/ParentHeight/ParentSameProperty/None）
        // 推迟到 layout 阶段解析 — 原样保留
        _ => vec![ComponentValue::PreservedToken(Token::Percentage(
            numeric.clone(),
        ))],
    }
}

/// §3 CSS Variables: var() 替换。
///
/// `var(--name, fallback)` → 查找自定义属性 --name，
/// 找到则替换为其值（递归解析），未找到则使用 fallback。
fn resolve_var(
    func: &Function,
    ctx: &ComputeContext,
    property: &str,
    visited: &mut HashSet<String>,
) -> Vec<ComponentValue> {
    // 解析参数：第一个 ident 是自定义属性名，逗号后是 fallback
    let mut var_name: Option<String> = None;
    let mut fallback: Vec<ComponentValue> = Vec::new();
    let mut after_comma = false;

    for arg in &func.value {
        match arg {
            ComponentValue::PreservedToken(Token::Whitespace) => continue,
            ComponentValue::PreservedToken(Token::Comma) => {
                after_comma = true;
            }
            ComponentValue::PreservedToken(Token::Ident(s))
                if !after_comma && var_name.is_none() =>
            {
                var_name = Some(s.clone());
            }
            other if after_comma => {
                fallback.push(other.clone());
            }
            _ => {}
        }
    }

    let name = match var_name {
        Some(n) => n,
        None => return Vec::new(), // 无效的 var() 调用
    };

    // §3 CSS Variables "Cycles in Custom Properties":
    // 若 --name 已在当前替换路径（visited）中，说明其值直接或间接
    // 依赖自身，该 var() 必须视为 invalid，返回空 Vec。
    if !visited.insert(name.clone()) {
        return Vec::new();
    }

    let result = if let Some(value) = ctx.custom_properties.get(&name) {
        // 递归解析替换值（可能含嵌套 var()）
        value
            .iter()
            .flat_map(|cv| resolve_component_value(cv, property, ctx, visited))
            .collect()
    } else {
        // 使用 fallback（递归解析）
        fallback
            .iter()
            .flat_map(|cv| resolve_component_value(cv, property, ctx, visited))
            .collect()
    };

    // 本路径解析完成，移除该名字：同一属性可被多条路径引用（DAG），
    // 只有沿单条替换路径上的重复才构成环。
    visited.remove(&name);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(value: f64, unit: &str) -> ComponentValue {
        ComponentValue::PreservedToken(Token::Dimension(
            Numeric {
                value,
                is_integer: false,
            },
            unit.to_string(),
        ))
    }

    fn pct(value: f64) -> ComponentValue {
        ComponentValue::PreservedToken(Token::Percentage(Numeric {
            value,
            is_integer: false,
        }))
    }

    fn empty_ctx() -> ComputeContext<'static> {
        static EMPTY: std::sync::OnceLock<HashMap<String, Vec<ComponentValue>>> =
            std::sync::OnceLock::new();
        let props = EMPTY.get_or_init(HashMap::new);
        ComputeContext::new(props)
    }

    fn ctx_with_custom(props: &HashMap<String, Vec<ComponentValue>>) -> ComputeContext<'_> {
        ComputeContext::new(props)
    }

    // —— 相对单位解析 ——

    #[test]
    fn em_resolves_to_px_using_parent_font_size() {
        let ctx = ComputeContext {
            parent_font_size: 20.0,
            ..empty_ctx()
        };
        let result = compute_value("margin-top", &[dim(2.0, "em")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                assert_eq!(cvs.len(), 1);
                match &cvs[0] {
                    ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                        assert_eq!(n.value, 40.0);
                        assert_eq!(u, "px");
                    }
                    other => panic!("expected Dimension, got {:?}", other),
                }
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn rem_resolves_to_px_using_root_font_size() {
        let ctx = ComputeContext {
            root_font_size: 18.0,
            ..empty_ctx()
        };
        let result = compute_value("margin-top", &[dim(3.0, "rem")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                    assert_eq!(n.value, 54.0);
                    assert_eq!(u, "px");
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn vh_resolves_to_px() {
        let ctx = ComputeContext {
            viewport_height: 1000.0,
            ..empty_ctx()
        };
        let result = compute_value("height", &[dim(50.0, "vh")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                    assert_eq!(n.value, 500.0);
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn vw_resolves_to_px() {
        let ctx = ComputeContext {
            viewport_width: 800.0,
            ..empty_ctx()
        };
        let result = compute_value("width", &[dim(25.0, "vw")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                    assert_eq!(n.value, 200.0);
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn vmin_and_vmax_resolve() {
        let ctx = ComputeContext {
            viewport_width: 800.0,
            viewport_height: 600.0,
            ..empty_ctx()
        };
        // vmin = min(800, 600) = 600, 10vmin = 60px
        let result = compute_value("width", &[dim(10.0, "vmin")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                    assert_eq!(n.value, 60.0);
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
        // vmax = max(800, 600) = 800, 10vmax = 80px
        let result = compute_value("width", &[dim(10.0, "vmax")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                    assert_eq!(n.value, 80.0);
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn px_unit_preserved() {
        let result = compute_value("width", &[dim(100.0, "px")], &empty_ctx());
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                    assert_eq!(n.value, 100.0);
                    assert_eq!(u, "px");
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    // —— 百分比解析 ——

    #[test]
    fn font_size_percentage_resolves_to_px() {
        let ctx = ComputeContext {
            parent_font_size: 20.0,
            ..empty_ctx()
        };
        // font-size: 150% → 30px
        let result = compute_value("font-size", &[pct(150.0)], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                    assert_eq!(n.value, 30.0);
                    assert_eq!(u, "px");
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn width_percentage_preserved() {
        // width 的百分比推迟到 layout — 原样保留
        let result = compute_value("width", &[pct(50.0)], &empty_ctx());
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Percentage(n)) => {
                    assert_eq!(n.value, 50.0);
                }
                other => panic!("expected Percentage, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    // —— var() 替换 ——

    #[test]
    fn var_substitutes_custom_property() {
        let mut props = HashMap::new();
        props.insert(
            "--main-color".to_string(),
            vec![ComponentValue::PreservedToken(Token::Ident(
                "red".to_string(),
            ))],
        );
        let ctx = ctx_with_custom(&props);

        let var_fn = ComponentValue::Function(Function {
            name: "var".to_string(),
            value: vec![ComponentValue::PreservedToken(Token::Ident(
                "--main-color".to_string(),
            ))],
        });

        let result = compute_value("color", &[var_fn], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                assert_eq!(cvs.len(), 1);
                match &cvs[0] {
                    ComponentValue::PreservedToken(Token::Ident(s)) => {
                        assert_eq!(s, "red");
                    }
                    other => panic!("expected Ident, got {:?}", other),
                }
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn var_uses_fallback_when_undefined() {
        let props = HashMap::new();
        let ctx = ctx_with_custom(&props);

        let var_fn = ComponentValue::Function(Function {
            name: "var".to_string(),
            value: vec![
                ComponentValue::PreservedToken(Token::Ident("--undefined".to_string())),
                ComponentValue::PreservedToken(Token::Comma),
                ComponentValue::PreservedToken(Token::Whitespace),
                ComponentValue::PreservedToken(Token::Ident("blue".to_string())),
            ],
        });

        let result = compute_value("color", &[var_fn], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                // Whitespace is preserved in fallback
                let idents: Vec<_> = cvs
                    .iter()
                    .filter_map(|cv| match cv {
                        ComponentValue::PreservedToken(Token::Ident(s)) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();
                assert!(idents.contains(&"blue".to_string()));
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn var_resolves_relative_units_in_substitution() {
        let mut props = HashMap::new();
        props.insert("--gap".to_string(), vec![dim(2.0, "em")]);
        let ctx = ComputeContext {
            parent_font_size: 20.0,
            ..ctx_with_custom(&props)
        };

        let var_fn = ComponentValue::Function(Function {
            name: "var".to_string(),
            value: vec![ComponentValue::PreservedToken(Token::Ident(
                "--gap".to_string(),
            ))],
        });

        // var(--gap) where --gap = 2em, parent font-size = 20px → 40px
        let result = compute_value("margin-top", &[var_fn], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => match &cvs[0] {
                ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                    assert_eq!(n.value, 40.0);
                    assert_eq!(u, "px");
                }
                other => panic!("expected Dimension, got {:?}", other),
            },
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    // —— §3 var() 循环检测 ——

    fn var_fn(name: &str) -> ComponentValue {
        ComponentValue::Function(Function {
            name: "var".to_string(),
            value: vec![ComponentValue::PreservedToken(Token::Ident(
                name.to_string(),
            ))],
        })
    }

    #[test]
    fn var_self_reference_returns_empty() {
        // --a: var(--a) → 自引用 → 空
        let mut props = HashMap::new();
        props.insert("--a".to_string(), vec![var_fn("--a")]);
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn("--a")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                assert!(cvs.is_empty(), "self-cycle must resolve to empty");
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn var_two_cycle_returns_empty() {
        // --a: var(--b); --b: var(--a) → 双环 → 空
        let mut props = HashMap::new();
        props.insert("--a".to_string(), vec![var_fn("--b")]);
        props.insert("--b".to_string(), vec![var_fn("--a")]);
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn("--a")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                assert!(cvs.is_empty(), "two-cycle must resolve to empty");
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn var_triangle_cycle_returns_empty() {
        // --a: var(--b); --b: var(--c); --c: var(--a) → 三角环 → 空
        let mut props = HashMap::new();
        props.insert("--a".to_string(), vec![var_fn("--b")]);
        props.insert("--b".to_string(), vec![var_fn("--c")]);
        props.insert("--c".to_string(), vec![var_fn("--a")]);
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn("--a")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                assert!(cvs.is_empty(), "triangle-cycle must resolve to empty");
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn var_normal_chain_still_resolves() {
        // --a: var(--b); --b: red → 正常链 → red
        let mut props = HashMap::new();
        props.insert("--a".to_string(), vec![var_fn("--b")]);
        props.insert(
            "--b".to_string(),
            vec![ComponentValue::PreservedToken(Token::Ident(
                "red".to_string(),
            ))],
        );
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn("--a")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                assert_eq!(cvs.len(), 1);
                match &cvs[0] {
                    ComponentValue::PreservedToken(Token::Ident(s)) => assert_eq!(s, "red"),
                    other => panic!("expected Ident, got {:?}", other),
                }
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    #[test]
    fn var_repeated_reference_is_not_a_cycle() {
        // --a: var(--b) var(--b); --b: red → 同一属性重复引用不是环
        let mut props = HashMap::new();
        props.insert("--a".to_string(), vec![var_fn("--b"), var_fn("--b")]);
        props.insert(
            "--b".to_string(),
            vec![ComponentValue::PreservedToken(Token::Ident(
                "red".to_string(),
            ))],
        );
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn("--a")], &ctx);
        match result {
            ComputedValue::Resolved(cvs) => {
                assert_eq!(cvs.len(), 2);
                let reds = cvs
                    .iter()
                    .filter(|cv| {
                        matches!(
                            cv,
                            ComponentValue::PreservedToken(Token::Ident(s)) if s == "red"
                        )
                    })
                    .count();
                assert_eq!(reds, 2);
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }

    // —— 混合值 ——

    #[test]
    fn keyword_value_preserved() {
        let id = ComponentValue::PreservedToken(Token::Ident("auto".to_string()));
        let result = compute_value("width", &[id], &empty_ctx());
        match result {
            ComputedValue::Resolved(cvs) => {
                assert_eq!(cvs.len(), 1);
                match &cvs[0] {
                    ComponentValue::PreservedToken(Token::Ident(s)) => {
                        assert_eq!(s, "auto");
                    }
                    other => panic!("expected Ident, got {:?}", other),
                }
            }
            other => panic!("expected Resolved, got {:?}", other),
        }
    }
}
