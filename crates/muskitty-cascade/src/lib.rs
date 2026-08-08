//! MusKitty Cascade — CSS Cascade Level 5 引擎。
//!
//! 实现 CSS Cascade Level 5 的核心算法：从 DOM 树 + 多源
//! CSSStyleSheet 列表 → 每元素每属性的 computed value。
//!
//! # 数据流
//!
//! ```text
//! DOM 树 + CssStyleSheet[] (带 origin 元数据)
//!     │  §5 Filtering
//!     ▼
//! DeclaredValue[] (每元素每属性，无序)
//!     │  §6.1 Cascade 排序
//!     ▼
//! DeclaredValue[] (有序，按 7 准则降序)
//!     │  取首项 → §4.2 Cascaded Value
//!     ▼
//!     │  §4.3 + §7 Defaulting (initial/inherit/unset)
//!     ▼
//! SpecifiedValue
//!     │  §4.4 Computed Value (相对单位解析、var() 求值)
//!     ▼
//! ComputedValue
//! ```
//!
//! # 规范依据
//!
//! - CSS Cascade Level 5: `d:\csswg\css-cascade-5\Overview.md`
//! - CSS Variables Level 1: `d:\csswg\css-variables-1\Overview.md`
//!
//! # 快速上手
//!
//! ```no_run
//! use muskitty_cascade::collect_declared_values;
//! use muskitty_cssom::CssStyleSheet;
//! use muskitty_selectors::matching::DomElement;
//! use muskitty_dom::Node;
//! use std::rc::Rc;
//! use std::cell::RefCell;
//!
//! // let element = DomElement::new(Rc::clone(&node));
//! // let sheets: Vec<CssStyleSheet> = Vec::new();
//! // let declared = collect_declared_values(&element, &sheets);
//! ```

pub mod cascade;
pub mod compute;
pub mod custom_properties;
pub mod defaulting;
pub mod filter;
pub mod origin;
pub mod registry;
pub mod style;
pub mod style_tree;

pub use cascade::{cascade_for_element, cascade_winner};
pub use compute::{compute_value, compute_value_with, ComputeContext, CustomPropertySource};
pub use custom_properties::collect_custom_properties;
pub use defaulting::apply_defaulting;
pub use filter::collect_declared_values;
pub use origin::Origin;
pub use registry::{lookup_property, PropertyDefinition, BUILTIN_PROPERTIES};
pub use style::{ComputedStyle, ComputedValue, DeclaredValue};
pub use style_tree::{compute_styles, StyleTreeOptions};
