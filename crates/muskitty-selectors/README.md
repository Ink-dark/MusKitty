# muskitty-selectors

Selectors Level 4 parser and matching engine for Rust.

Part of the [MusKitty](https://github.com/Ink-dark/MusKitty) browser engine.

## Status

| Feature                                | Spec coverage   | Tests |
| ------------------------------------- | --------------- | ----- |
| §3 Data Model                         | ✅ L716-1357     | 6     |
| §5 Elemental selectors                | ✅ L1805-1995    | —     |
| §6 Attribute selectors                | ✅ L1996-2533    | 11    |
| §4 Logical combinations               | ✅ L1358-1804    | 10    |
| §13 Tree-structural pseudo-classes    | ✅ L3792-4359    | 12    |
| §15 Combinators                       | ✅ L4360-4532    | 12    |
| §17 Specificity                       | ✅ L4534-4633    | 22    |
| §18 Matching engine                   | ✅ L4816-5026    | 19+   |

Total: 130+ tests, all passing.

## Architecture

- **Parser** (`src/parser/`) — consumes a token stream produced by
  `muskitty-css::tokenize` and builds selector data structures.
  No DOM dependency.
- **Specificity** (`src/specificity.rs`) — computes the A/B/C
  triplet per §17.
- **Matching** (`src/matching/`) — matches parsed selectors against
  an element tree via the `Element` trait. A reference impl for
  `muskitty_dom::Node` lives in `src/matching/dom_impl.rs`.

## Quick start

```rust
use muskitty_selectors::{parse_a_selector, matches, Specificity};

let list = parse_a_selector("div.foo > span").unwrap();
let spec: Specificity = list.specificity_max();
// (0, 1, 2) — one class + two type selectors.
```

To match against your own element tree, implement the
`muskitty_selectors::Element` trait:

```rust
use muskitty_selectors::{parse_a_selector, matches, Element};

#[derive(Clone)]
struct MyElement { /* ... */ }

impl Element for MyElement {
    fn local_name(&self) -> String { /* ... */ }
    // ... 13 other trait methods
}

let list = parse_a_selector("a:hover").unwrap();
let el = MyElement { /* ... */ };
if matches(&list, &el) {
    // ...
}
```

## Spec references

- Selectors Level 4: <https://drafts.csswg.org/selectors-4/>
- Spec source (Markdown): `D:\CSSWG\selectors-4\Overview.md`

## License

Apache-2.0.
