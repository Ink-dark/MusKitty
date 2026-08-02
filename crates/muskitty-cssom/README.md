# muskitty-cssom

[English](README.md) | [简体中文](README.zh-CN.md)

[![crates.io](https://img.shields.io/crates/v/muskitty-cssom.svg)](https://crates.io/crates/muskitty-cssom)
[![Documentation](https://docs.rs/muskitty-cssom/badge.svg)](https://docs.rs/muskitty-cssom)
[![License](https://img.shields.io/crates/l/muskitty-cssom.svg)](https://github.com/muskitty-dev/muskitty-cssom/blob/main/LICENSE)

CSS Object Model (CSSOM) data structures and serialization, implementing
[CSSOM Level 1](https://drafts.csswg.org/cssom-1/) on top of
[`muskitty-css-parser`](https://crates.io/crates/muskitty-css-parser).

Part of the [MusKitty](https://github.com/muskitty-dev) browser engine project.

## Status

| Component | Spec | Tests |
|-----------|------|-------|
| CssStyleDeclaration + CssDeclaration | §8.5 / §8.6 | 10 |
| CssRule enum (9 variants) | §8.4 | 13 |
| CssStyleSheet container | §8.1 | 5 |
| Parser → CSSOM conversion | §8.4 / §8.6 | 20 |
| Serialization (ToCss trait) | §3 / §8.4-§8.6 | 19+ |
| **Total** | | **81** |

- Zero `unsafe` code
- Zero C/C++ dependencies
- One-way conversion: parser `Stylesheet` → CSSOM `CssStyleSheet`
- Rust stable toolchain only
- MSRV 1.82

## Installation

```toml
[dependencies]
muskitty-cssom = "0.1.0"
```

## Quick Start

```rust
use muskitty_cssom::{from_stylesheet, CssStyleSheet};
use muskitty_css::parse_stylesheet;

let parsed = parse_stylesheet("body { color: red; }");
let sheet: CssStyleSheet = from_stylesheet(&parsed);
```

## Architecture

```
muskitty-cssom/
  src/
    lib.rs              Public API + re-exports
    stylesheet.rs       CssStyleSheet + Origin (§8.1)
    rule.rs             CssRule enum + 9 rule types (§8.4)
    declaration.rs      CssStyleDeclaration / CssDeclaration (§8.5 / §8.6)
    convert.rs          Parser Stylesheet → CSSOM CssStyleSheet
    serialize.rs        ToCss trait + serialization (§3 / §8.4-§8.6)
  tests/
    2 test files, 81 tests total
```

## Design Principles

1. **One-way conversion** — Parser output flows into CSSOM; CSSOM is
   independent and does not reference parser types.
2. **Enum over trait objects** — CSSOM rule types use an enum for value
   semantics and clear pattern matching.
3. **CSSWG is ground truth** — Implementation follows the spec exactly.
4. **Zero unsafe** — Pure safe Rust.

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

Copyright 2026 MusCat / MusKitty Bit-Torch Community
