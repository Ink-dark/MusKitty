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
//! - [`token_stream`] — §5.3 "Token Streams" (TokenStream struct +
//!   8 operations).
//! - [`algorithms`] — §5.5 "Parser Algorithms" (CP-3 covers
//!   §5.5.7-§5.5.11; CP-4 covers §5.5.6; CP-5 covers §5.5.1-§5.5.5).
//! - [`entry_points`] — §5.4 "Parser Entry Points" (9 of 10;
//!   §5.4.1 / §5.4.2 grammar hooks deferred).

pub mod algorithms;
pub mod entry_points;
pub mod token_stream;
pub mod types;

pub use algorithms::{
    consume_a_block, consume_a_blocks_contents, consume_a_component_value, consume_a_declaration,
    consume_a_function, consume_a_list_of_component_values, consume_a_qualified_rule,
    consume_a_simple_block, consume_a_stylesheets_contents, consume_a_unicode_range_value,
    consume_an_at_rule, consume_the_remnants_of_a_bad_declaration, BlockContents,
};
pub use entry_points::{
    parse_a_blocks_contents, parse_a_comma_separated_list_of_component_values,
    parse_a_component_value, parse_a_declaration, parse_a_list_of_component_values, parse_a_rule,
    parse_a_stylesheet, parse_a_stylesheets_contents,
};
pub use token_stream::TokenStream;
pub use types::{
    AtRule, BlockKind, ComponentValue, Declaration, Function, ParseError, QualifiedRule, Rule,
    SimpleBlock, Stylesheet,
};
