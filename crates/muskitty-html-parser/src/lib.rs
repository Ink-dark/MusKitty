//! MusKitty HTML Parser
//!
//! Implements the WHATWG HTML parsing algorithm.
//!
//! # Architecture
//!
//! The parser follows the standard two-stage model (§13.2.1):
//! 1. **Tokenization** ([`tokenizer`]) — consumes a stream of code points
//!    and emits tokens.
//! 2. **Tree construction** ([`parser`]) — consumes tokens and builds the DOM.
//!
//! # References
//!
//! - WHATWG HTML Living Standard: <https://html.spec.whatwg.org/multipage/parsing.html>
//! - WPT test suite: <https://github.com/web-platform-tests/wpt/tree/master/html/syntax/parsing>

pub mod dom;
pub mod error;
pub mod parser;
pub mod tokenizer;
