//! MusKitty CSS (umbrella)
//!
//! Re-exports the tokenizer (`muskitty-css-tokenizer`) and parser
//! (`muskitty-css-parser`) crates. Downstream crates can either depend
//! on this umbrella for the full CSS Syntax stack, or on the individual
//! sub-crates for finer-grained dependencies.
//!
//! # Architecture
//!
//! 1. **Tokenization** ([`tokenizer`]) — re-exported from
//!    `muskitty-css-tokenizer` (§4.3, fully implemented).
//! 2. **Parsing** ([`parser`]) — re-exported from `muskitty-css-parser`
//!    (§5, fully implemented).
//!
//! # Top-level API
//!
//! - [`parse_stylesheet`] — full stylesheet parse (§5.4.3)
//! - [`parse_rule`] — single rule parse (§5.4.6)
//! - [`parse_declaration`] — single declaration parse (§5.4.7)
//! - [`parse_component_value`] — single component value (§5.4.8)
//! - [`parse_list_of_component_values`] — list of component values (§5.4.9)
//! - [`parse_comma_separated_list_of_component_values`] — comma-separated list (§5.4.10)
//! - [`tokenize`] — token stream only (§4.3)
//!
//! # References
//!
//! - CSS Syntax Module Level 3: <https://drafts.csswg.org/css-syntax-3/>
//! - Spec source (Markdown): `D:\CSSWG\css-syntax-3\Overview.md`

pub mod parser;
pub mod tokenizer;

// Re-export the top-level convenience functions from muskitty-css-parser.
pub use muskitty_css_parser::{
    parse_comma_separated_list_of_component_values, parse_component_value, parse_declaration,
    parse_list_of_component_values, parse_rule, parse_stylesheet, tokenize,
};

// Re-export key parser types at the crate root (backward compat).
pub use parser::{BlockKind, Function, SimpleBlock};
