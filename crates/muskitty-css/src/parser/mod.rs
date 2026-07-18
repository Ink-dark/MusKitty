//! CSS Syntax parser.
//!
//! Implements the parsing stage of CSS Syntax Module Level 3 §5
//! "Parsing". The parser consumes a token stream produced by the
//! [`crate::tokenizer`] and produces CSS objects: stylesheets,
//! rules, declarations, and component values.
//!
//! # Module layout
//!
//! - [`types`] — §5.2 "CSS Parsing Results" data structures
//!   (Stylesheet, Rule, AtRule, QualifiedRule, Declaration,
//!   ComponentValue, Function, SimpleBlock, BlockKind).
//!
//! Future batches (CP-2 onward) will add the `token_stream` (§5.3),
//! `algorithms` (§5.5), and `entry_points` (§5.4) submodules.

pub mod types;

pub use types::{
    AtRule, BlockKind, ComponentValue, Declaration, Function, QualifiedRule, Rule, SimpleBlock,
    Stylesheet,
};
