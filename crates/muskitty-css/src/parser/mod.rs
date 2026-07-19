//! CSS Syntax parser — re-export shim.
//!
//! The parser implementation has been extracted into the standalone
//! `muskitty-css-parser` crate (independent git repository, published
//! to crates.io). This module re-exports the public API so that existing
//! `crate::parser::*` references inside `muskitty-css` (and downstream
//! crates using `muskitty_css::parser::*`) continue to resolve without
//! modification.
//!
//! See `crates/muskitty-css-parser/` for the implementation.
//! Spec: CSS Syntax Module Level 3 §5 "Parser Algorithms".

pub use muskitty_css_parser::{
    algorithms::{
        consume_a_block, consume_a_blocks_contents, consume_a_component_value,
        consume_a_declaration, consume_a_function, consume_a_list_of_component_values,
        consume_a_qualified_rule, consume_a_simple_block, consume_a_stylesheets_contents,
        consume_a_unicode_range_value, consume_an_at_rule,
        consume_the_remnants_of_a_bad_declaration, BlockContents,
    },
    entry_points::{
        parse_a_blocks_contents, parse_a_comma_separated_list_of_component_values,
        parse_a_component_value, parse_a_declaration, parse_a_list_of_component_values,
        parse_a_rule, parse_a_stylesheet, parse_a_stylesheets_contents,
    },
    token_stream::TokenStream,
    types::{
        AtRule, BlockKind, ComponentValue, Declaration, Function, ParseError, QualifiedRule, Rule,
        SimpleBlock, Stylesheet,
    },
};
