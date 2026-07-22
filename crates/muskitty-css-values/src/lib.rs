//! MusKitty CSS Values — CSS Values Level 4 typed value parsing.
//!
//! 实现 CSS Values Level 4 的类型化值解析：数值（length/angle/time/
//! frequency/resolution/ratio/number/integer）、文本类型（keyword/ident/
//! string/url）、数学函数 AST（calc/min/max/clamp）、var() 语法解析。
//!
//! # 设计原则
//!
//! **解析与求值分离**：本 crate 只构建类型化 AST，不做数值计算和
//! var() 替换求值（留到 Cascade 阶段）。
//!
//! # 规范依据
//!
//! - CSS Values Level 4: `d:\csswg\css-values-4\Overview.md`
//! - CSS Variables Level 1: `d:\csswg\css-variables-1\Overview.md`

pub mod math;
pub mod numeric;
pub mod textual;
pub mod var;
