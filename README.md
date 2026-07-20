# MusKitty

From-scratch browser core modules in safe Rust. Independent implementations of
WHATWG and CSSWG specifications — not a Chromium fork, not a binding around an
existing engine. Each module is published as an independent crate on crates.io
and developed in its own git repository under the
[`muskitty-dev`](https://github.com/muskitty-dev) organization. This repository
is the workspace coordinator and project-level documentation hub.

## Project status

| Crate | Spec coverage | crates.io | Repo |
|-------|---------------|-----------|------|
| `muskitty-html5-tokenizer` | WHATWG HTML §13.2.5.1–§13.2.5.80 (80/80 states) | v0.1.2 | [muskitty-dev/muskitty-html5-tokenizer](https://github.com/muskitty-dev/muskitty-html5-tokenizer) |
| `muskitty-html5-parser` | WHATWG HTML §13.2.6 (all insertion modes + AAA / foster parenting / foreign content) | v0.1.2 | [muskitty-dev/muskitty-html5-parser](https://github.com/muskitty-dev/muskitty-html5-parser) |
| `muskitty-dom` | DOM Living Standard §4–§7 | v0.1.0 | [muskitty-dev/muskitty-dom](https://github.com/muskitty-dev/muskitty-dom) |
| `muskitty-css-tokenizer` | CSS Syntax §4.3.1–§4.3.13 | v0.1.1 | [muskitty-dev/muskitty-css-tokenizer](https://github.com/muskitty-dev/muskitty-css-tokenizer) |
| `muskitty-css-parser` | CSS Syntax §5.2–§5.5 + §5.4.1/§5.4.2 grammar hooks | v0.1.0 | [muskitty-dev/muskitty-css-parser](https://github.com/muskitty-dev/muskitty-css-parser) |
| `muskitty-css` | Facade combining tokenizer + parser | v0.4.0 | [muskitty-dev/muskitty-css](https://github.com/muskitty-dev/muskitty-css) |
| `muskitty-selectors` | Selectors Level 4 §3/§4/§5/§6/§13/§14/§15/§17/§18 | v0.1.0 | [muskitty-dev/muskitty-selectors](https://github.com/muskitty-dev/muskitty-selectors) |

Test status (latest CI): each independent repo runs 6 jobs (Check / Unit
Tests / Integration Tests / Format / Clippy / MSRV 1.82). See PROGRESS.md for
the per-crate test matrix.

### What is intentionally out of scope

- Rendering / compositing / GPU integration
- JavaScript engine (no V8, no Blink)
- Browser UI / Chrome / extensions
- Networking stack (deferred to a later layer)

The project is currently in Phase 2 (CSS parsing layer). Layer 3 (Layout),
Layer 4 (Renderer), and Layer 5 (Network) are future work — see
[PROGRESS.md](PROGRESS.md) for the layer roadmap.

## Repository layout

```
MusKitty/                              # this repo — workspace coordinator
├── Cargo.toml                         # members = [], exclude = [7 published crates]
├── PROGRESS.md                        # project-wide progress dashboard
├── CLAUDE.md                          # engineering rules / hard constraints
├── README.md                          # this file
├── crates/                            # each subdir is an independent git repo
│   ├── muskitty-dom/                  # → muskitty-dev/muskitty-dom
│   ├── muskitty-html5-tokenizer/      # → muskitty-dev/muskitty-html5-tokenizer
│   ├── muskitty-html5-parser/         # → muskitty-dev/muskitty-html5-parser
│   ├── muskitty-css-tokenizer/        # → muskitty-dev/muskitty-css-tokenizer
│   ├── muskitty-css-parser/           # → muskitty-dev/muskitty-css-parser
│   ├── muskitty-css/                  # → muskitty-dev/muskitty-css
│   └── muskitty-selectors/            # → muskitty-dev/muskitty-selectors
├── docs/
│   ├── spec/                          # source specs (css-syntax-3 Overview.bs)
│   └── archive/                       # historical design docs / review reports
└── .trae/archive/                     # archived phase plans
```

Each `crates/*` subdirectory has its own `.git` and remote. The top-level
`Cargo.toml` lists them in `exclude` (not `members`) so they are not picked up
by the parent workspace — each crate carries its own `[workspace]` block.

## Using the published crates

Each crate can be consumed independently. The common case is to depend on a
facade crate (e.g. `muskitty-css`, `muskitty-selectors`) and let it pull in
the lower-level pieces.

```toml
[dependencies]
muskitty-css = "0.4"
muskitty-selectors = "0.1"
muskitty-html5-parser = "0.1"
muskitty-dom = "0.1"
```

MSRV: Rust 1.82+ across all crates.

## Building locally

Because each crate is its own git repo, clone the crates you need side-by-side
so `path = "../..."` dependencies resolve:

```bash
git clone https://github.com/muskitty-dev/muskitty-css.git
git clone https://github.com/muskitty-dev/muskitty-css-parser.git
git clone https://github.com/muskitty-dev/muskitty-css-tokenizer.git
```

Or clone the coordinator repo (this one) and the sub-repos into `crates/`.
Each crate's README has its own build / test instructions.

## Engineering conventions

- **Ground truth**: WHATWG / CSSWG specs. WPT and html5lib test suites are
  consumed as conformance checks, but if a test diverges from the current
  spec, the spec wins.
- **Safety**: stable Rust, zero `unsafe` outside FFI boundaries (none
  currently), zero C/C++ dependencies.
- **Coverage**: each module ships its own unit + integration tests; public
  APIs have doc comments citing the spec section.
- **History**: linear, rebase-only. Each sub-task is one commit formatted as
  `[module] what + why` (e.g. `[tokenizer] add Data state, §13.2.5.1`).
- **Extraction**: a crate is split into its own git repo once it reaches spec
  coverage parity with the next layer's entry threshold, then published to
  crates.io via a tag-triggered GitHub Actions workflow.

See [CLAUDE.md](CLAUDE.md) for the full hard-constraint list and
[PROGRESS.md](PROGRESS.md) for detailed progress per layer.

## License

Apache-2.0, consistent with all published crates.

## About the name

**MusCat** (口罩猫, "mask cat") is the eventual browser product.
**MusKitty** is its in-development core: the smart kitten inside the mask,
listening for instructions. The project is unrelated to any other cat-themed
project or brand.
