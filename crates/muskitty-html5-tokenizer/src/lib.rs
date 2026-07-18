//! MusKitty HTML5 Tokenizer
//!
//! Implements the tokenization stage of the WHATWG HTML parsing model
//! (§13.2.5 Tokenization).
//!
//! The tokenizer consumes a stream of Unicode code points and emits
//! tokens (start tags, end tags, comments, characters, DOCTYPEs, EOF).
//! These tokens are consumed by the tree construction stage to build
//! the DOM tree.
//!
//! # References
//!
//! - WHATWG HTML Living Standard: <https://html.spec.whatwg.org/multipage/parsing.html>
//! - WPT test suite: <https://github.com/web-platform-tests/wpt/tree/master/html/syntax/parsing>

mod entities;
mod impls;
mod trait_def;
mod types;

pub use impls::HtmlTokenizer;
pub use trait_def::Tokenizer;
pub use types::{DoctypeToken, State, TagKind, TagToken, Token};
