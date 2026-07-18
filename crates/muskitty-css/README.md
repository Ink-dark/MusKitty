# muskitty-css

A from-scratch implementation of the [CSS Syntax Module Level 3][spec]
tokenizer and parser in safe Rust. Part of the [MusKitty][workspace]
browser-engine project.

[spec]: https://drafts.csswg.org/css-syntax-3/
[workspace]: https://github.com/Ink-dark/MusKitty

## Status

| Stage | Spec coverage | Tests |
|-------|---------------|-------|
| Tokenizer (§4.3) | §4.3.1–§4.3.13 complete | unit + html5lib-style fixture tests |
| Parser (§5) | §5.2, §5.3, §5.4.3–§5.4.10, §5.5.1–§5.5.11 complete | 55 unit tests + 7 doctests |

Deferred to later phases:

- §5.4.1 / §5.4.2 grammar-based hooks (require Selectors / Values
  grammar knowledge).
- `consume_a_declaration`'s `original_text` capture for custom
  properties (§5.5.6 L2693-2698 — requires source-text tracking in
  `TokenStream`).
- `unicode-range` descriptor re-tokenization (§5.5.6 L2707-2712).

## Install

```toml
[dependencies]
muskitty-css = "0.2.0"
```

MSRV: Rust 1.82+ (uses `Option::is_none_or` and other recently-stabilized stdlib APIs).

## Quick start

```rust
use muskitty_css::{parse_stylesheet, tokenize};
use muskitty_css::parser::Rule;
use muskitty_css::tokenizer::Token;

// Tokenize only.
let tokens = tokenize("color: red;");
assert!(matches!(tokens[0], Token::Ident(_)));

// Parse a full stylesheet.
let ss = parse_stylesheet("a { color: red; } @media print { p { font: 16px } }");
assert_eq!(ss.rules.len(), 2);

// Inspect the first rule's prelude.
if let Rule::QualifiedRule(qr) = &ss.rules[0] {
    println!("selector had {} component values", qr.prelude.len());
    println!("rule has {} declarations", qr.declarations.len());
}
```

Other top-level entry points: [`parse_rule`](crate::parse_rule),
[`parse_declaration`](crate::parse_declaration),
[`parse_component_value`](crate::parse_component_value),
[`parse_list_of_component_values`](crate::parse_list_of_component_values),
[`parse_comma_separated_list_of_component_values`](crate::parse_comma_separated_list_of_component_values).

## Architecture

The crate follows the two-stage model of CSS Syntax §3.1:

1. **Tokenization** (`tokenizer` module) — consumes a stream of Unicode
   code points (after §5.3 preprocessing) and emits `Token`s per the
   §4.3 "Tokenizer Algorithms": `consume_a_token` (§4.3.1) dispatches
   to sub-algorithms such as `consume_an_ident_like_token` (§4.3.4),
   `consume_a_numeric_token` (§4.3.3), `consume_a_string_token`
   (§4.3.5), and `consume_a_url_token` (§4.3.6).
2. **Parsing** (`parser` module) — consumes the token stream and
   produces CSS objects (Stylesheet, Rule, Declaration,
   ComponentValue) per §5.
   - `parser::types` — §5.2 data structures.
   - `parser::token_stream` — §5.3 `TokenStream` with mark/restore for
     backtracking.
   - `parser::algorithms` — §5.5 the 11 parser algorithms.
   - `parser::entry_points` — §5.4 the 9 entry points (§5.4.1 / §5.4.2
     grammar hooks deferred).

## Spec source

Authoritative spec source for this implementation:
`D:\CSSWG\css-syntax-3\Overview.md` (Markdown rendering of the editor's
draft). All line numbers in code comments reference this file.

Public spec: <https://drafts.csswg.org/css-syntax-3/>

## License

Apache-2.0, same as the workspace root.
