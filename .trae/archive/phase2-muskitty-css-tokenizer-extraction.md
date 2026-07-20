# Phase 2 — 抽离 `muskitty-css-tokenizer` 独立 crate

> 创建日期：2026-07-18
> 状态：待用户批准
> 参考模板：[muskitty-html5-tokenizer 抽离记录](file:///d:/Muskitty/crates/muskitty-html5-tokenizer/)、[memory/topics.md](file:///c:/Users/Administrator/.trae-cn/memory/projects/-d-Muskitty/20260718/topics.md)

## 摘要

按 [muskitty-html5-tokenizer](file:///d:/Muskitty/crates/muskitty-html5-tokenizer/) 抽离的成熟工程模板，把 [crates/muskitty-css/src/tokenizer/](file:///d:/Muskitty/crates/muskitty-css/src/tokenizer/) 4 个文件抽成独立 git 仓库 `crates/muskitty-css-tokenizer/`，作为 crates.io 上 `muskitty-css-tokenizer` v0.1.0 发布。主仓库 [muskitty-css](file:///d:/Muskitty/crates/muskitty-css/) 通过 path+version 依赖并通过 `pub use` 再导出，**保持 100% 向后兼容**（所有 `muskitty_css::tokenizer::*` 路径无需改动）。

## 一、依赖方向审计（已验证可拆）

### 1.1 当前依赖关系

```
                   ┌──────────────────────────────────────────────────┐
                   │  crates/muskitty-css/src/                         │
                   │  ┌─────────────┐         ┌──────────────────┐    │
muskitty-selectors─┼─►│  parser/    │────────►│  tokenizer/      │    │
  (dev/main)       │  │  (5 files)  │  uses   │  (4 files)       │    │
                   │  │             │ ◄────── │  零对外依赖       │    │
                   │  └─────────────┘         └──────────────────┘    │
                   │       ▲                          ▲                │
                   │       │                          │                │
                   │  lib.rs::tokenize()      lib.rs re-exports        │
                   └───────┼──────────────────────────┼────────────────┘
                           │                          │
                           ▼                          ▼
                   外部使用者（selectors/tests）
```

### 1.2 Grep 审计结果（关键数据）

| 方向 | 引用数 | 文件 |
|---|---|---|
| `tokenizer/` → `crate::parser` | **0** | 无 |
| `parser/` → `crate::tokenizer` | 5 | [types.rs](file:///d:/Muskitty/crates/muskitty-css/src/parser/types.rs#L8)、[token_stream.rs](file:///d:/Muskitty/crates/muskitty-css/src/parser/token_stream.rs#L8)、[entry_points.rs](file:///d:/Muskitty/crates/muskitty-css/src/parser/entry_points.rs#L21)、[algorithms.rs#L14](file:///d:/Muskitty/crates/muskitty-css/src/parser/algorithms.rs#L14) + [#L150](file:///d:/Muskitty/crates/muskitty-css/src/parser/algorithms.rs#L150) |
| `lib.rs` → `crate::tokenizer` | 1 | [lib.rs#L39](file:///d:/Muskitty/crates/muskitty-css/src/lib.rs#L39) |
| `muskitty-selectors/src` → `muskitty_css::tokenizer` | 8 | [types.rs](file:///d:/Muskitty/crates/muskitty-selectors/src/types.rs#L19)、[an_plus_b.rs](file:///d:/Muskitty/crates/muskitty-selectors/src/parser/an_plus_b.rs#L43)、[compound.rs](file:///d:/Muskitty/crates/muskitty-selectors/src/parser/compound.rs#L32)、[complex.rs](file:///d:/Muskitty/crates/muskitty-selectors/src/parser/complex.rs#L36)、[simple.rs](file:///d:/Muskitty/crates/muskitty-selectors/src/parser/simple.rs#L47)、[relative.rs](file:///d:/Muskitty/crates/muskitty-selectors/src/parser/relative.rs#L21)、[list.rs](file:///d:/Muskitty/crates/muskitty-selectors/src/parser/list.rs#L23) |
| `muskitty-selectors/tests` → `muskitty_css::tokenizer` | 1 | [parser_types.rs#L8](file:///d:/Muskitty/crates/muskitty-selectors/tests/parser_types.rs#L8) |
| `muskitty-css/tests` → `muskitty_css::tokenizer` | 9 处 | [token_stream.rs](file:///d:/Muskitty/crates/muskitty-css/tests/token_stream.rs#L6)、[parser_algorithms_cp3/4/5.rs](file:///d:/Muskitty/crates/muskitty-css/tests/)、[parser_entry_points.rs](file:///d:/Muskitty/crates/muskitty-css/tests/parser_entry_points.rs#L11) |

**结论**：tokenizer 是叶子节点（零对外依赖），向上单向被 parser 引用。**结构上完全可拆**。

### 1.3 关键优势：re-export 友好

[muskitty-css/src/lib.rs#L32](file:///d:/Muskitty/crates/muskitty-css/src/lib.rs#L32) 已声明 `pub mod tokenizer;`，所有内部和外部使用都通过 `crate::tokenizer::*` / `muskitty_css::tokenizer::*` 路径访问。**拆分后只需要把 [tokenizer/mod.rs](file:///d:/Muskitty/crates/muskitty-css/src/tokenizer/mod.rs) 的实现替换为 `pub use muskitty_css_tokenizer::*;`，所有路径自动保持可用**。

对比 [muskitty-html5-parser](file:///d:/Muskitty/crates/muskitty-html5-parser/) 当时是直接用 `muskitty_html5_tokenizer::*` 路径（跨 crate），所以抽离后没有任何包装层。muskitty-css 抽离会保留一层 re-export，**对调用方完全透明**。

## 二、抽离步骤（参考 html5-tokenizer 模板）

### Step 0：先做 .gitignore + git 追踪踢除（用户明确要求先做）

**前置原因**：[muskitty-html5-tokenizer 抽离时踩过坑](file:///c:/Users/Administrator/.trae-cn/memory/projects/-d-Muskitty/20260718/topics.md) — `.gitignore` 没有预先排除新 crate 路径，导致主仓库把独立仓库的文件也追踪了，事后需要 `git rm --cached -r` 清理。

**操作顺序**（必须先 .gitignore，再 git rm）：

1. 编辑主仓库 [.gitignore](file:///d:/Muskitty/.gitignore)，追加：
   ```
   # muskitty-css-tokenizer 同样作为独立 git 仓库剥离，
   # 由 crates/muskitty-css-tokenizer/.git 自行追踪。
   crates/muskitty-css-tokenizer/
   ```
2. 提交 `.gitignore` 改动（单个 commit：`[chore] gitignore: exclude muskitty-css-tokenizer submodule path`）。
3. **此时 `crates/muskitty-css-tokenizer/` 目录还不存在，git rm 阶段无需操作**。后续 Step 1+ 创建目录和文件后，因为已在 .gitignore 中，git 不会误追踪。

### Step 1：创建独立 crate 骨架

新建 `crates/muskitty-css-tokenizer/` 目录，文件结构对齐 [muskitty-html5-tokenizer](file:///d:/Muskitty/crates/muskitty-html5-tokenizer/)：

```
crates/muskitty-css-tokenizer/
├── .github/
│   └── workflows/
│       ├── ci.yml         # 模板：html5-tokenizer/ci.yml
│       └── publish.yml    # 模板：html5-tokenizer/publish.yml（幂等跳过已发布版本）
├── src/
│   ├── lib.rs             # crate root，pub mod 声明 + re-exports
│   ├── impls.rs           # 从 muskitty-css/src/tokenizer/impls.rs 搬移
│   ├── trait_def.rs       # 从 muskitty-css/src/tokenizer/trait_def.rs 搬移
│   └── types.rs           # 从 muskitty-css/src/tokenizer/types.rs 搬移
├── tests/
│   └── (空，tokenizer 测试主要在 muskitty-css 侧通过 re-export 跑；后续可选迁入)
├── .gitignore             # target/
├── Cargo.toml
├── LICENSE                # 复制主仓库 LICENSE
└── README.md              # 简短说明
```

#### Cargo.toml 模板

```toml
[package]
name = "muskitty-css-tokenizer"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"   # 与 muskitty-css 一致
license = "Apache-2.0"
description = "CSS Syntax Module Level 3 tokenizer (§4.3) extracted from muskitty-css"
repository = "https://github.com/Ink-dark/MusKitty"
homepage = "https://github.com/Ink-dark/MusKitty"
documentation = "https://docs.rs/muskitty-css-tokenizer"
keywords = ["css", "tokenizer", "syntax", "parser", "web"]
categories = ["parser-implementations", "web-programming"]
authors = ["MusCat / MusKitty Bit-Torch Community"]

[dependencies]

[dev-dependencies]

# 防止被父 workspace (D:\Muskitty\Cargo.toml) 误吸入。
# tokenizer 作为独立 git 仓库剥离，有自己的 workspace 根。
[workspace]
```

#### src/lib.rs 模板

```rust
//! MusKitty CSS Tokenizer
//!
//! Implements the tokenization stage of the CSS Syntax Module Level 3
//! (§4.3 "Tokenizer Algorithms"). Extracted from muskitty-css as a
//! standalone crate.
//!
//! # References
//!
//! - CSS Syntax Module Level 3: <https://drafts.csswg.org/css-syntax-3/>
//! - Spec source (Markdown): `D:\CSSWG\css-syntax-3\Overview.md`

mod impls;
mod trait_def;
mod types;

pub use impls::CssTokenizer;
pub use trait_def::Tokenizer;
pub use types::{HashType, Numeric, State, Token};
```

#### 文件搬移对照

| 源文件 | 目标文件 | 改动 |
|---|---|---|
| `crates/muskitty-css/src/tokenizer/impls.rs` | `crates/muskitty-css-tokenizer/src/impls.rs` | `use super::trait_def::Tokenizer;` → `use crate::trait_def::Tokenizer;`、`use super::types::*;` → `use crate::types::*;` |
| `crates/muskitty-css/src/tokenizer/trait_def.rs` | `crates/muskitty-css-tokenizer/src/trait_def.rs` | `use super::types::{State, Token};` → `use crate::types::{State, Token};` |
| `crates/muskitty-css/src/tokenizer/types.rs` | `crates/muskitty-css-tokenizer/src/types.rs` | 无改动（仅 `use std::fmt;`） |
| `crates/muskitty-css/src/tokenizer/mod.rs` | **不搬移**，原地改写为 re-export 壳 | 见下文 Step 3 |

### Step 2：在 `crates/muskitty-css-tokenizer/` 初始化独立 git 仓库

```powershell
cd crates/muskitty-css-tokenizer
git init
git add .
git commit -m "[css-tokenizer] initial commit: extract tokenizer from muskitty-css"
# 后续可选：git remote add origin <url>; git push -u origin main
```

注意：由于 Step 0 已在主仓库 .gitignore 排除该路径，主仓库 git **不会**看到这些文件，**不会**误追踪。

### Step 3：改写 `muskitty-css/src/tokenizer/mod.rs` 为 re-export 壳

```rust
//! CSS Syntax tokenizer types and trait — re-exported from
//! `muskitty-css-tokenizer`.
//!
//! The tokenizer was extracted into a standalone crate
//! ([`muskitty_css_tokenizer`]) for independent versioning and
//! publication. This module re-exports its public API so that the
//! `muskitty_css::tokenizer::*` paths used by the parser and external
//! callers (e.g. `muskitty-selectors`) continue to work unchanged.

pub use muskitty_css_tokenizer::{
    CssTokenizer, HashType, Numeric, State, Token, Tokenizer,
};
```

**关键点**：
- 删除 `mod impls; mod trait_def; mod types;`（文件已搬走，原文件可删除）。
- 删除 `crates/muskitty-css/src/tokenizer/impls.rs`、`trait_def.rs`、`types.rs` 三个原文件（已搬到新 crate）。
- 保留 `crates/muskitty-css/src/tokenizer/mod.rs`（仅作为 re-export 壳）。

### Step 4：更新 `muskitty-css` Cargo.toml

```toml
[package]
name = "muskitty-css"
version = "0.3.0"   # bump 0.2.0 → 0.3.0（breaking 不算，但新增依赖建议 minor bump）
# ... 其他字段不变
rust-version = "1.82"

[dependencies]
muskitty-css-tokenizer = { path = "../muskitty-css-tokenizer", version = "0.1.0" }

[dev-dependencies]
```

### Step 5：更新 `muskitty-selectors` Cargo.toml

`muskitty-selectors` 当前依赖 `muskitty-css = "0.2.0"`，需要 bump：

```toml
[dependencies]
muskitty-css = { path = "../muskitty-css", version = "0.3.0" }
```

**注意**：`muskitty-selectors` 的源码和测试**不需要任何改动**，因为所有 `muskitty_css::tokenizer::*` 和 `muskitty_css::parser::TokenStream` 路径都通过 re-export 保持可用。

### Step 6：主仓库 workspace Cargo.toml 不变

[Cargo.toml](file:///d:/Muskitty/Cargo.toml) 的 `members` 列表**不需要**包含 `muskitty-css-tokenizer`（独立仓库有自己的 `[workspace]` 声明，已通过 `[workspace]` 空数组阻断父 workspace 吸入）。`muskitty-css` 和 `muskitty-selectors` 仍作为 member，路径解析时 Cargo 会通过 `path = "../muskitty-css-tokenizer"` 找到本地源码。

### Step 7：跑质量门（在主仓库根目录）

```powershell
cargo fmt --all -- --check
cargo test -p muskitty-css
cargo test -p muskitty-selectors
cargo check -p muskitty-css -p muskitty-selectors
cargo clippy -p muskitty-css -p muskitty-selectors --all-targets -- -D warnings
```

**预期**：所有现有测试（muskitty-css 的 6 个 test 文件、muskitty-selectors 的 5 个 test 文件共 49 tests）全部通过，因为路径透明、行为不变。

### Step 8：在 `muskitty-css-tokenizer/` 内跑质量门（独立仓库视角）

```powershell
cd crates/muskitty-css-tokenizer
cargo fmt -- --check
cargo test
cargo check
cargo clippy --all-targets -- -D warnings
```

### Step 9：提交（多个 commit）

| # | Commit message | 位置 |
|---|---|---|
| 1 | `[chore] gitignore: exclude muskitty-css-tokenizer submodule path` | 主仓库 |
| 2 | `[css-tokenizer] initial commit: extract tokenizer from muskitty-css` | 新独立仓库 |
| 3 | `[css] split tokenizer into muskitty-css-tokenizer crate (v0.3.0)` | 主仓库（re-export 壳 + Cargo.toml + 删除原 3 个文件） |
| 4 | `[selectors] bump muskitty-css dependency to 0.3.0` | 主仓库（仅 Cargo.toml） |

**主仓库 commit 3+4 可合并**：因为它们是同一次语义改动（拆分 + 依赖升级），合并后 message：

```
[css] split tokenizer into muskitty-css-tokenizer crate (v0.3.0)

Extracts crates/muskitty-css/src/tokenizer/{impls,trait_def,types}.rs
into the new standalone crate muskitty-css-tokenizer (independently
versioned on crates.io). crates/muskitty-css/src/tokenizer/mod.rs is
now a thin re-export shim, preserving the muskitty_css::tokenizer::*
path for all existing callers (parser internals + muskitty-selectors
src/tests). Zero behavioural change.

- crates/muskitty-css/Cargo.toml: add muskitty-css-tokenizer dep, bump 0.2.0 → 0.3.0.
- crates/muskitty-selectors/Cargo.toml: bump muskitty-css dep 0.2.0 → 0.3.0.
- All 49 muskitty-selectors tests + 6 muskitty-css test files unchanged and passing.

Quality gate: cargo fmt --check + cargo test + cargo check + cargo clippy --all-targets -- -D warnings all pass.
```

### Step 10：发布到 crates.io（可选，本计划范围外）

```powershell
cd crates/muskitty-css-tokenizer
cargo publish  # 发布 v0.1.0
# 后续：在 crates/muskitty-css-tokenizer/ 内打 tag v0.1.0，触发 publish.yml
```

发布顺序：**先** `muskitty-css-tokenizer` v0.1.0，**再** `muskitty-css` v0.3.0（因为后者依赖前者）。crates.io 的幂等 publish.yml workflow 已在 [html5-tokenizer/publish.yml](file:///d:/Muskitty/crates/muskitty-html5-tokenizer/.github/workflows/publish.yml) 验证过，直接复刻即可。

## 三、风险与回滚

### 3.1 风险点（很少）

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 主仓库 .gitignore 未生效，新 crate 文件被主仓库追踪 | 低（Step 0 提前做） | 中（事后 git rm --cached -r 清理） | Step 0 先做 .gitignore，commit 后才创建目录 |
| re-export 路径与原 mod 路径不完全等价 | 低 | 高（编译失败） | Step 3 严格对照 [tokenizer/mod.rs](file:///d:/Muskitty/crates/muskitty-css/src/tokenizer/mod.rs) 原 `pub use` 列表，逐项核对 |
| 独立仓库 `[workspace]` 空数组未阻断父 workspace 吸入 | 低 | 中（构建行为异常） | Cargo.toml 模板已含 `[workspace]` 块 |
| muskitty-selectors 的 `muskitty_css::parser::TokenStream` 路径失效 | 极低 | 高（编译失败） | TokenStream 在 muskitty-css 的 parser/ 下，**不**随 tokenizer 一起搬走，路径不变 |
| 测试 doctest 中 `muskitty_css::tokenizer::Token` 失效 | 极低 | 低（doctest 失败） | re-export 后路径透明，[lib.rs#L57-L66](file:///d:/Muskitty/crates/muskitty-css/src/lib.rs#L57) doctest 不需改 |

### 3.2 回滚方案

如果质量门失败或行为异常：

```powershell
# 主仓库回滚 commit 3（拆分）
git revert <commit-3-hash>
# 恢复被删除的 tokenizer/{impls,trait_def,types}.rs
# 删除 crates/muskitty-css-tokenizer/ 目录
# （.gitignore 中的排除项可保留，未来重试仍需要）
```

独立仓库 `crates/muskitty-css-tokenizer/.git` 不受影响，可保留以备重试。

## 四、不在本计划范围内

- **muskitty-css-tokenizer 的独立测试迁入**：当前 tokenizer 测试都通过 re-export 在 muskitty-css 侧跑，本计划不迁。后续可作为单独 commit 把 tokenizer-specific 的测试（如纯 token 类型测试）搬到独立仓库。
- **muskitty-css 的 parser/ 拆分**：parser 依赖 tokenizer，方向相反，不抽离。
- **muskitty-selectors 抽离**：当前规模小，不抽。
- **crates.io 实际发布**：本计划只到"代码可构建、测试通过、commit 完成"为止。发布是后续独立动作。

## 五、交付清单

完成后应满足：

- [ ] `crates/muskitty-css-tokenizer/` 独立 git 仓库就位，4 个 src 文件 + Cargo.toml + .gitignore + LICENSE + README.md + .github/workflows/{ci,publish}.yml
- [ ] 主仓库 `.gitignore` 排除 `crates/muskitty-css-tokenizer/`
- [ ] `crates/muskitty-css/src/tokenizer/mod.rs` 改为 re-export 壳
- [ ] `crates/muskitty-css/src/tokenizer/{impls,trait_def,types}.rs` 删除
- [ ] `crates/muskitty-css/Cargo.toml` bump 0.2.0 → 0.3.0，加 `muskitty-css-tokenizer` 依赖
- [ ] `crates/muskitty-selectors/Cargo.toml` bump `muskitty-css` 0.2.0 → 0.3.0
- [ ] `cargo fmt --check` + `cargo test`（49 selectors + muskitty-css 全部 tests） + `cargo check` + `cargo clippy --all-targets -- -D warnings` 在主仓库根目录全绿
- [ ] 独立仓库 `crates/muskitty-css-tokenizer/` 内同样全绿
- [ ] 主仓库 commit：`[chore] gitignore` + `[css] split tokenizer into muskitty-css-tokenizer crate (v0.3.0)`
- [ ] 独立仓库 commit：`[css-tokenizer] initial commit: extract tokenizer from muskitty-css`
