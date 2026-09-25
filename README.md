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
| `muskitty-html5-tokenizer` | WHATWG HTML §13.2.5.1–§13.2.5.85 (85/85 states) | v0.1.4 | [muskitty-dev/muskitty-html5-tokenizer](https://github.com/muskitty-dev/muskitty-html5-tokenizer) |
| `muskitty-html5-parser` | WHATWG HTML §13.2.6 (all insertion modes + AAA / foster parenting / foreign content) | v0.2.3 | [muskitty-dev/muskitty-html5-parser](https://github.com/muskitty-dev/muskitty-html5-parser) |
| `muskitty-dom` | DOM Living Standard §4–§7 (incl. Events) | v0.2.1 | [muskitty-dev/muskitty-dom](https://github.com/muskitty-dev/muskitty-dom) |
| `muskitty-css-tokenizer` | CSS Syntax §4.3.1–§4.3.13 (+ span tracking, `Numeric::has_sign` in 0.3.0) | v0.2.1 | [muskitty-dev/muskitty-css-tokenizer](https://github.com/muskitty-dev/muskitty-css-tokenizer) |
| `muskitty-css-parser` | CSS Syntax §5.2–§5.5 + §5.4.1/§5.4.2 grammar hooks | v0.3.1 | [muskitty-dev/muskitty-css-parser](https://github.com/muskitty-dev/muskitty-css-parser) |
| `muskitty-css` | Facade combining tokenizer + parser | v0.6.0 | [muskitty-dev/muskitty-css](https://github.com/muskitty-dev/muskitty-css) |
| `muskitty-selectors` | Selectors Level 4 §3/§4/§5/§6/§13/§14/§15/§17/§18 (+ Shadow L1 pseudo-elements; WPT parsing 99.8%) | v0.2.1 | [muskitty-dev/muskitty-selectors](https://github.com/muskitty-dev/muskitty-selectors) |
| `muskitty-css-values` | CSS Values L4 §4/§5/§6/§8/§9 + CSS Variables §2/§3 | v0.1.1 | [muskitty-dev/muskitty-css-values](https://github.com/muskitty-dev/muskitty-css-values) |
| `muskitty-cssom` | CSSOM §3/§8.1/§8.4/§8.5/§8.6 | v0.1.1 | [muskitty-dev/muskitty-cssom](https://github.com/muskitty-dev/muskitty-cssom) |
| `muskitty-cascade` | CSS Cascade L5 §4.1–§4.4/§5/§6.1/§7 (+ sheet-level disabled/media gating) | v0.1.1 | [muskitty-dev/muskitty-cascade](https://github.com/muskitty-dev/muskitty-cascade) |
| `muskitty-layout` | CSS Display L3 §2 + Box Model L3 §2/§3 + Flexbox L1 §4-§8 + taffy 0.12 | v0.1.1 | [muskitty-dev/muskitty-layout](https://github.com/muskitty-dev/muskitty-layout) |
| `muskitty-network` | NetworkFetcher trait abstraction + reqwest (rustls) backend + URL primitives (`resolve` / file↔path / subresource scheme policy / `data:`); top-level document GET and stylesheet subresources wired to the browser chrome | local v0.1.0 | in this repo (workspace member) |
| `muskitty-chrome` | Self-drawn browser chrome (tab strip / toolbar / address bar) + winit/softbuffer windowing + navigation + stylesheet sources (document-order `<style>`/`<link>`, `@import`, minimal UA stylesheet, file hot reload) | local v0.1.0 | in this repo (workspace member) |

Test status (latest CI): each independent repo runs 6 jobs (Check / Unit
Tests / Integration Tests / Format / Clippy / MSRV 1.82). See PROGRESS.md for
the per-crate test matrix.

### What is intentionally out of scope

- Rendering / compositing / GPU integration
- JavaScript engine (no V8, no Blink)
- Browser UI beyond the self-drawn chrome (extensions, DevTools, IME)
- Networking beyond top-level document GET + stylesheet subresources (the
  network layer covers plain `http(s)` document fetches and `<link
  rel=stylesheet>` / `@import` / `data:` CSS fetching today; CORS / cookies /
  cache and full WHATWG Fetch semantics are future work)

The project has completed Phase 2 (CSS parsing layer: tokenizer, parser,
selectors, values, CSSOM, and cascade), Phase 3 (Layout: taffy 0.12
integration with CSS Cascade + DOM), and Phase 4 (Renderer: DOM → CSS →
Layout → Render to PNG/window, including text rendering via cosmic-text).
Position/overflow/grid layout, HiDPI scaling, input dispatch, headless
rendering, and multi-tab window management are all in. Recent work
completed the stylesheet-sources track (CS-1, 2026-09-14): document-order
collection of `<style>`/`<link rel=stylesheet>`, relative-URL resolution via
the new `muskitty-network::url` module (WHATWG URL reference implementation),
external fetch with scheme policy / size-and-count limits / dedup,
load-time `@import` expansion, per-sheet `disabled`/`media` gating in
cascade, a minimal UA stylesheet (HTML §15 Rendering), and file hot reload
that watches external CSS. The selectors layer sits at 99.8% (507/508) on
the WPT parsing suite. Layer 5 (Network) provides a `NetworkFetcher` trait
abstraction + reqwest backend and is wired to the browser chrome: the
address bar navigates `http(s)` and `file` URLs (top-level document GET) and
subresource stylesheets are fetched inside the navigation thread — see
[PROGRESS.md](PROGRESS.md) for the layer roadmap and the native HTTP stack
plan.

## Repository layout

```
MusKitty/                              # this repo — workspace coordinator
├── Cargo.toml                         # members = [renderer, network, chrome], exclude = [11 extracted crates]
├── PROGRESS.md                        # project-wide progress dashboard
├── CLAUDE.md / AGENTS.md              # engineering rules / hard constraints
├── README.md                          # this file
├── fetch-crates.ps1                   # Windows: pull standalone crates (reads crates.json)
├── fetch-crates.sh                    # macOS/Linux: pull standalone crates (reads crates.json)
├── crates.json                        # single source of truth: standalone + bundled crate lists
├── crates/
│   ├── muskitty-renderer/             # 📦 workspace member (tiny-skia backend)
│   ├── muskitty-network/              # 📦 workspace member (NetworkFetcher trait + reqwest)
│   ├── muskitty-chrome/               # 📦 workspace member (self-drawn chrome + winit window)
│   ├── muskitty-cascade/              # 🔗 extracted → muskitty-dev/muskitty-cascade
│   ├── muskitty-layout/               # 🔗 extracted → muskitty-dev/muskitty-layout
│   ├── muskitty-css/                  # 🔗 extracted → muskitty-dev/muskitty-css
│   ├── muskitty-css-parser/           # 🔗 extracted → muskitty-dev/muskitty-css-parser
│   ├── muskitty-css-tokenizer/        # 🔗 extracted → muskitty-dev/muskitty-css-tokenizer
│   ├── muskitty-css-values/           # 🔗 extracted → muskitty-dev/muskitty-css-values
│   ├── muskitty-cssom/               # 🔗 extracted → muskitty-dev/muskitty-cssom
│   ├── muskitty-dom/                  # 🔗 extracted → muskitty-dev/muskitty-dom
│   ├── muskitty-html5-parser/         # 🔗 extracted → muskitty-dev/muskitty-html5-parser
│   ├── muskitty-html5-tokenizer/      # 🔗 extracted → muskitty-dev/muskitty-html5-tokenizer
│   └── muskitty-selectors/            # 🔗 extracted → muskitty-dev/muskitty-selectors
├── docs/
│   ├── spec/                          # source specs
│   ├── plans/                         # current phase plan documents
│   ├── decisions/                     # architecture decision records (ADR)
│   └── archive/                       # historical design docs / review reports
└── .trae/archive/                     # archived phase plans
```

📦 = workspace member, tracked in this repo.
🔗 = extracted as independent repo (gitignored here); use `fetch-crates.ps1` to pull.

The 3 workspace members (`muskitty-renderer`, `muskitty-network`,
`muskitty-chrome`) depend on the extracted crates via `path = "..."`. The
extracted crates are listed in `Cargo.toml → exclude` (not `members`) and are
each their own `[workspace]` root.

## Using the published crates

Each crate can be consumed independently. The common case is to depend on a
facade crate (e.g. `muskitty-css`, `muskitty-selectors`) and let it pull in
the lower-level pieces.

```toml
[dependencies]
muskitty-css = "0.6"
muskitty-selectors = "0.2"
muskitty-html5-parser = "0.2"
muskitty-dom = "0.2"
```

MSRV: Rust 1.82+ (see each crate's `Cargo.toml`; `muskitty-layout` declares 1.85).

## Building locally

The workspace members (`muskitty-renderer`, `muskitty-network`,
`muskitty-chrome`) depend on crates that are **not** tracked in this repo
because each has been extracted to its own repository under
[`muskitty-dev`](https://github.com/muskitty-dev). A fresh clone will be
missing those directories — run the fetch script first.

### One-time setup

```bash
git clone https://github.com/Ink-dark/MusKitty.git
cd MusKitty

# Pull all standalone dependency crates into crates/ (list = crates.json)
pwsh ./fetch-crates.ps1          # Windows: pull mode (default); clones any missing crate
# or
./fetch-crates.sh clone           # macOS / Linux: explicit clone subcommand
```

`crates.json` is the single source of truth for which crates are standalone
(fetched from `muskitty-dev`) and which are bundled workspace members; both
scripts read it and cross-check it against `crates/`, `Cargo.toml` members,
and the remote org at the end of every run.

### Day-to-day

```bash
# Update all standalone crates to latest
pwsh ./fetch-crates.ps1           # pull mode (default)

# Workspace-wide checks
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

The MSRV floor across the workspace members is Rust 1.82 (`muskitty-layout`
declares 1.85); see each crate's `Cargo.toml` `rust-version` for the exact
value.

See [CLAUDE.md](CLAUDE.md) for the full hard-constraint list and
[PROGRESS.md](PROGRESS.md) for detailed progress per layer.

## License

Apache-2.0, consistent with all published crates.

## About the name

**MusCat** (口罩猫, "mask cat") is the eventual browser product.
**MusKitty** is its in-development core: the smart kitten inside the mask,
listening for instructions. The project is unrelated to any other cat-themed
project or brand.
