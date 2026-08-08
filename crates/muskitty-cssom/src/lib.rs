//! MusKitty CSSOM — CSS Object Model 数据结构层。
//!
//! 将 css-parser 的语法层结构（`Stylesheet`/`Rule`/`AtRule`/
//! `QualifiedRule`/`Declaration`，CSS Syntax §5.2）映射到 CSSOM
//! 语义层结构（`CssStyleSheet`/`CssStyleRule`/`CssMediaRule`/
//! `CssStyleDeclaration` 等，CSSOM §8）。
//!
//! # 设计
//!
//! **单向转换**：语法→语义是 one-way 的。转换后 CSSOM 树独立存在，
//! 不反向引用 css-parser 的 `Stylesheet`，避免生命周期耦合。
//!
//! **枚举分发**：`CssRule` 用 Rust enum 而非 trait 对象，符合值语义，
//! pattern matching 清晰，避免 `Rc<RefCell<>>` 所有权复杂度。
//!
//! # 规范依据
//!
//! - CSSOM: `d:\csswg\cssom-1\Overview.md`（§3 序列化、§8.1
//!   CSSStyleSheet、§8.4 CSS Rules、§8.5 CSS Declarations、§8.6
//!   CSS Declaration Blocks）
//! - CSS Syntax: `d:\csswg\css-syntax-3\Overview.md`（§5.2 数据结构）
//!
//! # 快速上手
//!
//! ```
//! use muskitty_cssom::{CssDeclaration, CssStyleDeclaration};
//!
//! let mut block = CssStyleDeclaration::new();
//! block.push(CssDeclaration::new("color", Vec::new(), false));
//! assert_eq!(block.len(), 1);
//! ```

pub mod convert;
pub mod declaration;
pub mod rule;
pub mod serialize;
pub mod stylesheet;

pub use convert::{from_stylesheet, from_stylesheet_with_origin};
pub use declaration::{CssDeclaration, CssStyleDeclaration};
pub use rule::{
    CssContainerRule, CssImportRule, CssLayerBlockRule, CssLayerStatementRule, CssMediaRule,
    CssNamespaceRule, CssRule, CssStyleRule, CssSupportsRule, OtherRule,
};
pub use serialize::{
    serialize_component_value, serialize_component_values, serialize_identifier, serialize_string,
    serialize_url, ToCss,
};
pub use stylesheet::{CssStyleSheet, Origin};

// 从 css-parser re-export ComponentValue，方便下游使用。
pub use muskitty_css::parser::ComponentValue;
