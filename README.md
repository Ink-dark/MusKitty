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
| `muskitty-css-values` | CSS Values L4 §4/§5/§6/§8/§9 + CSS Variables §2/§3 | local v0.1.0 | in-tree (📦) |
| `muskitty-cssom` | CSSOM §3/§8.1/§8.4/§8.5/§8.6 | local v0.1.0 | in-tree (📦) |
| `muskitty-cascade` | CSS Cascade L5 §4.1–§4.4/§5/§6.1/§7 | local v0.1.0 | in-tree (📦) |

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
├── Cargo.toml                         # members = [cascade, cssom, css-values]
│                                      # exclude = [7 extracted crates]
├── PROGRESS.md                        # project-wide progress dashboard
├── CLAUDE.md                          # engineering rules / hard constraints
├── README.md                          # this file
├── fetch-crates.ps1                   # Windows: pull standalone crates
├── fetch-crates.sh                    # macOS/Linux: pull standalone crates
├── crates/
│   ├── muskitty-cascade/              # 📦 workspace member (in-tree, tracked)
│   ├── muskitty-cssom/                # 📦 workspace member (in-tree, tracked)
│   ├── muskitty-css-values/           # 📦 workspace member (in-tree, tracked)
│   ├── muskitty-css/                  # 🔗 extracted → muskitty-dev/muskitty-css
│   ├── muskitty-css-parser/           # 🔗 extracted → muskitty-dev/muskitty-css-parser
│   ├── muskitty-css-tokenizer/        # 🔗 extracted → muskitty-dev/muskitty-css-tokenizer
│   ├── muskitty-dom/                  # 🔗 extracted → muskitty-dev/muskitty-dom
│   ├── muskitty-html5-parser/         # 🔗 extracted → muskitty-dev/muskitty-html5-parser
│   ├── muskitty-html5-tokenizer/      # 🔗 extracted → muskitty-dev/muskitty-html5-tokenizer
│   └── muskitty-selectors/            # 🔗 extracted → muskitty-dev/muskitty-selectors
├── docs/
│   ├── spec/                          # source specs
│   └── archive/                       # historical design docs / review reports
└── .trae/archive/                     # archived phase plans
```

📦 = workspace member, tracked in this repo.  
🔗 = extracted as independent repo (gitignored here); use `fetch-crates.ps1` to pull.

The 3 workspace members depend on the extracted crates via `path = "..."`. The
extracted crates are listed in `Cargo.toml → exclude` (not `members`) and are
each their own `[workspace]` root.

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

The 3 workspace members (`muskitty-cascade`, `muskitty-cssom`,
`muskitty-css-values`) depend on crates that are **not** tracked in this repo
because each has been extracted to its own repository under
[`muskitty-dev`](https://github.com/muskitty-dev). A fresh clone will be
missing those directories — run the fetch script first.

### One-time setup

```bash
git clone https://github.com/Ink-dark/MusKitty.git
cd MusKitty

# Pull all 7 standalone dependency crates into crates/
pwsh ./fetch-crates.ps1 clone    # Windows
# or
./fetch-crates.sh clone           # macOS / Linux
```

### Day-to-day

```bash
# Update all standalone crates to latest
pwsh ./fetch-crates.ps1           # pull mode (default)

# Workspace-wide checks — all 10 crates
cargo check --workspace
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Each standalone crate can also be built individually inside `crates/<name>/`.

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
