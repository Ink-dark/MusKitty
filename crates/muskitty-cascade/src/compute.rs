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

/// var() 查找自定义属性值的来源。
///
/// - [`Flat`]: 平面表 —— 单元素原语路径（`compute_value` 直接调用方），
///   继承表由调用方克隆合并。
/// - [`Chain`]: 链式继承 —— 整树路径（[`crate::style_tree::compute_styles`]），
///   `own` 只含本元素 cascade 胜出的 `--*` 声明，查表先 own 再回溯 `parent`
///   链，每层零克隆（PERF-4）。
///
/// [`Flat`]: CustomPropertySource::Flat
/// [`Chain`]: CustomPropertySource::Chain
#[derive(Debug, Clone)]
pub enum CustomPropertySource<'a> {
    /// 平面表。
    Flat(&'a HashMap<String, Vec<ComponentValue>>),
    /// 链式继承：本元素声明表 + 父链。
    Chain {
        /// 本元素 cascade 胜出的 `--*` 声明。
        own: &'a HashMap<String, Vec<ComponentValue>>,
        /// 父元素来源（继承回溯）。
        parent: Option<&'a CustomPropertySource<'a>>,
    },
}

impl<'a> CustomPropertySource<'a> {
    /// 按名称查找自定义属性值；链式来源先查 own，再回溯父链。
    pub fn get(&self, name: &str) -> Option<&'a Vec<ComponentValue>> {
        match self {
            CustomPropertySource::Flat(map) => map.get(name),
            CustomPropertySource::Chain { own, parent } => {
                own.get(name).or_else(|| parent.and_then(|p| p.get(name)))
            }
        }
    }
}

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
    /// 自定义属性来源（name → value），用于 var() 替换。
    pub custom_properties: CustomPropertySource<'a>,
}

impl<'a> ComputeContext<'a> {
    /// 创建默认上下文（font-size 16px, viewport 1920x1080, 平面自定义属性）。
    pub fn new(custom_properties: &'a HashMap<String, Vec<ComponentValue>>) -> Self {
        Self {
            parent_font_size: 16.0,
            root_font_size: 16.0,
            viewport_width: 1920.0,
            viewport_height: 1080.0,
            custom_properties: CustomPropertySource::Flat(custom_properties),
        }
    }

    /// 用显式 font-size 与视口尺寸构造上下文（平面自定义属性来源）。
    ///
    /// `parent_font_size` 是 em/百分比基准：计算 font-size 属性时为父元素
    /// font-size；计算其余属性时为元素自身 font-size（em 语义）。`root_font_size`
    /// 是 rem 基准（根元素 font-size，px）。
    pub fn with_font_sizes(
        custom_properties: &'a HashMap<String, Vec<ComponentValue>>,
        parent_font_size: f64,
        root_font_size: f64,
        viewport_width: f64,
        viewport_height: f64,
    ) -> Self {
        Self {
            parent_font_size,
            root_font_size,
            viewport_width,
            viewport_height,
            custom_properties: CustomPropertySource::Flat(custom_properties),
        }
    }

    /// 用链式自定义属性来源构造上下文（整树路径，PERF-4 零克隆继承）。
    pub fn with_source(
        source: &'a CustomPropertySource<'a>,
        parent_font_size: f64,
        root_font_size: f64,
        viewport_width: f64,
        viewport_height: f64,
    ) -> Self {
        Self {
            parent_font_size,
            root_font_size,
            viewport_width,
            viewport_height,
            custom_properties: source.clone(),
        }
    }
}

/// §4.4: 将 specified value 转换为 computed value。
///
/// 处理：
/// - 相对长度单位（em/rem/vh/vw/vmin/vmax）→ px
/// - `var()` 替换（记忆化递归，含 fallback）
/// - font-size 百分比 → px
///
/// 其他属性的百分比保持原样（推迟到 layout 阶段解析）。
///
/// 兼容包装：值含无效 var()（首参非 `--*` 或引用 guaranteed-invalid 且无
/// fallback）时返回空 `Resolved`（旧行为）；整树路径应使用
/// [`compute_value_with`] 感知 invalid-at-computed-value（P2-5）。
pub fn compute_value(
    property: &str,
    specified: &[ComponentValue],
    ctx: &ComputeContext,
) -> ComputedValue {
    compute_value_with(property, specified, ctx)
        .unwrap_or_else(|_| ComputedValue::from_tokens(Vec::new()))
}

/// §4.4: 同 [`compute_value`]，但报告 invalid-at-computed-value。
///
/// `Err(())` = 值含无效 var()（css-variables-1 §3.1）：首参非自定义属性名，
/// 或引用了 guaranteed-invalid 的自定义属性且无 fallback。调用方（如
/// [`crate::style_tree`]）此时应将该属性按 unset 处理（继承属性取父值、
/// 非继承属性取初始值）。
///
/// `Err` 携带 `()`：无效即 sentinel，无需错误数据；调用方只需区分
/// 有效/无效（B4 计划约定的签名）。
#[allow(clippy::result_unit_err)]
pub fn compute_value_with(
    property: &str,
    specified: &[ComponentValue],
    ctx: &ComputeContext,
) -> Result<ComputedValue, ()> {
    let mut resolver = VarResolver::new(ctx);
    let resolved = resolver.resolve_tokens(specified, property)?;
    Ok(ComputedValue::from_tokens(resolved))
}

/// 单个自定义属性的解析结果。
#[derive(Debug, Clone)]
enum ResolvedVar {
    /// 完整展开后的 token 序列（相对单位已按当前 ctx 解析）。
    Tokens(Vec<ComponentValue>),
    /// guaranteed-invalid（css-variables-1 §3.1）：环、未定义、或值含
    /// 无效 var()。
    GuaranteedInvalid,
}

/// §3 var() 解析器：3 色 DFS 记忆化（P1-1 / PERF-3）。
///
/// - `resolved`（black）：已解析完成的变量，结果直接复用；
/// - `in_progress`（gray）：解析中的变量（DFS 栈），命中即环。
///
/// 每个 var 名在同一解析器内至多求值一次。对 `--v{i}: var(--v{i-1})
/// var(--v{i-1})` 这类重复引用链，输出规模 2^N 是语义使然，但每级只展开
/// 一次 —— 时间线性于输出规模，不再按 N×2^N 指数重复递归（修复前实测
/// N=22 需 30.9s）。
struct VarResolver<'a> {
    ctx: &'a ComputeContext<'a>,
    resolved: HashMap<String, ResolvedVar>,
    in_progress: HashSet<String>,
}

impl<'a> VarResolver<'a> {
    fn new(ctx: &'a ComputeContext<'a>) -> Self {
        Self {
            ctx,
            resolved: HashMap::new(),
            in_progress: HashSet::new(),
        }
    }

    /// 解析一组 token。任一 token 无效 → `Err`（整条属性 invalid at
    /// computed-value time）。
    ///
    /// 输入 token 的生命周期独立于 `'a`：调用方可能是属性 specified 值
    /// （短借用），也可能是 `ctx` 内自定义属性值（长借用）。
    fn resolve_tokens(
        &mut self,
        tokens: &[ComponentValue],
        property: &str,
    ) -> Result<Vec<ComponentValue>, ()> {
        let mut out = Vec::with_capacity(tokens.len());
        for cv in tokens {
            out.extend(self.resolve_component(cv, property)?);
        }
        Ok(out)
    }

    /// 递归解析单个 component value。
    ///
    /// 返回 `Vec` 是因为 `var()` 替换可能展开为多个值；`Err` 表示无效
    /// var()（见 [`compute_value_with`]）。
    fn resolve_component(
        &mut self,
        cv: &ComponentValue,
        property: &str,
    ) -> Result<Vec<ComponentValue>, ()> {
        match cv {
            // 相对长度单位解析
            ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
                Ok(resolve_dimension(numeric, unit, property, self.ctx))
            }
            // 百分比解析（仅 font-size 等需要在此阶段解析）
            ComponentValue::PreservedToken(Token::Percentage(numeric)) => {
                Ok(resolve_percentage(numeric, property, self.ctx))
            }
            // var() 替换：首参非 `--*` 或语法无效 → invalid at computed-value time（P2-5）
            ComponentValue::Function(func) if func.name.eq_ignore_ascii_case("var") => {
                let (name, fallback) = parse_var_args(&func.value).ok_or(())?;
                self.resolve_var_ref(&name, fallback, property)
            }
            // 其他函数（如 calc()）— 递归解析参数
            ComponentValue::Function(func) => {
                let resolved_args = self.resolve_tokens(&func.value, property)?;
                Ok(vec![ComponentValue::Function(Function {
                    name: func.name.clone(),
                    value: resolved_args,
                })])
            }
            // 其他 token 原样保留
            other => Ok(vec![other.clone()]),
        }
    }

    /// 解析单个 var() 引用。
    ///
    /// 被引用的自定义属性 guaranteed-invalid（环、未定义、值含无效 var()）时：
    /// 有 fallback 则递归解析 fallback（P1-2），无 fallback 则 `Err`。
    fn resolve_var_ref(
        &mut self,
        name: &str,
        fallback: &[ComponentValue],
        property: &str,
    ) -> Result<Vec<ComponentValue>, ()> {
        let ctx = self.ctx; // Copy，避免字段访问与 &mut self 冲突

        // black：已解析 → 直接复用展开结果。
        let cached = self.resolved.get(name).cloned();
        if let Some(rv) = cached {
            return self.materialize(rv, fallback, property);
        }
        // gray：DFS 栈命中 → 环。该 var() 视为 guaranteed-invalid（P1-2）。
        if !self.in_progress.insert(name.to_string()) {
            return self.resolve_fallback(fallback, property);
        }

        // 首次解析：求值该变量的值（可能含嵌套 var()）。
        let result = match ctx.custom_properties.get(name) {
            Some(value) => self.resolve_tokens(value, property),
            None => Err(()), // 未定义 → guaranteed-invalid
        };
        self.in_progress.remove(name);

        let rv = match result {
            Ok(tokens) => ResolvedVar::Tokens(tokens),
            Err(()) => ResolvedVar::GuaranteedInvalid,
        };
        self.resolved.insert(name.to_string(), rv.clone());
        self.materialize(rv, fallback, property)
    }

    /// 将缓存结果物化为输出：Tokens 直接返回；GuaranteedInvalid 走 fallback。
    fn materialize(
        &mut self,
        rv: ResolvedVar,
        fallback: &[ComponentValue],
        property: &str,
    ) -> Result<Vec<ComponentValue>, ()> {
        match rv {
            ResolvedVar::Tokens(tokens) => Ok(tokens),
            ResolvedVar::GuaranteedInvalid => self.resolve_fallback(fallback, property),
        }
    }

    /// 解析 fallback（被引用变量 guaranteed-invalid 时）。无 fallback → Err。
    fn resolve_fallback(
        &mut self,
        fallback: &[ComponentValue],
        property: &str,
    ) -> Result<Vec<ComponentValue>, ()> {
        if fallback.is_empty() {
            Err(())
        } else {
            self.resolve_tokens(fallback, property)
        }
    }
}

/// 解析 `var()` 参数：`var(--name, fallback...)`。
///
/// 返回 `(name, fallback_tokens)`。css-variables-1 §3.1 要求首参是自定义
/// 属性名（`--*`）；否则该 var() 在 computed-value time 无效（P2-5）。
fn parse_var_args(value: &[ComponentValue]) -> Option<(String, &[ComponentValue])> {
    // 跳过前导空白，取第一个非空白 token 作为首参。
    let mut name = None;
    let mut i = 0;
    while i < value.len() {
        match &value[i] {
            ComponentValue::PreservedToken(Token::Whitespace) => i += 1,
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                name = Some(s.clone());
                i += 1;
                break;
            }
            _ => return None, // 首参不是 ident → 无效
        }
    }
    let name = name?;
    if !name.starts_with("--") {
        return None; // 首参非自定义属性名 → invalid at computed-value time
    }
    // 找逗号；逗号前不得再有 token。
    let mut j = i;
    while j < value.len() {
        match &value[j] {
            ComponentValue::PreservedToken(Token::Whitespace) => j += 1,
            ComponentValue::PreservedToken(Token::Comma) => break,
            _ => return None,
        }
    }
    // 逗号后的全部是 fallback。跳过后导前导空白：前导空白替换后无语义，
    // 且旧实现（resolve_var 的 Whitespace => continue）本就丢弃，保持兼容
    // （避免 cvs[0] 为 Whitespace 破坏下游按首 token 的断言）。
    let mut fallback_start = j + 1;
    while fallback_start < value.len() {
        if let ComponentValue::PreservedToken(Token::Whitespace) = &value[fallback_start] {
            fallback_start += 1;
        } else {
            break;
        }
    }
    let fallback = if j < value.len() {
        &value[fallback_start..]
    } else {
        &[]
    };
    Some((name, fallback))
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
    // PERF-5：逐 eq_ignore_ascii_case 分支匹配，避免 to_ascii_lowercase()
    // 堆分配（px 等常用单位也经过该分支）。
    let resolved = if unit.eq_ignore_ascii_case("em") {
        Some(value * ctx.parent_font_size)
    } else if unit.eq_ignore_ascii_case("rem") {
        Some(value * ctx.root_font_size)
    } else if unit.eq_ignore_ascii_case("vh") {
        Some(value * ctx.viewport_height / 100.0)
    } else if unit.eq_ignore_ascii_case("vw") {
        Some(value * ctx.viewport_width / 100.0)
    } else if unit.eq_ignore_ascii_case("vmin") {
        Some(value * ctx.viewport_width.min(ctx.viewport_height) / 100.0)
    } else if unit.eq_ignore_ascii_case("vmax") {
        Some(value * ctx.viewport_width.max(ctx.viewport_height) / 100.0)
    } else {
        // 绝对单位 — 不转换
        None
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
        let cvs = result.tokens();
        assert_eq!(cvs.len(), 1);
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 40.0);
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
        }
    }

    #[test]
    fn rem_resolves_to_px_using_root_font_size() {
        let ctx = ComputeContext {
            root_font_size: 18.0,
            ..empty_ctx()
        };
        let result = compute_value("margin-top", &[dim(3.0, "rem")], &ctx);
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 54.0);
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
        }
    }

    #[test]
    fn vh_resolves_to_px() {
        let ctx = ComputeContext {
            viewport_height: 1000.0,
            ..empty_ctx()
        };
        let result = compute_value("height", &[dim(50.0, "vh")], &ctx);
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                assert_eq!(n.value, 500.0);
            }
            other => panic!("expected Dimension, got {:?}", other),
        }
    }

    #[test]
    fn vw_resolves_to_px() {
        let ctx = ComputeContext {
            viewport_width: 800.0,
            ..empty_ctx()
        };
        let result = compute_value("width", &[dim(25.0, "vw")], &ctx);
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                assert_eq!(n.value, 200.0);
            }
            other => panic!("expected Dimension, got {:?}", other),
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
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                assert_eq!(n.value, 60.0);
            }
            other => panic!("expected Dimension, got {:?}", other),
        }
        // vmax = max(800, 600) = 800, 10vmax = 80px
        let result = compute_value("width", &[dim(10.0, "vmax")], &ctx);
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, _)) => {
                assert_eq!(n.value, 80.0);
            }
            other => panic!("expected Dimension, got {:?}", other),
        }
    }

    #[test]
    fn px_unit_preserved() {
        let result = compute_value("width", &[dim(100.0, "px")], &empty_ctx());
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 100.0);
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
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
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 30.0);
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
        }
    }

    #[test]
    fn width_percentage_preserved() {
        // width 的百分比推迟到 layout — 原样保留
        let result = compute_value("width", &[pct(50.0)], &empty_ctx());
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Percentage(n)) => {
                assert_eq!(n.value, 50.0);
            }
            other => panic!("expected Percentage, got {:?}", other),
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
        let cvs = result.tokens();
        assert_eq!(cvs.len(), 1);
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "red");
            }
            other => panic!("expected Ident, got {:?}", other),
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
        let cvs = result.tokens();
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
        let cvs = result.tokens();
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Dimension(n, u)) => {
                assert_eq!(n.value, 40.0);
                assert_eq!(u, "px");
            }
            other => panic!("expected Dimension, got {:?}", other),
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
        assert!(
            result.tokens().is_empty(),
            "self-cycle must resolve to empty"
        );
    }

    #[test]
    fn var_two_cycle_returns_empty() {
        // --a: var(--b); --b: var(--a) → 双环 → 空
        let mut props = HashMap::new();
        props.insert("--a".to_string(), vec![var_fn("--b")]);
        props.insert("--b".to_string(), vec![var_fn("--a")]);
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn("--a")], &ctx);
        assert!(
            result.tokens().is_empty(),
            "two-cycle must resolve to empty"
        );
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
        assert!(
            result.tokens().is_empty(),
            "triangle-cycle must resolve to empty"
        );
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
        let cvs = result.tokens();
        assert_eq!(cvs.len(), 1);
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Ident(s)) => assert_eq!(s, "red"),
            other => panic!("expected Ident, got {:?}", other),
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
        let cvs = result.tokens();
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

    // —— var() 记忆化（P1-1 / PERF-3）与 invalid-at-computed-value（P2-5）——

    /// `var(--name, fallback)` 形式的函数节点。
    fn var_fn_fb(name: &str, fallback: &str) -> ComponentValue {
        ComponentValue::Function(Function {
            name: "var".to_string(),
            value: vec![
                ComponentValue::PreservedToken(Token::Ident(name.to_string())),
                ComponentValue::PreservedToken(Token::Comma),
                ComponentValue::PreservedToken(Token::Whitespace),
                ComponentValue::PreservedToken(Token::Ident(fallback.to_string())),
            ],
        })
    }

    #[test]
    fn var_doubling_chain_resolves_in_linear_time() {
        // --v0: red; --v{i}: var(--v{i-1}) var(--v{i-1})
        // 输出规模 2^N 是语义使然；记忆化保证每级只展开一次，时间线性于输出。
        // 修复前（N=22）需 30.9s 且按 N×2^N 指数重复递归。
        const N: usize = 18;
        let mut props = HashMap::new();
        props.insert(
            "--v0".to_string(),
            vec![ComponentValue::PreservedToken(Token::Ident(
                "red".to_string(),
            ))],
        );
        for i in 1..=N {
            let prev = format!("--v{}", i - 1);
            props.insert(format!("--v{}", i), vec![var_fn(&prev), var_fn(&prev)]);
        }
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn(&format!("--v{}", N))], &ctx);
        let cvs = result.tokens();
        assert_eq!(cvs.len(), 1 << N, "doubling chain expands to 2^N tokens");
        let reds = cvs
            .iter()
            .filter(|cv| {
                matches!(
                    cv,
                    ComponentValue::PreservedToken(Token::Ident(s)) if s == "red"
                )
            })
            .count();
        assert_eq!(reds, 1 << N, "all tokens must be `red`");
    }

    #[test]
    fn var_cycle_with_fallback_resolves_to_fallback() {
        // P1-2: 环中变量 guaranteed-invalid，但 var() 有 fallback 则解析 fallback。
        // --a: var(--b, red); --b: var(--a, blue) → var(--a) 得 blue
        // （--b 值含 var(--a, blue)：--a 在 gray 命中 → fallback blue → --b=blue
        //   → var(--b, red) 用 --b 计算值 blue，忽略 fallback red）。
        let mut props = HashMap::new();
        props.insert("--a".to_string(), vec![var_fn_fb("--b", "red")]);
        props.insert("--b".to_string(), vec![var_fn_fb("--a", "blue")]);
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn("--a")], &ctx);
        let cvs = result.tokens();
        let idents: Vec<_> = cvs
            .iter()
            .filter_map(|cv| match cv {
                ComponentValue::PreservedToken(Token::Ident(s)) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert!(
            idents.iter().any(|s| s == "blue"),
            "expected fallback blue, got {:?}",
            idents
        );
    }

    #[test]
    fn var_undefined_with_fallback_still_resolves() {
        // 未定义变量 + fallback → 仍解析为 fallback（既有行为保持）
        let props = HashMap::new();
        let ctx = ctx_with_custom(&props);
        let result = compute_value("color", &[var_fn_fb("--nope", "green")], &ctx);
        let cvs = result.tokens();
        assert!(
            cvs.iter().any(|cv| matches!(
                cv,
                ComponentValue::PreservedToken(Token::Ident(s)) if s == "green"
            )),
            "expected fallback green, got {:?}",
            cvs
        );
    }

    #[test]
    fn compute_value_with_reports_invalid_var() {
        // P2-5: var(color) 首参非 --* → Err（invalid at computed-value time）
        let props = HashMap::new();
        let ctx = ctx_with_custom(&props);
        let bad = ComponentValue::Function(Function {
            name: "var".to_string(),
            value: vec![ComponentValue::PreservedToken(Token::Ident(
                "color".to_string(),
            ))],
        });
        assert!(
            compute_value_with("color", &[bad], &ctx).is_err(),
            "var(color) must be Err via compute_value_with"
        );
        // 兼容包装：compute_value 对该场景返回空 tokens（旧行为）
        let empty = compute_value(
            "color",
            &[ComponentValue::Function(Function {
                name: "var".to_string(),
                value: vec![ComponentValue::PreservedToken(Token::Ident(
                    "color".to_string(),
                ))],
            })],
            &ctx,
        );
        assert!(empty.tokens().is_empty());
    }

    // —— 混合值 ——

    #[test]
    fn keyword_value_preserved() {
        let id = ComponentValue::PreservedToken(Token::Ident("auto".to_string()));
        let result = compute_value("width", &[id], &empty_ctx());
        let cvs = result.tokens();
        assert_eq!(cvs.len(), 1);
        match &cvs[0] {
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                assert_eq!(s, "auto");
            }
            other => panic!("expected Ident, got {:?}", other),
        }
    }
}
