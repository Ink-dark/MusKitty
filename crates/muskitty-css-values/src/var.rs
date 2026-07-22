//! var() 语法解析 — CSS Variables Level 1 §3。
//!
//! 解析 `var(--name, <fallback>?)` 的语法树，不做求值。
//! 求值（查 custom property 值、循环检测、fallback 激活）留到 Cascade 阶段。

use crate::numeric::ParseError;
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;

/// var() 引用的语法解析结果 (css-variables-1 §3)。
///
/// 只解析语法结构，不求值。
#[derive(Debug, Clone)]
pub struct VarReference {
    /// 自定义属性名（如 `--foo`），含 `--` 前缀。
    pub name: String,
    /// 可选的 fallback 值（逗号后的 component values，保留原始顺序含 whitespace）。
    ///
    /// - `None`：无 fallback（无逗号）
    /// - `Some(vec![])`：空 fallback（bare comma，逗号后无值）
    /// - `Some(cvs)`：正常 fallback
    pub fallback: Option<Vec<ComponentValue>>,
}

impl VarReference {
    /// 从 CSS 字符串解析 `var()` 引用。
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::from_cvs(&cvs)
    }

    /// 从 ComponentValue 列表解析。
    pub fn from_cvs(cvs: &[ComponentValue]) -> Result<Self, ParseError> {
        // 找到 var() function（在 cvs 中可能被 whitespace 包围）
        let func = cvs.iter().find_map(|cv| match cv {
            ComponentValue::Function(f) if f.name.eq_ignore_ascii_case("var") => Some(f),
            _ => None,
        });

        let func = func.ok_or_else(|| ParseError::new("expected var() function"))?;
        Self::from_function(func)
    }

    /// 从已知的 var() Function 解析（供 calc() 嵌套调用）。
    pub fn from_function(func: &muskitty_css::parser::Function) -> Result<Self, ParseError> {
        // 第一个参数必须是 custom-property-name (--ident)
        // 过滤首尾 whitespace 找第一个非 ws token
        let first_non_ws = func
            .value
            .iter()
            .find(|cv| !is_whitespace(cv))
            .ok_or_else(|| ParseError::new("var() requires at least one argument"))?;

        let name = match first_non_ws {
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                if !is_custom_property_name(s) {
                    return Err(ParseError::new(format!(
                        "var() first argument must be a custom property name (starting with '--'), got: {s}"
                    )));
                }
                s.clone()
            }
            _ => {
                return Err(ParseError::new(
                    "var() first argument must be an ident token",
                ))
            }
        };

        // 查找逗号分隔 fallback
        // 逗号后的所有 component values（保留原始顺序，包括 whitespace）作为 fallback
        let comma_idx = func
            .value
            .iter()
            .position(|cv| matches!(cv, ComponentValue::PreservedToken(Token::Comma)));

        let fallback = match comma_idx {
            None => None,
            Some(idx) => {
                // 逗号后可能有内容（包括空），全部保留
                let after_comma = &func.value[idx + 1..];
                Some(after_comma.to_vec())
            }
        };

        Ok(VarReference { name, fallback })
    }
}

/// 检查字符串是否是 custom-property-name (css-variables-1 §2)。
///
/// 必须以 `--` 开头且长度 > 2（`--` 本身不是合法的 custom property 名）。
pub fn is_custom_property_name(s: &str) -> bool {
    s.starts_with("--") && s.len() > 2
}

/// 判断 ComponentValue 是否是 whitespace token。
fn is_whitespace(cv: &ComponentValue) -> bool {
    matches!(cv, ComponentValue::PreservedToken(Token::Whitespace))
}
