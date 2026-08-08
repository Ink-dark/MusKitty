//! §4.3/§4.4 — 整树 computed style 计算。
//!
//! 规范源: CSS Cascading Level 5 §4.3 "Computed Value" / §4.2 "Specified Value"
//!         CSS Values Level 4 §5.5（em/rem 相对长度）
//!
//! [`compute_styles`] 对整棵 DOM 树做单次自顶向下遍历，为每个元素计算
//! `ComputedStyle`。与逐个元素手写 filter→cascade→defaulting→compute 相比：
//!
//! - **单次 cascade**（PERF-2）：`collect_declared_values` + `cascade_for_element`
//!   每元素只跑一次，`--*` 自定义属性表从同一份 cascade 分组派生，不再
//!   调用会重复级联的 `collect_custom_properties`。
//! - **font-size 传播**（P0-1）：两步算法——先用父 font-size 作 em/百分比
//!   基准算本元素 font-size，再用本元素 font-size 作其余属性的 em 基准；
//!   rem 基准（根元素 font-size）自根向下传播。
//!
//! 参考实现：Servo `components/style/cascade.rs::compute_style` + 两遍
//! font-size（先字体后其余属性，em 语义 = 元素自身 font-size）。

use crate::cascade::{cascade_for_element, cascade_winner};
use crate::compute::{compute_value_with, ComputeContext, CustomPropertySource};
use crate::custom_properties::is_css_wide_keyword;
use crate::defaulting::apply_defaulting;
use crate::filter::{collect_declared_values_prepared, prepare_sheets, PreparedSheets};
use crate::registry::BUILTIN_PROPERTIES;
use crate::style::{ComputedStyle, ComputedValue, DeclaredValue};
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::{Numeric, Token};
use muskitty_cssom::CssStyleSheet;
use muskitty_dom::{Node, NodeKind};
use muskitty_selectors::matching::DomElement;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 浏览器默认 font-size（px）。CSS 初始值 `medium` = 16px。
const DEFAULT_FONT_SIZE: f64 = 16.0;

/// 整树样式计算的视口选项（vw/vh 单位解析需要）。
#[derive(Debug, Clone, Copy)]
pub struct StyleTreeOptions {
    /// 视口宽度（px）。
    pub viewport_width: f64,
    /// 视口高度（px）。
    pub viewport_height: f64,
}

impl Default for StyleTreeOptions {
    fn default() -> Self {
        Self {
            viewport_width: 1920.0,
            viewport_height: 1080.0,
        }
    }
}

/// §4.3: 计算整棵 DOM 树的 computed style。
///
/// 返回 `HashMap<usize, ComputedStyle>`，key 为 `Rc::as_ptr(node) as usize`
/// （与现有跨 crate 约定一致；opaque 句柄化见
/// docs/security-audit-2026-08-02.md M-1，推迟到架构重构）。
/// 非 Element 节点（Document/Text/Comment）不产生样式条目。
///
/// 继承链：`--*` 自定义属性与继承属性（color/font-* 等）自父向子传递；
/// font-size 按两步算法传播（见模块文档）。
pub fn compute_styles(
    root: &Rc<RefCell<Node>>,
    sheets: &[CssStyleSheet],
    options: &StyleTreeOptions,
) -> HashMap<usize, ComputedStyle> {
    // PERF-1: 选择器解析一次，整树每个元素复用 prepared sheets。
    let prepared = prepare_sheets(sheets);
    let mut styles = HashMap::new();
    walk(
        root,
        &prepared,
        options,
        None, // 根元素无父级自定义属性来源
        None,
        DEFAULT_FONT_SIZE,
        None,
        &mut styles,
    );
    styles
}

/// 自顶向下遍历 DOM 树。
///
/// - `prepared`：已预处理的 sheets（PERF-1，选择器已缓存、整树复用）。
/// - `parent_source`：父级 `--*` 来源（链式继承，PERF-4；根为 `None`）。
/// - `parent_style`：父元素 ComputedStyle（继承属性）。
/// - `parent_font_size`：父元素 font-size（px），根为浏览器默认 16px。
/// - `root_font_size`：根元素 font-size（px）；根元素自身计算时为 `None`。
#[allow(clippy::too_many_arguments)]
fn walk<'a>(
    node: &Rc<RefCell<Node>>,
    prepared: &PreparedSheets,
    options: &StyleTreeOptions,
    parent_source: Option<&'a CustomPropertySource<'a>>,
    parent_style: Option<&ComputedStyle>,
    parent_font_size: f64,
    root_font_size: Option<f64>,
    styles: &mut HashMap<usize, ComputedStyle>,
) {
    // 非 Element 节点（Document/Text/Comment）：不计算样式，原样向下递归。
    if !matches!(&node.borrow().kind, NodeKind::Element(_)) {
        let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
        for child in &children {
            walk(
                child,
                prepared,
                options,
                parent_source,
                parent_style,
                parent_font_size,
                root_font_size,
                styles,
            );
        }
        return;
    }

    let addr = Rc::as_ptr(node) as usize;

    // 每元素一次 filter + cascade，派生本元素 `--*` 表 + 属性组（PERF-2）。
    let element = DomElement::new(Rc::clone(node));
    let declared = collect_declared_values_prepared(&element, prepared);
    let groups = cascade_for_element(declared);
    let own_props = derive_own_custom_props(&groups);
    // 链式来源：本元素声明优先，未声明回溯父链（PERF-4 零克隆继承）。
    let source = CustomPropertySource::Chain {
        own: &own_props,
        parent: parent_source,
    };
    let (cs, own_font_size) = compute_element_style(
        &groups,
        parent_style,
        parent_font_size,
        root_font_size,
        options,
        &source,
    );
    styles.insert(addr, cs);

    // 根元素自身计算 font-size 后，其 px 成为子树 rem 基准。
    let child_root_fs = if root_font_size.is_none() {
        Some(own_font_size)
    } else {
        root_font_size
    };
    let parent_cs = styles.get(&addr).cloned();
    let children: Vec<Rc<RefCell<Node>>> = node.borrow().child_nodes().to_vec();
    for child in &children {
        walk(
            child,
            prepared,
            options,
            Some(&source),
            parent_cs.as_ref(),
            own_font_size,
            child_root_fs,
            styles,
        );
    }
}

/// 从 cascade 分组派生本元素自己的 `--*` 表（不含继承）。
///
/// 与 [`crate::collect_custom_properties`] 等价，但复用调用方已完成的
/// `cascade_for_element` 结果，避免第二次级联（PERF-2）；且不克隆父级表
/// —— 继承由调用方构造 [`CustomPropertySource::Chain`] 链式回溯完成
/// （PERF-4 零克隆）。
fn derive_own_custom_props(
    groups: &HashMap<String, Vec<DeclaredValue>>,
) -> HashMap<String, Vec<ComponentValue>> {
    let mut props = HashMap::new();
    for (name, group) in groups {
        if name.starts_with("--") {
            if let Some(winner) = cascade_winner(group) {
                // P2-4：CSS-wide 关键字（initial/inherit/unset/revert）不写入，
                // 避免 var() 替换出字面量关键字。
                if !is_css_wide_keyword(&winner.value) {
                    props.insert(name.clone(), winner.value.clone());
                }
            }
        }
    }
    props
}

/// 计算单个元素的 ComputedStyle，返回 `(style, own_font_size_px)`。
///
/// 两步 font-size 算法：
/// 1. 用父 font-size 作 em/百分比基准算本元素 font-size → px；
/// 2. 用本元素 font-size 作 em 基准算其余属性（em 语义 = 自身 font-size）。
fn compute_element_style<'a>(
    groups: &HashMap<String, Vec<DeclaredValue>>,
    parent_style: Option<&ComputedStyle>,
    parent_font_size: f64,
    root_font_size: Option<f64>,
    options: &StyleTreeOptions,
    source: &'a CustomPropertySource<'a>,
) -> (ComputedStyle, f64) {
    let root_fs = root_font_size.unwrap_or(DEFAULT_FONT_SIZE);
    let mut cs = ComputedStyle::new();

    // 步骤 1：font-size（em/百分比基准 = 父 font-size）。
    let fs_ctx = ComputeContext::with_source(
        source,
        parent_font_size,
        root_fs,
        options.viewport_width,
        options.viewport_height,
    );
    let font_size = compute_one("font-size", groups, parent_style, &fs_ctx);
    let own_font_size = extract_font_size_px(&font_size).unwrap_or(parent_font_size);
    // CSS 语义下 font-size 的 computed value 是解析后的长度：关键字（如
    // "medium"=16px）归一化为 px Dimension，供 layout 等下游直接读取。
    cs.set("font-size", normalize_font_size(font_size, own_font_size));

    // 步骤 2：其余属性（em 基准 = 自身 font-size）。
    let ctx = ComputeContext::with_source(
        source,
        own_font_size,
        root_fs,
        options.viewport_width,
        options.viewport_height,
    );
    for property in groups.keys() {
        if property == "font-size" {
            continue;
        }
        let computed = compute_one(property, groups, parent_style, &ctx);
        cs.set(property.clone(), computed);
    }
    for prop_def in BUILTIN_PROPERTIES.iter() {
        if prop_def.name == "font-size" || cs.properties.contains_key(prop_def.name) {
            continue;
        }
        let computed = compute_one(prop_def.name, groups, parent_style, &ctx);
        cs.set(prop_def.name.to_string(), computed);
    }

    (cs, own_font_size)
}

/// 对单个属性执行 defaulting + compute。
///
/// cascade 胜者 → `apply_defaulting`（CSS-wide 关键字/继承/初始值）→
/// 若是 `Raw` 中间态则 `compute_value_with` 解析相对单位与 var()。
/// 值含无效 var()（`Err`）时按 unset 处理（css-variables-1 §3.1：
/// invalid at computed-value time）：继承属性取父值、非继承属性取初始值
/// —— 与未声明的 `apply_defaulting(property, None, parent)` 等价（P2-5）。
fn compute_one(
    property: &str,
    groups: &HashMap<String, Vec<DeclaredValue>>,
    parent_style: Option<&ComputedStyle>,
    ctx: &ComputeContext,
) -> ComputedValue {
    let winner = groups.get(property).and_then(|g| cascade_winner(g));
    let cascaded = winner.map(|w| w.value.as_slice());
    let parent_value = parent_style.and_then(|ps| ps.get(property));
    let specified = apply_defaulting(property, cascaded, parent_value);
    match &specified {
        ComputedValue::Raw(cvs) => match compute_value_with(property, cvs, ctx) {
            Ok(computed) => computed,
            Err(()) => apply_defaulting(property, None, parent_value),
        },
        _ => specified,
    }
}

/// 将 font-size 的 `Keyword` 值归一化为 px `Resolved`。
///
/// 关键字形态（`medium`、初始值数字字符串）说明该值来自 defaulting 而非
/// 显式长度声明；CSS 语义下 font-size 的计算值是长度，故转成 px Dimension。
/// 已是 `Resolved`/`Raw` 的（显式 px/em 等）原样保留。
fn normalize_font_size(cv: ComputedValue, px: f64) -> ComputedValue {
    match cv {
        ComputedValue::Keyword(_) => {
            ComputedValue::Resolved(vec![ComponentValue::PreservedToken(Token::Dimension(
                Numeric {
                    value: px,
                    is_integer: false,
                },
                "px".to_string(),
            ))])
        }
        other => other,
    }
}

/// 从 ComputedStyle 的 font-size 值提取 px 数值。
///
/// - `Resolved`/`Raw`：取第一个 px Dimension。
/// - `Keyword`：`medium`（=16px）或数字字符串（如初始值 "16px"）。
/// - 其余返回 `None`（调用方回退到父 font-size）。
fn extract_font_size_px(cv: &ComputedValue) -> Option<f64> {
    match cv {
        ComputedValue::Resolved(cvs) | ComputedValue::Raw(cvs) => {
            cvs.iter().find_map(|v| match v {
                ComponentValue::PreservedToken(Token::Dimension(n, u))
                    if u.eq_ignore_ascii_case("px") =>
                {
                    Some(n.value)
                }
                _ => None,
            })
        }
        ComputedValue::Keyword(s) => {
            if s.eq_ignore_ascii_case("medium") {
                Some(DEFAULT_FONT_SIZE)
            } else {
                s.trim().parse::<f64>().ok()
            }
        }
    }
}
