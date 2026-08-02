# muskitty-cascade

[![crates.io](https://img.shields.io/crates/v/muskitty-cascade.svg)](https://crates.io/crates/muskitty-cascade)
[![Documentation](https://docs.rs/muskitty-cascade/badge.svg)](https://docs.rs/muskitty-cascade)
[![License](https://img.shields.io/crates/l/muskitty-cascade.svg)](https://github.com/muskitty-dev/muskitty-cascade/blob/main/LICENSE)

CSS Cascade Level 5 engine — from DOM tree + multi-origin CSSStyleSheet list
to per-element per-property computed values. Implements the core pipeline of
[CSS Cascade Level 5](https://drafts.csswg.org/css-cascade-5/): filtering →
cascade sorting → defaulting → computed value resolution.

Part of the [MusKitty](https://github.com/muskitty-dev) browser engine project.

## Status

| Component | Spec | Tests |
|-----------|------|-------|
| DeclaredValue + ComputedValue + ComputedStyle | §4.1 / §4.4 | — |
| §5 Filtering (selector matching + inline style) | §5 | 10 |
| §6.1 Cascade sorting (7 criteria) | §6.1 | 18 |
| §7 Defaulting (initial / inherit / unset) | §7 | 10 |
| §4.4 Computed value resolution (relative units) | §4.4 | 8 |
| Property registry (built-in properties) | §4.5 | 5 |
| End-to-end integration | §4-§7 | 20 |
| **Total** | | **71** |

- Zero `unsafe` code
- Zero C/C++ dependencies
- Rust stable toolchain only
- MSRV 1.82

## Pipeline

```text
DOM tree + CssStyleSheet[] (with origin metadata)
    │  §5 Filtering
    ▼
DeclaredValue[] (per element per property, unordered)
    │  §6.1 Cascade sorting (7 criteria)
    ▼
DeclaredValue[] (ordered, descending)
    │  take first → §4.2 Cascaded Value
    ▼
    │  §4.3 + §7 Defaulting (initial / inherit / unset)
    ▼
SpecifiedValue
    │  §4.4 Computed Value (relative unit resolution, var() evaluation)
    ▼
ComputedValue
```

## Usage

```toml
[dependencies]
muskitty-cascade = "0.1"
```

```rust
use muskitty_cascade::collect_declared_values;
```

## License

Apache-2.0, consistent with all MusKitty crates.
