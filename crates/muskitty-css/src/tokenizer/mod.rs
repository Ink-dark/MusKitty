//! CSS Syntax tokenizer types and trait.
//!
//! Implements the tokenization stage of the CSS Syntax Module Level 3
//! (§4.3 "Tokenizer Algorithms").
//!
//! The tokenizer consumes a stream of Unicode code points (after §5.3
//! preprocessing) and emits [`Token`]s. These tokens are consumed by the
//! (future) tree construction stage to build the CSSOM.

mod impls;
mod trait_def;
mod types;

pub use impls::CssTokenizer;
pub use trait_def::Tokenizer;
pub use types::{HashType, Numeric, State, Token};
