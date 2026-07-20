# Phase 2 抽取计划：muskitty-css-parser 独立 crate

> 创建日期：2026-07-18
> 状态：待用户批准
> 关联文档：[phase2-muskitty-css-tokenizer-extraction.md](file:///d:/Muskitty/.trae/documents/phase2-muskitty-css-tokenizer-extraction.md)（已执行完毕，作为本计划的模板）

## 摘要

按 [muskitty-css-tokenizer 抽取](file:///d:/Muskitty/.trae/documents/phase2-muskitty-css-tokenizer-extraction.md) 完全相同的工程模板，把 `muskitty-css` 的 `src/parser/` 模块剥离为独立 crate `muskitty-css-parser`，建立独立 git 仓库并发布到 crates.io。剥离后 `muskitty-css` 成为 umbrella crate（仅 re-export tokenizer + parser），保持 100% 向后兼容——`muskitty-selectors` 零代码改动，仅 Cargo.toml 版本号 bump。

## 当前状态分析

### 已就位

- **muskitty-css v0.3.0**：parser 模块成熟（§5.2/§5.3/§5.4.3-5.4.10/§5.5.1-5.5.11 完整覆盖），1044 行实现 + 55 unit tests + 7 doctests 全过
- **muskitty-css-tokenizer v0.1.0**：已独立发布到 crates.io（commit `1678de5`，独立 git 仓库 `github.com/muskitty-dev/muskitty-css-tokenizer`）
- **muskitty-css 现有依赖图**：
  ```
  muskitty-css-tokenizer (v0.1.0, 独立)
      ↑ path + version
  muskitty-css (v0.3.0, umbrella)
      ↑ path + version
  muskitty-selectors (v0.1.0)
  ```
- **muskitty-selectors 实际使用**（16 处 import）：
  - `muskitty_css::parser::TokenStream`（核心）
  - `muskitty_css::tokenizer::Token`（已通过 tokenizer shim）
  - `muskitty_css::tokenize()`（lib.rs 顶层函数）

### 缺失

- **muskitty-css-parser 目录不存在**
- **parser 模块仍耦合在 muskitty-css 内部**：`use crate::tokenizer::Token` 等跨模块引用需要改为 `use muskitty_css_tokenizer::Token`

## 提议改动

### 目标依赖图

```
muskitty-css-tokenizer (v0.1.0, 独立)
    ↑ path + version
muskitty-css-parser (v0.1.0, 独立) ← 新建
    ↑ path + version
muskitty-css (v0.4.0, umbrella re-export)
    ↑ path + version
muskitty-selectors (v0.1.0, 仅 Cargo.toml 版本号 bump)
```

### 文件清单

**新建**（`crates/muskitty-css-parser/`）：

| 文件 | 内容 | 模板参考 |
|------|------|---------|
| `Cargo.toml` | `name = "muskitty-css-parser"`, v0.1.0, edition 2021, rust-version 1.82, dep `muskitty-css-tokenizer = "0.1.0"`, 独立 `[workspace]` 块 | [muskitty-css-tokenizer/Cargo.toml](file:///d:/Muskitty/crates/muskitty-css-tokenizer/Cargo.toml) |
| `.gitignore` | `target/` + `Cargo.lock` | 同上 |
| `src/lib.rs` | crate root，模块声明 + re-exports + `tokenize()` / `parse_*()` 6 个顶层函数 | 见下方"lib.rs 设计" |
| `src/types.rs` | 搬移自 `muskitty-css/src/parser/types.rs` | — |
| `src/token_stream.rs` | 搬移自 `muskitty-css/src/parser/token_stream.rs` | — |
| `src/algorithms.rs` | 搬移自 `muskitty-css/src/parser/algorithms.rs` | — |
| `src/entry_points.rs` | 搬移自 `muskitty-css/src/parser/entry_points.rs` | — |
| `tests/parser_types.rs` | 搬移自 `muskitty-css/tests/parser_types.rs` | — |
| `tests/token_stream.rs` | 搬移自 `muskitty-css/tests/token_stream.rs` | — |
| `tests/parser_algorithms_cp3.rs` | 搬移 | — |
| `tests/parser_algorithms_cp4.rs` | 搬移 | — |
| `tests/parser_algorithms_cp5.rs` | 搬移 | — |
| `tests/parser_entry_points.rs` | 搬移 | — |
| `.github/workflows/ci.yml` | fmt + check + unit tests + clippy + MSRV 1.82 | [muskitty-css-tokenizer/.github/workflows/ci.yml](file:///d:/Muskitty/crates/muskitty-css-tokenizer/.github/workflows/ci.yml) |
| `.github/workflows/publish.yml` | 幂等 crates.io publish + GitHub Release | [muskitty-css-tokenizer/.github/workflows/publish.yml](file:///d:/Muskitty/crates/muskitty-css-tokenizer/.github/workflows/publish.yml) |
| `README.md` | 状态表、架构、Quick Start、规范引用 | [muskitty-css-tokenizer/README.md](file:///d:/Muskitty/crates/muskitty-css-tokenizer/README.md) |
| `LICENSE` | Apache-2.0（同主仓库） | — |
| `LLM_GENERATION.md` | LLM authorship disclosure | — |

**修改**：

| 文件 | 改动 |
|------|------|
| `.gitignore` | 添加 `crates/muskitty-css-parser/` 行（在 `crates/muskitty-css-tokenizer/` 之后） |
| `Cargo.toml`（workspace 根） | `exclude` 数组添加 `"crates/muskitty-css-parser"` |
| `crates/muskitty-css/Cargo.toml` | version 0.3.0 → 0.4.0；`[dependencies]` 添加 `muskitty-css-parser = { path = "../muskitty-css-parser", version = "0.1.0" }` |
| `crates/muskitty-css/src/lib.rs` | 改为 re-export shim：删除 `pub mod parser;` 内的代码（保留 `pub mod parser;` 但 parser/mod.rs 自己是 shim），把 `tokenize()` / `parse_*()` 6 个函数改为 `pub use muskitty_css_parser::{tokenize, parse_stylesheet, ...};` |
| `crates/muskitty-css/src/parser/mod.rs` | 改为 re-export shim：`pub use muskitty_css_parser::{algorithms::*, entry_points::*, token_stream::*, types::*, ...};` |
| `crates/muskitty-selectors/Cargo.toml` | `muskitty-css` 依赖版本 0.3.0 → 0.4.0（仅版本号，代码零改动） |

**删除**（从 muskitty-css 内部）：

- `crates/muskitty-css/src/parser/types.rs`
- `crates/muskitty-css/src/parser/token_stream.rs`
- `crates/muskitty-css/src/parser/algorithms.rs`
- `crates/muskitty-css/src/parser/entry_points.rs`
- `crates/muskitty-css/tests/parser_types.rs`
- `crates/muskitty-css/tests/token_stream.rs`
- `crates/muskitty-css/tests/parser_algorithms_cp3.rs`
- `crates/muskitty-css/tests/parser_algorithms_cp4.rs`
- `crates/muskitty-css/tests/parser_algorithms_cp5.rs`
- `crates/muskitty-css/tests/parser_entry_points.rs`

## lib.rs 设计

新 crate `muskitty-css-parser/src/lib.rs`：

```rust
//! MusKitty CSS Parser
//!
//! Implements the parsing stage of the CSS Syntax Module Level 3 (§5
//! "Parser Algorithms"). Extracted from muskitty-css as a standalone
//! crate for independent versioning and publication.
//!
//! Re-exports [`Token`] / [`Tokenizer`] / [`CssTokenizer`] from
//! `muskitty-css-tokenizer` so downstream crates (e.g.
//! `muskitty-selectors`) can depend on this single crate to access
//! the full CSS Syntax stack.
//!
//! # References
//!
//! - CSS Syntax Module Level 3: <https://drafts.csswg.org/css-syntax-3/>
//! - Spec source (Markdown): `D:\CSSWG\css-syntax-3\Overview.md`

mod algorithms;
mod entry_points;
mod token_stream;
mod types;

// Re-export from muskitty-css-tokenizer for downstream convenience.
pub use muskitty_css_tokenizer::{
    CssTokenizer, HashType, Numeric, State, Token, Tokenizer,
};

// Re-export parser data structures and algorithms at crate root.
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

/// Tokenize a CSS input string into a vector of tokens.
///
/// (Moved from muskitty-css/src/lib.rs — implements the tokenization
/// stage of CSS Syntax §3.1.)
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tz = CssTokenizer::new(input);
    let mut out = Vec::new();
    while let Some(token) = tz.next_token() {
        if matches!(token, Token::Eof) {
            break;
        }
        out.push(token);
    }
    out
}

/// Parse a CSS string into a [`Stylesheet`] (§5.4.3).
pub fn parse_stylesheet(input: &str) -> Stylesheet {
    parse_a_stylesheet(input)
}

/// Parse a CSS string into a single [`Rule`] (§5.4.6).
pub fn parse_rule(input: &str) -> Option<Rule> {
    parse_a_rule(input)
}

/// Parse a CSS string into a single [`Declaration`] (§5.4.7).
pub fn parse_declaration(input: &str) -> Option<Declaration> {
    parse_a_declaration(input)
}

/// Parse a CSS string into a single [`ComponentValue`] (§5.4.8).
pub fn parse_component_value(input: &str) -> Option<ComponentValue> {
    parse_a_component_value(input)
}

/// Parse a CSS string into a list of [`ComponentValue`] (§5.4.9).
pub fn parse_list_of_component_values(input: &str) -> Vec<ComponentValue> {
    parse_a_list_of_component_values(input)
}

/// Parse a CSS string into a comma-separated list of [`ComponentValue`] (§5.4.10).
pub fn parse_comma_separated_list_of_component_values(input: &str) -> Vec<Vec<ComponentValue>> {
    parse_a_comma_separated_list_of_component_values(input)
}
```

## 源文件搬运规则

| 源 | 目标 | 改动 |
|----|------|------|
| `muskitty-css/src/parser/types.rs` | `muskitty-css-parser/src/types.rs` | `use crate::tokenizer::Token;` → `use muskitty_css_tokenizer::Token;` |
| `muskitty-css/src/parser/token_stream.rs` | `muskitty-css-parser/src/token_stream.rs` | 同上 |
| `muskitty-css/src/parser/algorithms.rs` | `muskitty-css-parser/src/algorithms.rs` | `use super::token_stream::TokenStream;` → `use crate::token_stream::TokenStream;`<br>`use super::types::{...};` → `use crate::types::{...};`<br>`use crate::tokenizer::Token;` → `use muskitty_css_tokenizer::Token;`<br>内嵌测试中 `use crate::tokenizer::{CssTokenizer, Tokenizer};` → `use muskitty_css_tokenizer::{CssTokenizer, Tokenizer};` |
| `muskitty-css/src/parser/entry_points.rs` | `muskitty-css-parser/src/entry_points.rs` | `use super::algorithms::{...};` → `use crate::algorithms::{...};`<br>`use super::token_stream::TokenStream;` → `use crate::token_stream::TokenStream;`<br>`use super::types::{...};` → `use crate::types::{...};`<br>`use crate::tokenizer::{CssTokenizer, Token, Tokenizer};` → `use muskitty_css_tokenizer::{CssTokenizer, Token, Tokenizer};` |
| `muskitty-css/tests/*.rs` (6 个文件) | `muskitty-css-parser/tests/*.rs` | `use muskitty_css::parser::{...};` → `use muskitty_css_parser::{...};`<br>`use muskitty_css::tokenizer::Token;` → `use muskitty_css_parser::Token;`<br>`use muskitty_css::{parse_stylesheet, ...};` → `use muskitty_css_parser::{parse_stylesheet, ...};` |

## 执行步骤

### Step 0：先把 `crates/muskitty-css-parser/` 加到 `.gitignore`

**目的**：避免新 crate 目录被主仓库 git 追踪（与 muskitty-css-tokenizer 抽取时同样的预防措施，坑已踩过）。

```powershell
# 编辑 d:\Muskitty\.gitignore
# 在 "crates/muskitty-css-tokenizer/" 行后添加：
#   crates/muskitty-css-parser/
```

最终 `.gitignore` 内容：
```
/target
crates/*/target
*.lock
.zcode/

crates/muskitty-html5-parser/
crates/muskitty-html5-tokenizer/
crates/muskitty-dom/
crates/muskitty-css-tokenizer/
crates/muskitty-css-parser/
```

然后 commit：
```powershell
git add .gitignore
git commit -m "[chore] gitignore: exclude muskitty-css-parser submodule path"
```

### Step 1：创建 `muskitty-css-parser/` 骨架

按"文件清单"创建以下文件（按顺序）：

1. `Cargo.toml`（含 `[workspace]` 空数组，防止父 workspace 吸入）
2. `.gitignore`
3. `src/lib.rs`（按上方"lib.rs 设计"）
4. `src/types.rs`（搬移 + 改 import）
5. `src/token_stream.rs`（搬移 + 改 import）
6. `src/algorithms.rs`（搬移 + 改 import）
7. `src/entry_points.rs`（搬移 + 改 import）
8. `tests/*.rs`（6 个测试文件，搬移 + 改 import）

**验证本地编译**：
```powershell
cd d:\Muskitty\crates\muskitty-css-parser
cargo check
cargo test --lib
cargo test
cargo clippy --all-targets -- -D warnings
```

预期：55 unit + 7 doctests 全过（与原 muskitty-css 一致）。

### Step 2：在新 crate 内 `git init` + initial commit

```powershell
cd d:\Muskitty\crates\muskitty-css-parser
git init
git branch -M main
git add .
git commit -m "[css-parser] initial commit: extract parser from muskitty-css"
```

**质量门**（提交前必跑）：
```powershell
cargo fmt -- --check
cargo test
cargo clippy --all-targets -- -D warnings
```

### Step 3：把 `muskitty-css/src/parser/mod.rs` 改写为 re-export 壳，删除原 4 个 src 文件 + 6 个 test 文件

新 `muskitty-css/src/parser/mod.rs` 内容：

```rust
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
```

同时改写 `muskitty-css/src/lib.rs`，把 6 个顶层函数改为 re-export：

```rust
//! MusKitty CSS (umbrella)
//!
//! Re-exports the tokenizer (`muskitty-css-tokenizer`) and parser
//! (`muskitty-css-parser`) crates. Downstream crates can either depend
//! on this umbrella for the full CSS Syntax stack, or on the individual
//! sub-crates for finer-grained dependencies.

pub mod parser;
pub mod tokenizer;

// Re-export the top-level convenience functions from muskitty-css-parser.
pub use muskitty_css_parser::{
    parse_comma_separated_list_of_component_values, parse_component_value, parse_declaration,
    parse_list_of_component_values, parse_rule, parse_stylesheet, tokenize,
};

// Re-export key parser types at the crate root (backward compat).
pub use parser::{BlockKind, Function, SimpleBlock};
```

**删除原文件**（10 个）：
- `crates/muskitty-css/src/parser/types.rs`
- `crates/muskitty-css/src/parser/token_stream.rs`
- `crates/muskitty-css/src/parser/algorithms.rs`
- `crates/muskitty-css/src/parser/entry_points.rs`
- `crates/muskitty-css/tests/parser_types.rs`
- `crates/muskitty-css/tests/token_stream.rs`
- `crates/muskitty-css/tests/parser_algorithms_cp3.rs`
- `crates/muskitty-css/tests/parser_algorithms_cp4.rs`
- `crates/muskitty-css/tests/parser_algorithms_cp5.rs`
- `crates/muskitty-css/tests/parser_entry_points.rs`

### Step 4：更新 `muskitty-css/Cargo.toml`

```toml
[package]
name = "muskitty-css"
version = "0.4.0"   # bump from 0.3.0
edition = "2021"
description = "CSS Syntax Module Level 3 tokenizer and parser for Rust"
license = "Apache-2.0"
repository = "https://github.com/Ink-dark/MusKitty"
homepage = "https://github.com/Ink-dark/MusKitty"
documentation = "https://docs.rs/muskitty-css"
keywords = ["css", "parser", "syntax", "tokenizer", "web"]
categories = ["parser-implementations", "web-programming"]
rust-version = "1.82"

[dependencies]
muskitty-css-tokenizer = { path = "../muskitty-css-tokenizer", version = "0.1.0" }
muskitty-css-parser = { path = "../muskitty-css-parser", version = "0.1.0" }

[dev-dependencies]
```

### Step 5：更新 `muskitty-selectors/Cargo.toml`

仅版本号 bump，代码零改动（umbrella 提供 100% 向后兼容）：

```toml
[dependencies]
muskitty-css = { path = "../muskitty-css", version = "0.4.0" }   # bump from 0.3.0
```

### Step 6：更新 workspace 根 `Cargo.toml`

添加 `exclude`：

```toml
[workspace]
members = [
    "crates/muskitty-html5-parser",
    "crates/muskitty-dom",
    "crates/muskitty-css",
    "crates/muskitty-selectors",
]
exclude = [
    "crates/muskitty-css-tokenizer",
    "crates/muskitty-css-parser",
]
resolver = "2"
```

### Step 7：主仓库质量门

```powershell
cd d:\Muskitty
cargo fmt --all -- --check
cargo check
cargo test -p muskitty-css         # 0 unit tests (all moved out), but doctests must still pass via re-exports
cargo test -p muskitty-selectors   # 49 tests must still pass
cargo clippy --all-targets -- -D warnings
```

预期：muskitty-selectors 全部 49 个测试通过（代码零改动），muskitty-css 的 doctests 通过 re-export shim 仍然有效。

### Step 8：新独立 crate 质量门

```powershell
cd d:\Muskitty\crates\muskitty-css-parser
cargo fmt -- --check
cargo test           # 55 unit + 7 doctests = 62 tests
cargo clippy --all-targets -- -D warnings
```

### Step 9：提交主仓库拆分 commit

```powershell
cd d:\Muskitty
git add Cargo.toml crates/muskitty-css/Cargo.toml crates/muskitty-css/src/lib.rs crates/muskitty-css/src/parser/mod.rs
git rm crates/muskitty-css/src/parser/types.rs crates/muskitty-css/src/parser/token_stream.rs crates/muskitty-css/src/parser/algorithms.rs crates/muskitty-css/src/parser/entry_points.rs
git rm crates/muskitty-css/tests/parser_types.rs crates/muskitty-css/tests/token_stream.rs crates/muskitty-css/tests/parser_algorithms_cp3.rs crates/muskitty-css/tests/parser_algorithms_cp4.rs crates/muskitty-css/tests/parser_algorithms_cp5.rs crates/muskitty-css/tests/parser_entry_points.rs
git add crates/muskitty-selectors/Cargo.toml
git commit -m "[css] split parser into muskitty-css-parser crate (v0.4.0)" -m "..."
```

### Step 10：初始化 GitHub 远程仓库 + CI/publish workflows

照搬 muskitty-css-tokenizer 的模板：

1. 创建 `.github/workflows/ci.yml`（fmt + check + unit tests + clippy + MSRV 1.82）
2. 创建 `.github/workflows/publish.yml`（幂等 crates.io publish + GitHub Release）
3. 创建 `README.md`（状态表、架构、Quick Start、规范引用）
4. 创建 `LICENSE`（Apache-2.0）
5. 创建 `LLM_GENERATION.md`（LLM authorship disclosure）
6. 在新 crate 内 commit + push 到 `https://github.com/muskitty-dev/muskitty-css-parser`

```powershell
cd d:\Muskitty\crates\muskitty-css-parser
git add .github/ README.md LICENSE LLM_GENERATION.md
git commit -m "[css-parser] add CI/publish workflows + README + LICENSE + LLM_GENERATION"
git remote add origin https://github.com/muskitty-dev/muskitty-css-parser.git
git push -u origin main
```

7. 设置 GitHub secret（用本地 crates.io token）：

```powershell
$cargoToken = (Get-Content "$env:USERPROFILE\.cargo\credentials.toml" | Select-String 'token = "(.+)"').Matches.Groups[1].Value
$cargoToken | gh secret set CARGO_REGISTRY_TOKEN --repo muskitty-dev/muskitty-css-parser
```

### Step 11：打 v0.1.0 tag 触发首次 publish

```powershell
cd d:\Muskitty\crates\muskitty-css-parser
git tag v0.1.0
git push origin v0.1.0
```

监控 publish workflow：
```powershell
Start-Sleep -Seconds 15
gh run list --repo muskitty-dev/muskitty-css-parser --workflow=publish.yml --limit 1
gh run watch <run-id> --repo muskitty-dev/muskitty-css-parser --exit-status
```

验证发布：
```powershell
curl https://crates.io/api/v1/crates/muskitty-css-parser | ConvertFrom-Json | Select-Object -ExpandProperty crate | Select-Object name,newest_version
```

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 主 workspace 报 "multiple workspace roots" 错误 | Step 6 添加 `exclude` 数组（与 css-tokenizer 抽取时同样处理） |
| muskitty-selectors 测试因 path 改变而失败 | umbrella 100% 向后兼容：`muskitty_css::parser::TokenStream` 等 16 处 import 全部通过 shim 继续工作，零代码改动 |
| muskitty-css 自身的 doctests 失败 | doctests 调用 `muskitty_css::tokenize` / `parse_stylesheet` 等，通过 re-export shim 仍然可达 |
| crates.io 上 `muskitty-css-parser` 名字被占用 | 名字独特，预计无冲突；若被占用则加 `-rs` 后缀（极小概率） |
| 新 crate 依赖 `muskitty-css-tokenizer` 但 crates.io 上 v0.1.0 是 yanked 状态 | 等待 2026-07-19 17:51（北京）后 css-tokenizer v0.1.1 重发完成，再发布 css-parser v0.1.0（避免 path 依赖与 crates.io 版本不一致） |
| PowerShell 不支持 heredoc | 使用多个 `-m` flag 传递多段 commit message |

## 顺序依赖

- **Step 0-9** 可以立即执行（不依赖 crates.io 状态）
- **Step 10-11** 推迟到 css-tokenizer v0.1.1 重发完成（2026-07-19 18:00 北京，由 Schedule 任务自动触发）后再执行，避免 publish 时 css-parser 的 `muskitty-css-tokenizer = "0.1.0"` 依赖解析失败

或者：先发布 css-parser v0.1.0 但 Cargo.toml 中 `muskitty-css-tokenizer` 版本号写成 `"0.1.1"`（指向即将重发的版本），这样 publish 在 css-tokenizer v0.1.1 上线后才会成功。但这样会让本地 path 依赖（指向 v0.1.0 的 css-tokenizer 目录）与 Cargo.toml 声明的 v0.1.1 不一致，cargo 会警告。

**推荐方案**：等 css-tokenizer v0.1.1 上线后再执行 Step 10-11。期间可以先把 Step 0-9 做完并 commit 到主仓库。

## 执行完毕后的状态

- `crates/muskitty-css-parser/` 独立 git 仓库（commit 历史与 muskitty-css 主仓库解耦）
- crates.io 上线 `muskitty-css-parser@0.1.0`
- `muskitty-css` v0.4.0（umbrella re-export）
- `muskitty-selectors` 代码零改动，仅 Cargo.toml 版本号 bump
- 主仓库 + 独立 crate 双重质量门通过
- CI/publish workflows 就位

下一步可选：
- 发布 `muskitty-css` v0.4.0 到 crates.io（umbrella 也要发布，因为它依赖外部 crate）
- 继续执行已批准的 Selectors Level 4 计划 SP-6（§15 combinators）
