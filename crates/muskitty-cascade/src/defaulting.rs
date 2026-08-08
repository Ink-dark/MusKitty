//! §7 Defaulting — initial/inherit/unset 关键字处理。
//!
//! 规范源: `d:\csswg\css-cascade-5\Overview.md` §7 L1505-1599
//!
//! 处理 CSS-wide 关键字（§7.3）和 cascade 无结果时的默认行为（§7.1-7.2）：
//! - `initial`（§7.3.1）→ 属性初始值
//! - `inherit`（§7.3.2）→ 父元素 computed value
//! - `unset`（§7.3.3）→ 继承属性当 `inherit`，非继承属性当 `initial`
//! - `revert`/`revert-layer`（§7.3.4-5）→ 当"无 cascaded value"处理
//!   （Author-only pipeline 下不存在更低 origin / 已回退的层）
//! - 无 cascaded value → 继承属性从父继承，非继承属性取初始值

use crate::registry::lookup_property;
use crate::style::ComputedValue;
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;

/// §7.3: 应用 defaulting，将 cascaded value 转换为 specified value。
///
/// - 若 cascaded value 是 CSS-wide 关键字（`initial`/`inherit`/`unset`），
///   按 §7.3.1-3 处理。
/// - 若 cascade 无结果（`None`），按 §7.1-7.2 defaulting：
///   继承属性从父元素继承，非继承属性取初始值。
/// - 否则原样返回（CC-6 `compute_value` 会进一步处理）。
pub fn apply_defaulting(
    property: &str,
    cascaded: Option<&[ComponentValue]>,
    parent_computed: Option<&ComputedValue>,
) -> ComputedValue {
    let def = lookup_property(property);
    let is_inherited = def.map(|d| d.inherited).unwrap_or(false);
    let initial_keyword =
        || ComputedValue::from_keyword(def.map(|d| d.initial_value).unwrap_or("initial"));

    match cascaded {
        Some(cvs) => {
            // 检查是否为 CSS-wide 关键字
            if let Some(keyword) = extract_single_ident(cvs) {
                match keyword.to_ascii_lowercase().as_str() {
                    // §7.3.1: initial → 属性初始值
                    "initial" => return initial_keyword(),
                    // §7.3.2: inherit → 父元素 computed value（无父则取初始值）
                    "inherit" => {
                        return parent_computed.cloned().unwrap_or_else(initial_keyword);
                    }
                    // §7.3.3: unset → 继承属性当 inherit，非继承当 initial
                    "unset" => {
                        if is_inherited {
                            return parent_computed.cloned().unwrap_or_else(initial_keyword);
                        } else {
                            return initial_keyword();
                        }
                    }
                    // §7.3.4-5: revert / revert-layer → 当"无 cascaded value"
                    // 处理（继承属性 inherit、非继承 initial）。
                    //
                    // §7.3.4: revert 回退到更低优先级 origin；本 pipeline
                    // 仅 Author origin，无更低 origin 可取 → 等价于未声明。
                    // §7.3.5: revert-layer 回退到上一条同名层；层排序在
                    // filter 内扁平化处理，此处近似"无 cascaded value"。
                    "revert" | "revert-layer" => {
                        if is_inherited {
                            return parent_computed.cloned().unwrap_or_else(initial_keyword);
                        } else {
                            return initial_keyword();
                        }
                    }
                    _ => {}
                }
            }
            // 普通值 → 原样返回（CC-6 compute_value 进一步处理）
            ComputedValue::from_tokens(cvs.to_vec())
        }
        // §7.1-7.2: cascade 无结果时的 defaulting
        None => {
            if is_inherited {
                // 继承属性 → 从父元素继承（根元素无父则取初始值）
                parent_computed.cloned().unwrap_or_else(initial_keyword)
            } else {
                // 非继承属性 → 取初始值
                initial_keyword()
            }
        }
    }
}

/// 从 component value 列表中提取单个 ident 关键字（跳过空白）。
///
/// 若列表含多个非空白 token 或非 ident token，返回 `None`。
fn extract_single_ident(cvs: &[ComponentValue]) -> Option<String> {
    let mut found: Option<String> = None;
    for cv in cvs {
        match cv {
            ComponentValue::PreservedToken(Token::Whitespace) => continue,
            ComponentValue::PreservedToken(Token::Ident(s)) => {
                if found.is_some() {
                    return None;
                }
                found = Some(s.clone());
            }
            _ => return None,
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(s: &str) -> Vec<ComponentValue> {
        vec![ComponentValue::PreservedToken(Token::Ident(s.to_string()))]
    }

    fn parent_keyword(s: &str) -> ComputedValue {
        ComputedValue::from_keyword(s)
    }

    // §7.3.1: initial 关键字

    #[test]
    fn initial_keyword_returns_property_initial_value() {
        // color 的初始值是 "black"
        let result = apply_defaulting("color", Some(&ident("initial")), None);
        assert_eq!(result.keyword(), Some("black"));
    }

    #[test]
    fn initial_keyword_for_non_inherited_property() {
        // display 的初始值是 "inline"
        let result = apply_defaulting("display", Some(&ident("initial")), None);
        assert_eq!(result.keyword(), Some("inline"));
    }

    // §7.3.2: inherit 关键字

    #[test]
    fn inherit_keyword_returns_parent_computed() {
        let parent = parent_keyword("red");
        let result = apply_defaulting("color", Some(&ident("inherit")), Some(&parent));
        assert_eq!(result.keyword(), Some("red"));
    }

    #[test]
    fn inherit_keyword_no_parent_falls_back_to_initial() {
        // 根元素无父 → 初始值
        let result = apply_defaulting("color", Some(&ident("inherit")), None);
        assert_eq!(result.keyword(), Some("black"));
    }

    // §7.3.3: unset 关键字

    #[test]
    fn unset_on_inherited_property_acts_as_inherit() {
        // color 是继承属性 → unset 当 inherit
        let parent = parent_keyword("blue");
        let result = apply_defaulting("color", Some(&ident("unset")), Some(&parent));
        assert_eq!(result.keyword(), Some("blue"));
    }

    #[test]
    fn unset_on_non_inherited_property_acts_as_initial() {
        // display 是非继承属性 → unset 当 initial
        let parent = parent_keyword("block");
        let result = apply_defaulting("display", Some(&ident("unset")), Some(&parent));
        assert_eq!(result.keyword(), Some("inline"));
    }

    #[test]
    fn unset_on_inherited_no_parent_falls_back_to_initial() {
        // color 继承属性，无父 → 初始值
        let result = apply_defaulting("color", Some(&ident("unset")), None);
        assert_eq!(result.keyword(), Some("black"));
    }

    // §7.1-7.2: cascade 无结果时的 defaulting

    #[test]
    fn no_cascaded_value_inherited_property_inherits_from_parent() {
        let parent = parent_keyword("green");
        let result = apply_defaulting("color", None, Some(&parent));
        assert_eq!(result.keyword(), Some("green"));
    }

    #[test]
    fn no_cascaded_value_non_inherited_property_uses_initial() {
        let parent = parent_keyword("block");
        let result = apply_defaulting("display", None, Some(&parent));
        assert_eq!(result.keyword(), Some("inline"));
    }

    #[test]
    fn no_cascaded_value_inherited_no_parent_uses_initial() {
        // 根元素，继承属性无父 → 初始值
        let result = apply_defaulting("color", None, None);
        assert_eq!(result.keyword(), Some("black"));
    }

    // 普通值透传

    #[test]
    fn normal_value_passes_through_as_raw() {
        let result = apply_defaulting("color", Some(&ident("red")), None);
        assert_eq!(result.tokens().len(), 1);
    }

    #[test]
    fn unknown_property_initial_keyword() {
        // 未注册属性 → 初始值回退为 "initial" 字符串
        let result = apply_defaulting("unknown-prop", Some(&ident("initial")), None);
        assert_eq!(result.keyword(), Some("initial"));
    }

    #[test]
    fn keyword_case_insensitive() {
        // CSS 关键字大小写不敏感
        let result = apply_defaulting("color", Some(&ident("INITIAL")), None);
        assert_eq!(result.keyword(), Some("black"));
    }

    // §7.3.4/7.3.5: revert / revert-layer 关键字

    #[test]
    fn revert_acts_as_no_cascaded_value_inherited_no_parent() {
        // Author-only pipeline：revert 回退的更低 origin 不存在 →
        // 当"无 cascaded value"处理（继承属性无父 → 初始值）
        let result = apply_defaulting("color", Some(&ident("revert")), None);
        assert_eq!(result.keyword(), Some("black"));
    }

    #[test]
    fn revert_acts_as_no_cascaded_value_inherited_with_parent() {
        let parent = parent_keyword("green");
        let result = apply_defaulting("color", Some(&ident("revert")), Some(&parent));
        assert_eq!(result.keyword(), Some("green"));
    }

    #[test]
    fn revert_acts_as_no_cascaded_value_non_inherited() {
        // 非继承属性 → 初始值
        let parent = parent_keyword("block");
        let result = apply_defaulting("display", Some(&ident("revert")), Some(&parent));
        assert_eq!(result.keyword(), Some("inline"));
    }

    #[test]
    fn revert_layer_acts_as_no_cascaded_value() {
        let parent = parent_keyword("blue");
        let result = apply_defaulting("color", Some(&ident("revert-layer")), Some(&parent));
        assert_eq!(result.keyword(), Some("blue"));
    }
}
