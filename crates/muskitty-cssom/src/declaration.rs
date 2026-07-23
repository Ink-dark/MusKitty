//! §8.5 CSS Declarations + §8.6 CSS Declaration Blocks。
//!
//! 规范源: `d:\csswg\cssom-1\Overview.md`
//! - §8.5 L2288-2305: A CSS declaration
//! - §8.6 L2306-2476: A CSS declaration block

use crate::ComponentValue;

/// §8.5: A CSS declaration.
///
/// 一个 CSS 声明包含属性名、值（component value 列表）和
/// `!important` 标志。
#[derive(Debug, Clone)]
pub struct CssDeclaration {
    /// 属性名（如 "color"、"font-size"）。
    pub name: String,
    /// §8.5 L2298: 值，以 component value 列表表示。
    pub value: Vec<ComponentValue>,
    /// 声明是否带 `!important` 标志。
    pub important: bool,
}

impl CssDeclaration {
    /// 创建一个新声明。
    pub fn new(name: impl Into<String>, value: Vec<ComponentValue>, important: bool) -> Self {
        Self {
            name: name.into(),
            value,
            important,
        }
    }
}

/// §8.6: A CSS declaration block.
///
/// CSS 声明的有序集合。§8.6 L2320 定义了 `readonly` 标志；本实现
/// 仅设置该标志（mutation API 未实现，Cascade 只读）。
#[derive(Debug, Clone, Default)]
pub struct CssStyleDeclaration {
    /// §8.6 L2323: CSS 声明列表，按出现顺序排列。
    pub declarations: Vec<CssDeclaration>,
    /// §8.6 L2320: 是否只读。
    pub readonly: bool,
}

impl CssStyleDeclaration {
    /// 创建一个空的声明块。
    pub fn new() -> Self {
        Self::default()
    }

    /// 声明数量。
    pub fn len(&self) -> usize {
        self.declarations.len()
    }

    /// 声明块是否为空。
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    /// 遍历声明。
    pub fn iter(&self) -> std::slice::Iter<'_, CssDeclaration> {
        self.declarations.iter()
    }

    /// 可变遍历声明。
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, CssDeclaration> {
        self.declarations.iter_mut()
    }

    /// §8.6 级联语义：返回**最后一个**匹配 name 的声明
    /// （CSS cascade 取最后出现的同名声明）。
    pub fn get_property(&self, name: &str) -> Option<&CssDeclaration> {
        self.declarations.iter().rev().find(|d| d.name == name)
    }

    /// 返回最后一个匹配 name 的声明的值切片。
    pub fn get_property_value(&self, name: &str) -> Option<&[ComponentValue]> {
        self.get_property(name).map(|d| d.value.as_slice())
    }

    /// 追加一个声明到末尾。
    pub fn push(&mut self, decl: CssDeclaration) {
        self.declarations.push(decl);
    }
}

impl CssStyleDeclaration {
    /// 从 `Vec<CssDeclaration>` 构造声明块（便捷方法）。
    pub fn from_vec(declarations: Vec<CssDeclaration>) -> Self {
        Self {
            declarations,
            readonly: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_new_basic() {
        let d = CssDeclaration::new("color", Vec::new(), false);
        assert_eq!(d.name, "color");
        assert!(d.value.is_empty());
        assert!(!d.important);
    }

    #[test]
    fn declaration_new_with_important() {
        let d = CssDeclaration::new("color", Vec::new(), true);
        assert!(d.important);
    }

    #[test]
    fn style_declaration_default_empty() {
        let block = CssStyleDeclaration::default();
        assert!(block.is_empty());
        assert_eq!(block.len(), 0);
        assert!(!block.readonly);
    }

    #[test]
    fn style_declaration_push_and_len() {
        let mut block = CssStyleDeclaration::new();
        block.push(CssDeclaration::new("a", Vec::new(), false));
        block.push(CssDeclaration::new("b", Vec::new(), false));
        assert_eq!(block.len(), 2);
        assert!(!block.is_empty());
    }

    #[test]
    fn style_declaration_get_property_returns_last_match() {
        let mut block = CssStyleDeclaration::new();
        block.push(CssDeclaration::new("color", Vec::new(), false));
        block.push(CssDeclaration::new("font-size", Vec::new(), false));
        // 同名声明后出现的应该胜出
        block.push(CssDeclaration::new("color", Vec::new(), true));

        let got = block.get_property("color").unwrap();
        assert!(got.important);
    }

    #[test]
    fn style_declaration_get_property_missing_returns_none() {
        let block = CssStyleDeclaration::new();
        assert!(block.get_property("color").is_none());
        assert!(block.get_property_value("color").is_none());
    }

    #[test]
    fn style_declaration_iter() {
        let mut block = CssStyleDeclaration::new();
        block.push(CssDeclaration::new("a", Vec::new(), false));
        block.push(CssDeclaration::new("b", Vec::new(), false));
        let names: Vec<_> = block.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn style_declaration_from_vec() {
        let block = CssStyleDeclaration::from_vec(vec![
            CssDeclaration::new("a", Vec::new(), false),
            CssDeclaration::new("b", Vec::new(), false),
        ]);
        assert_eq!(block.len(), 2);
        assert!(!block.readonly);
    }

    #[test]
    fn style_declaration_iter_mut_can_modify() {
        let mut block = CssStyleDeclaration::new();
        block.push(CssDeclaration::new("a", Vec::new(), false));
        for d in block.iter_mut() {
            d.important = true;
        }
        assert!(block.get_property("a").unwrap().important);
    }

    #[test]
    fn style_declaration_clone_preserves_state() {
        let mut block = CssStyleDeclaration::new();
        block.push(CssDeclaration::new("a", Vec::new(), false));
        block.readonly = true;
        let cloned = block.clone();
        assert_eq!(cloned.len(), 1);
        assert!(cloned.readonly);
    }
}
