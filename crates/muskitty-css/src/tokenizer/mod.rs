//! CSS Syntax tokenizer — re-export shim.
//!
//! The tokenizer implementation has been extracted into the standalone
//! `muskitty-css-tokenizer` crate (independent git repository, published
//! to crates.io). This module re-exports the public API so that existing
//! `crate::tokenizer::*` references inside `muskitty-css` continue to
//! resolve without modification.
//!
//! See `crates/muskitty-css-tokenizer/` for the implementation.
//! Spec: CSS Syntax Module Level 3 §4.3 "Tokenizer Algorithms".

pub use muskitty_css_tokenizer::{CssTokenizer, HashType, Numeric, State, Token, Tokenizer};
