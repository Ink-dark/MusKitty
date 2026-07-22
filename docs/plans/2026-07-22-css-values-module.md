# CSS Values Module 实现计划

> **项目**: MusKitty Phase 2 子阶段 3
> **日期**: 2026-07-22
> **状态**: 待 review
> **规范源**: `d:\csswg\css-values-4\Overview.md`、`d:\csswg\css-variables-1\Overview.md`、`d:\csswg\css-syntax-3\Overview.md`

**Goal**: 新建 `muskitty-css-values` crate，实现 CSS Values Level 4 的类型化值解析（数值/文本/数学表达式/var() 语法树），为 Layer 3 Layout 提供强类型 CSS 值。

**Architecture**: 解析与求值分离——本阶段只构建类型化 AST（`Length`/`Angle`/`MathExpression`/`VarReference` 等），不做数值计算和 var() 替换求值（留到子阶段 5 Cascade）。通过 §5.4.1 `Grammar` trait 接入现有 css-parser。前置任务 CV-0 补回 css-parser 的 §5.5.6 `original_text`（var() 延迟替换的前提）。

**Tech Stack**: Rust stable，零 unsafe，零 C/C++ 依赖。依赖 `muskitty-css`（facade）+ `muskitty-css-parser`（grammar hook）。MSRV 1.82。

---

## 范围边界

### 做什么

1. **类型化数值** — `Length`/`Percentage`/`Angle`/`Time`/`Frequency`/`Resolution`/`Ratio`/`Number`/`Integer`，带单位枚举 + §4.4 范围检查 + 单位规范化（如 `1in = 96px`）。
2. **文本类型** — `Keyword`/`CustomIdent`/`DashedIdent`/`String`/`Url`。
3. **数学函数 AST** — `calc()`/`min()`/`max()`/`clamp()` 解析成 `MathExpression` 枚举树，**不求值**。支持常量 `e`/`pi`/`infinity`/`NaN`。
4. **var() 语法解析** — `VarReference { name, fallback }`，支持嵌套 var()（fallback 内可有 var()）。**不求值**。
5. **序列化** — §8.1 functional notation 序列化 + §9.7 calc-serialize 规则。
6. **grammar hook 接入** — 实现 `Grammar` trait，通过 `parse_a_grammar` 入口复用 css-parser。

### 不做什么（明确推迟）

| 项目 | 原因 | 推迟到 |
|------|------|--------|
| calc() 数值计算 | 需要百分比解析依赖（`50%` 的 px 值取决于布局上下文） | 子阶段 5 Cascade |
| min()/max()/clamp() 比较 | 同上 | 子阶段 5 |
| var() 替换求值（§3 的 4 步算法） | 需要元素上下文 + 已计算 custom property 表 + 循环检测 | 子阶段 5 |
| 三角/指数/round/mod/rem/sign/abs | CSS Values 4 新增，布局用不到 | 按需 |
| WPT 集成 | 拆仓后做 | 拆仓后 |

### 关键设计决策

**CV-0 的 TokenStream source-text tracking 方案**：

当前 `TokenStream` 只存 `Vec<Token>`，无 source text。要补 §5.5.6 `original_text`（custom property 的原始 token 文本），需要知道每个 token 在原始 source 里的 byte range。

方案：给 `CssTokenizer` 加 token byte-range 追踪（tokenizer 内部已有 `pos` 字段），新增 `next_token_with_span() -> Option<(Token, std::ops::Range<usize>)>`。`TokenStream` 加平行字段 `token_spans: Vec<Range<usize>>` + `source: Option<String>`，新增 `source_slice(range) -> Option<&str>`。`normalize_from_string` 改用 `next_token_with_span`。

为什么不改 `Token` 本身（不加 `Span`）：避免侵入所有下游 crate（selectors/html5-parser 不需要 span）。span 只在 css-parser 内部使用。

---

## 规范依据（全部本地）

| 规范 | 路径 | 章节 |
|------|------|------|
| CSS Values Level 4 | `d:\csswg\css-values-4\Overview.md` | §2 语法(L22)、§3 textual(L580)、§4 numeric(L1166)、§5 length(L1630)、§6 other(L2460)、§8 functional(L2763)、§9 math(L2856) |
| CSS Variables Level 1 | `d:\csswg\css-variables-1\Overview.md` | §2 custom props(L51)、§3 var()(L450, 算法 L628-660) |
| CSS Syntax Level 3 | `d:\csswg\css-syntax-3\Overview.md` | §5.3 TokenStream、§5.5.6 consume_a_declaration |

---

## 文件结构

### CV-0 改动（现有 crate）

```
crates/muskitty-css-tokenizer/src/
├── trait_def.rs          # 修改：Tokenizer trait 加 span 相关方法
├── impls.rs              # 修改：CssTokenizer 实现 next_token_with_span
└── types.rs              # 不变

crates/muskitty-css-parser/src/
├── token_stream.rs       # 修改：加 source/token_spans 字段 + source_slice 方法
├── entry_points.rs       # 修改：normalize_from_string 用 next_token_with_span
├── algorithms.rs         # 修改：consume_a_declaration 补 original_text
└── lib.rs                # 不变
```

### CV-1~CV-6 新建 crate

```
crates/muskitty-css-values/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs            # 顶层 API + re-export
│   ├── numeric.rs        # CV-1: Length/Percentage/Angle/Time/Frequency/Resolution/Number/Integer
│   ├── textual.rs        # CV-2: Keyword/CustomIdent/DashedIdent/String/Url
│   ├── math.rs           # CV-3: MathExpression AST + calc/min/max/clamp 解析
│   ├── var.rs            # CV-4: VarReference 解析
│   ├── grammar.rs        # CV-5: ValuesGrammar impl Grammar + 类型检查
│   └── serialize.rs      # CV-5: 序列化
└── tests/
    ├── numeric.rs        # CV-1 测试
    ├── textual.rs        # CV-2 测试
    ├── math.rs           # CV-3 测试
    ├── var.rs            # CV-4 测试
    └── integration.rs    # CV-5 端到端测试
```

### 依赖链

```
muskitty-css-tokenizer (v0.2.0, CV-0a)
    └─→ muskitty-css-parser (v0.2.0, CV-0b)
         └─→ muskitty-css (v0.5.0, re-export)
              └─→ muskitty-css-values (v0.1.0, CV-1~CV-6)
```

CV-0 发版后，css-parser/css facade 跟着升 minor 版本（新增 API，向后兼容）。

---

## CV-0a: Tokenizer span 追踪

**改动 crate**: `muskitty-css-tokenizer` → v0.2.0
**规范**: 无（工程基础设施，非规范要求）

### Task 0a-1: Tokenizer trait 加 span 方法

**Files**:
- Modify: `crates/muskitty-css-tokenizer/src/trait_def.rs`

- [ ] **Step 1: 读现有 trait_def.rs，确认 Tokenizer trait 结构**

```bash
# 确认 trait 定义位置
```

- [ ] **Step 2: 给 Tokenizer trait 加 `next_token_with_span` 方法**

在 `trait_def.rs` 的 `Tokenizer` trait 中新增默认方法（基于 `next_token` + 位置追踪）：

```rust
/// 返回下一个 token 及其在原始输入中的 byte range。
///
/// 默认实现基于 `next_token` + `position()`，具体 tokenizer 可覆盖
/// 以获得精确 span（因为 `next_token` 内部会推进 position）。
fn next_token_with_span(&mut self) -> Option<(Token, std::ops::Range<usize>)> {
    let start = self.position();
    let token = self.next_token()?;
    let end = self.position();
    Some((token, start..end))
}

/// 当前 position（byte offset）。默认实现返回 0，具体 tokenizer 覆盖。
fn position(&self) -> usize {
    0
}
```

- [ ] **Step 3: 给 CssTokenizer 实现 `position()`**

在 `impls.rs` 的 `impl Tokenizer for CssTokenizer` 中：

```rust
fn position(&self) -> usize {
    self.pos
}
```

注意：`CssTokenizer` 已有 `pos: usize` 字段（`impls.rs:39`）。`next_token_with_span` 的默认实现会先记 `start = self.pos`，再调 `next_token`（内部推进 `pos`），再记 `end = self.pos`。但 `next_token` 在 reconsume 场景下可能不推进——需验证 `pos` 语义。如果 `pos` 在 `next_token` 返回 `None`（EOF 后）不推进，则 EOF 的 span 是 `pos..pos`（空 range），这是可接受的。

- [ ] **Step 4: 跑测试确认不回归**

```bash
cd crates/muskitty-css-tokenizer
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Expected: 全绿（新增方法是纯增量，不改现有行为）。

- [ ] **Step 5: 写 span 精确性测试**

```rust
// tests/span.rs
use muskitty_css_tokenizer::{CssTokenizer, Tokenizer};

#[test]
fn span_covers_exact_token_bytes() {
    let mut tz = CssTokenizer::new("10px");
    let (token, range) = tz.next_token_with_span().unwrap();
    assert!(matches!(token, Token::Dimension(_, _)));
    assert_eq!(range, 0..3); // "10px"
}

#[test]
fn span_tracks_position_across_tokens() {
    let mut tz = CssTokenizer::new("a b");
    let (_, r1) = tz.next_token_with_span().unwrap(); // "a"
    let (_, r2) = tz.next_token_with_span().unwrap(); // " "
    let (_, r3) = tz.next_token_with_span().unwrap(); // "b"
    assert_eq!(r1, 0..1);
    assert_eq!(r2, 1..2);
    assert_eq!(r3, 2..3);
}

#[test]
fn eof_span_is_empty_at_end() {
    let mut tz = CssTokenizer::new("a");
    tz.next_token_with_span(); // "a"
    let (token, range) = tz.next_token_with_span().unwrap(); // EOF
    assert!(matches!(token, Token::Eof));
    assert_eq!(range, 1..1);
}
```

- [ ] **Step 6: 跑测试通过后 commit**

```bash
git add src/trait_def.rs src/impls.rs tests/span.rs
git commit -m "[tokenizer] add next_token_with_span + position for source-text tracking"
```

### Task 0a-2: 升版本 + 更新 Cargo.toml

**Files**:
- Modify: `crates/muskitty-css-tokenizer/Cargo.toml`

- [ ] **Step 1: version 从 0.1.1 → 0.2.0**（新增 API，minor bump）

- [ ] **Step 2: commit**

```bash
git add Cargo.toml
git commit -m "[tokenizer] bump to v0.2.0 (add span tracking API)"
```

---

## CV-0b: TokenStream source-text tracking + §5.5.6 original_text

**改动 crate**: `muskitty-css-parser` → v0.2.0
**规范**: CSS Syntax §5.3 TokenStream、§5.5.6 consume_a_declaration (L2693-2698)

### Task 0b-1: TokenStream 加 source + token_spans

**Files**:
- Modify: `crates/muskitty-css-parser/src/token_stream.rs`

- [ ] **Step 1: 升级 muskitty-css-tokenizer 依赖到 0.2.0**

`Cargo.toml`:
```toml
muskitty-css-tokenizer = { path = "../muskitty-css-tokenizer", version = "0.2.0" }
```

- [ ] **Step 2: TokenStream 加字段**

```rust
#[derive(Debug, Clone)]
pub struct TokenStream {
    pub tokens: Vec<Token>,
    pub index: usize,
    marked_indexes: Vec<usize>,
    /// 与 `tokens` 平行的 byte ranges。空 Vec 表示无 span 信息
    /// （向后兼容：`new()` 构造的 stream 无 source tracking）。
    pub token_spans: Vec<std::ops::Range<usize>>,
    /// 原始 source text。`None` 表示无 source tracking。
    source: Option<String>,
}
```

- [ ] **Step 3: 新增 `with_source` 构造器**

```rust
impl TokenStream {
    /// 构造一个带 source-text tracking 的 TokenStream。
    ///
    /// tokenize `source` 并记录每个 token 的 byte range，使
    /// [`Self::source_slice`] 能返回原始 source 片段。
    pub fn with_source(source: &str) -> Self {
        let mut tz = muskitty_css_tokenizer::CssTokenizer::new(source);
        let mut tokens = Vec::new();
        let mut spans = Vec::new();
        while let Some((token, range)) = tz.next_token_with_span() {
            let is_eof = matches!(token, Token::Eof);
            tokens.push(token);
            spans.push(range);
            if is_eof {
                break;
            }
        }
        // 防御：确保 EOF 存在（与 new() 一致）
        if tokens.last().is_none_or(|t| !matches!(t, Token::Eof)) {
            tokens.push(Token::Eof);
            spans.push(source.len()..source.len());
        }
        Self {
            tokens,
            index: 0,
            marked_indexes: Vec::new(),
            token_spans: spans,
            source: Some(source.to_string()),
        }
    }

    /// 返回 `tokens[start_index..end_index]` 对应的原始 source text。
    ///
    /// 返回 `None` 如果无 source tracking 或 index 越界。
    pub fn source_slice(&self, start_index: usize, end_index: usize) -> Option<&str> {
        let source = self.source.as_ref()?;
        let start = self.token_spans.get(start_index)?.start;
        let end = self.token_spans.get(end_index.saturating_sub(1))?.end;
        source.get(start..end)
    }
}
```

- [ ] **Step 4: 确保 `new()` 保持兼容**（source=None, token_spans=空）

```rust
pub fn new(mut tokens: Vec<Token>) -> Self {
    if tokens.last().is_none_or(|t| !matches!(t, Token::Eof)) {
        tokens.push(Token::Eof);
    }
    let n = tokens.len();
    Self {
        tokens,
        index: 0,
        marked_indexes: Vec::new(),
        token_spans: vec![],
        source: None,
    }
}
```

注意：`token_spans` 为空 Vec 时 `source_slice` 总返回 `None`，向后兼容。

- [ ] **Step 5: 跑现有测试确认不回归**

```bash
cd crates/muskitty-css-parser
cargo test
```

Expected: 全绿（`new()` 路径不变，现有测试不受影响）。

- [ ] **Step 6: 写 source_slice 测试**

```rust
// tests/token_stream_source.rs
use muskitty_css_parser::TokenStream;

#[test]
fn with_source_tracks_token_ranges() {
    let stream = TokenStream::with_source("color: red");
    // tokens: [Ident("color"), Colon, Whitespace, Ident("red"), Eof]
    // indices: 0, 1, 2, 3, 4
    let slice = stream.source_slice(0, 4).unwrap();
    assert_eq!(slice, "color: red");
}

#[test]
fn source_slice_returns_none_without_source() {
    let stream = TokenStream::new(vec![]);
    assert_eq!(stream.source_slice(0, 1), None);
}

#[test]
fn source_slice_handles_partial_range() {
    let stream = TokenStream::with_source("10px solid");
    // tokens: [Dimension(10,"px"), Whitespace, Ident("solid"), Eof]
    let slice = stream.source_slice(0, 1).unwrap();
    assert_eq!(slice, "10px");
}
```

- [ ] **Step 7: commit**

```bash
git add Cargo.toml src/token_stream.rs tests/token_stream_source.rs
git commit -m "[css-parser] TokenStream::with_source + source_slice for source-text tracking"
```

### Task 0b-2: normalize_from_string 改用 with_source

**Files**:
- Modify: `crates/muskitty-css-parser/src/entry_points.rs`

- [ ] **Step 1: 替换 normalize_from_string 实现**

```rust
/// §5.4 (L1827-1842) Normalize into a token stream.
///
/// 改用 `TokenStream::with_source` 以保留原始 source text，
/// 供 §5.5.6 `original_text`（custom property）使用。
pub(crate) fn normalize_from_string(input: &str) -> TokenStream {
    TokenStream::with_source(input)
}
```

删除原来的手动 tokenize 循环（L30-40），`TokenStream::with_source` 已封装。

- [ ] **Step 2: 跑全量测试**

```bash
cargo test
```

Expected: 全绿（行为不变，只是 TokenStream 现在带 source）。

- [ ] **Step 3: commit**

```bash
git add src/entry_points.rs
git commit -m "[css-parser] normalize_from_string uses TokenStream::with_source"
```

### Task 0b-3: 补回 §5.5.6 original_text

**Files**:
- Modify: `crates/muskitty-css-parser/src/algorithms.rs` (L234-253 区域)

**规范**: CSS Syntax §5.5.6 L2693-2698 — "If the declaration's name is a custom property name, set the declaration's original_text to the concatenation of the values' representations."

- [ ] **Step 1: 写 failing test**

```rust
// tests/original_text.rs
use muskitty_css_parser::parse_declaration;

#[test]
fn custom_property_captures_original_text() {
    let decl = parse_declaration("--foo: 10px solid red").unwrap();
    assert_eq!(decl.name, "--foo");
    // original_text 是 value 部分的原始 source（colon 后、semicolon 前）
    assert_eq!(decl.original_text.as_deref(), Some(" 10px solid red"));
}

#[test]
fn non_custom_property_has_no_original_text() {
    let decl = parse_declaration("color: red").unwrap();
    assert_eq!(decl.original_text, None);
}

#[test]
fn custom_property_with_calc_preserves_original_text() {
    let decl = parse_declaration("--bar: calc(100% - 20px)").unwrap();
    assert_eq!(decl.original_text.as_deref(), Some(" calc(100% - 20px)"));
}
```

- [ ] **Step 2: 跑测试确认 fail**

```bash
cargo test --test original_text
```

Expected: 3 个 fail（`original_text` 当前总是 `None`）。

- [ ] **Step 3: 修改 consume_a_declaration 补 original_text**

在 `algorithms.rs` 的 `consume_a_declaration` 中，Step 8 区域（当前 L243-247 的 TODO 块）：

```rust
// 记录 value 的 token range（Step 5 消费前的 index 到消费后的 index）
// 在 Step 5 之前插入：
let value_start_index = input.index;
// Step 5: consume value
let mut value = consume_a_list_of_component_values(input, Some(Token::Semicolon), nested);
let value_end_index = input.index; // 指向 ; 或 EOF

// ... Step 6-7 不变 ...

let mut decl = Declaration {
    name,
    value,
    important,
    original_text: None,
};

// Step 8 (§5.5.6 L2693-2705):
if is_custom_property_name(&decl.name) {
    // §5.5.6 L2693-2698: set original_text to the source text
    // spanning the value tokens.
    decl.original_text = input.source_slice(value_start_index, value_end_index).map(|s| s.to_string());
} else {
    // §5.5.6 L2700-2705: top-level {}-block validity check
    if has_top_level_curly_block_with_other_values(&decl.value) {
        return None;
    }
}
```

注意：`value_start_index` 是 Step 5 消费**前**的 `input.index`（即 colon + trailing whitespace 之后的第一个 value token）。`value_end_index` 是消费**后**的 `input.index`（指向 `;` 或 EOF）。`source_slice(start, end)` 返回 `[start, end)` 范围 token 对应的 source text。

- [ ] **Step 4: 跑测试确认 pass**

```bash
cargo test --test original_text
cargo test  # 全量回归
```

Expected: 全绿。

- [ ] **Step 5: 删除 algorithms.rs 里的 TODO 注释**（L185-191, L244-246）

- [ ] **Step 6: cargo fmt + clippy**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 7: commit**

```bash
git add src/algorithms.rs tests/original_text.rs
git commit -m "[css-parser] implement §5.5.6 original_text for custom properties"
```

### Task 0b-4: 升版本 + 更新 muskitty-css facade

**Files**:
- Modify: `crates/muskitty-css-parser/Cargo.toml` (version 0.2.0)
- Modify: `crates/muskitty-css/Cargo.toml` (依赖升 0.2.0)
- Modify: `crates/muskitty-css/Cargo.toml` (version 0.5.0)

- [ ] **Step 1: css-parser Cargo.toml version → 0.2.0**
- [ ] **Step 2: css facade Cargo.toml 依赖 muskitty-css-parser = "0.2.0"，自身 version → 0.5.0**
- [ ] **Step 3: 跑 css facade 测试**

```bash
cd crates/muskitty-css
cargo test
```

- [ ] **Step 4: 分别 commit**

```bash
# css-parser
cd crates/muskitty-css-parser
git add Cargo.toml
git commit -m "[css-parser] bump to v0.2.0 (add source-text tracking + original_text)"

# css facade
cd ../muskitty-css
git add Cargo.toml
git commit -m "[css] bump to v0.5.0 (depend on muskitty-css-parser 0.2.0)"
```

---

## CV-1: 数值类型

**新建 crate**: `crates/muskitty-css-values/`
**规范**: CSS Values Level 4 §4 Numeric (L1166)、§5 Length (L1630)、§6 Other Quantities (L2460)

### Task 1-1: crate 骨架 + Cargo.toml

**Files**:
- Create: `crates/muskitty-css-values/Cargo.toml`
- Create: `crates/muskitty-css-values/src/lib.rs`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "muskitty-css-values"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"
license = "Apache-2.0"
description = "CSS Values Level 4 typed value parsing (length/angle/time/calc/var)"
repository = "https://github.com/muskitty-dev/muskitty-css-values"
homepage = "https://github.com/muskitty-dev"
documentation = "https://docs.rs/muskitty-css-values"
keywords = ["css", "values", "length", "calc", "web"]
categories = ["parser-implementations", "web-programming"]
authors = ["MusCat / MusKitty Bit-Torch Community"]

[dependencies]
muskitty-css = { path = "../muskitty-css", version = "0.5.0" }

# 防止被父 workspace 误吸入
[workspace]
```

- [ ] **Step 2: 创建 lib.rs 骨架**

```rust
//! MusKitty CSS Values — CSS Values Level 4 typed value parsing.
//!
//! 实现 CSS Values Level 4 的类型化值解析：数值（length/angle/time/
//! frequency/resolution/ratio/number/integer）、文本类型（keyword/ident/
//! string/url）、数学函数 AST（calc/min/max/clamp）、var() 语法解析。
//!
//! # 设计原则
//!
//! **解析与求值分离**：本 crate 只构建类型化 AST，不做数值计算和
//! var() 替换求值（留到 Cascade 阶段）。
//!
//! # 规范依据
//!
//! - CSS Values Level 4: `d:\csswg\css-values-4\Overview.md`
//! - CSS Variables Level 1: `d:\csswg\css-variables-1\Overview.md`

pub mod numeric;
pub mod textual;
pub mod math;
pub mod var;
pub mod grammar;
pub mod serialize;
```

- [ ] **Step 3: 创建空模块文件**（每个 `pub mod` 对应一个文件，先放占位）

- [ ] **Step 4: cargo check 通过**

- [ ] **Step 5: 加入主仓库 .gitignore + workspace exclude**

修改 `d:\Muskitty\.gitignore`：加入 `crates/muskitty-css-values/`
修改 `d:\Muskitty\Cargo.toml`：`exclude` 列表加 `"crates/muskitty-css-values"`

- [ ] **Step 6: commit**

```bash
cd crates/muskitty-css-values
git init  # 作为独立 git 仓库（按项目 extraction discipline）
git add Cargo.toml src/
git commit -m "[css-values] crate skeleton + Cargo.toml"
```

### Task 1-2: Length 类型

**Files**:
- Create: `crates/muskitty-css-values/src/numeric.rs`
- Test: `crates/muskitty-css-values/tests/numeric.rs`

**规范**: css-values-4 §5 (L1630)，§5.1 Relative Lengths (L1705)，§5.2 Absolute Lengths (L2296)

- [ ] **Step 1: 写 failing test**

```rust
// tests/numeric.rs
use muskitty_css_values::numeric::{Length, LengthUnit};

#[test]
fn parse_px_length() {
    let len = Length::parse("10px").unwrap();
    assert_eq!(len.value, 10.0);
    assert_eq!(len.unit, LengthUnit::Px);
}

#[test]
fn parse_em_length() {
    let len = Length::parse("1.5em").unwrap();
    assert_eq!(len.value, 1.5);
    assert_eq!(len.unit, LengthUnit::Em);
}

#[test]
fn parse_negative_length() {
    let len = Length::parse("-5px").unwrap();
    assert_eq!(len.value, -5.0);
}

#[test]
fn parse_absolute_length_units() {
    assert_eq!(Length::parse("1in").unwrap().unit, LengthUnit::In);
    assert_eq!(Length::parse("1cm").unwrap().unit, LengthUnit::Cm);
    assert_eq!(Length::parse("1mm").unwrap().unit, LengthUnit::Mm);
    assert_eq!(Length::parse("1pt").unwrap().unit, LengthUnit::Pt);
    assert_eq!(Length::parse("1pc").unwrap().unit, LengthUnit::Pc);
    assert_eq!(Length::parse("1Q").unwrap().unit, LengthUnit::Q);
}

#[test]
fn parse_relative_length_units() {
    assert_eq!(Length::parse("1rem").unwrap().unit, LengthUnit::Rem);
    assert_eq!(Length::parse("1ex").unwrap().unit, LengthUnit::Ex);
    assert_eq!(Length::parse("1ch").unwrap().unit, LengthUnit::Ch);
    assert_eq!(Length::parse("1vw").unwrap().unit, LengthUnit::Vw);
    assert_eq!(Length::parse("1vh").unwrap().unit, LengthUnit::Vh);
    assert_eq!(Length::parse("1vmin").unwrap().unit, LengthUnit::Vmin);
    assert_eq!(Length::parse("1vmax").unwrap().unit, LengthUnit::Vmax);
}

#[test]
fn reject_unitless_number_as_length() {
    assert!(Length::parse("10").is_err());
}

#[test]
fn reject_unknown_unit() {
    assert!(Length::parse("10foo").is_err());
}
```

- [ ] **Step 2: 跑测试确认 fail**（类型未定义）

- [ ] **Step 3: 实现 Length**

```rust
// src/numeric.rs
use muskitty_css::{ComponentValue, Token};

/// CSS `<length>` 值 (css-values-4 §5)。
///
/// 一个数值 + 长度单位。本阶段不计算绝对长度（如 em→px），
/// 保留原始值和单位，求值留到 Cascade 阶段。
#[derive(Debug, Clone, PartialEq)]
pub struct Length {
    pub value: f64,
    pub unit: LengthUnit,
}

/// 长度单位 (css-values-4 §5.1 相对、§5.2 绝对)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    // 相对 (§5.1)
    Em, Rem, Ex, Ch, Vw, Vh, Vmin, Vmax,
    // 绝对 (§5.2)
    Px, Cm, Mm, In, Pt, Pc, Q,
}

impl Length {
    /// 从 CSS 字符串解析一个 length 值。
    ///
    /// 接受 `<number><length-unit>` 形式（如 `10px`、`1.5em`）。
    /// 拒绝无单位数字和未知单位。
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        parse_length_from_cvs(&cvs)
    }
}

fn parse_length_from_cvs(cvs: &[ComponentValue]) -> Result<Length, ParseError> {
    // 跳过首尾 whitespace
    let cvs: Vec<_> = cvs
        .iter()
        .filter(|cv| !matches!(cv, ComponentValue::PreservedToken(Token::Whitespace)))
        .collect();
    if cvs.len() != 1 {
        return Err(ParseError::new("expected exactly one length value"));
    }
    match &cvs[0] {
        ComponentValue::PreservedToken(Token::Dimension(numeric, unit)) => {
            let unit = LengthUnit::from_str(unit)
                .ok_or_else(|| ParseError::new(format!("unknown length unit: {unit}")))?;
            Ok(Length {
                value: numeric.value,
                unit,
            })
        }
        _ => Err(ParseError::new("expected a dimension token")),
    }
}

impl LengthUnit {
    /// 从字符串解析长度单位（ASCII case-insensitive）。
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "em" => Some(Self::Em),
            "rem" => Some(Self::Rem),
            "ex" => Some(Self::Ex),
            "ch" => Some(Self::Ch),
            "vw" => Some(Self::Vw),
            "vh" => Some(Self::Vh),
            "vmin" => Some(Self::Vmin),
            "vmax" => Some(Self::Vmax),
            "px" => Some(Self::Px),
            "cm" => Some(Self::Cm),
            "mm" => Some(Self::Mm),
            "in" => Some(Self::In),
            "pt" => Some(Self::Pt),
            "pc" => Some(Self::Pc),
            "q" => Some(Self::Q),
            _ => None,
        }
    }
}

/// 值解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl ParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSS value parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}
```

- [ ] **Step 4: 跑测试确认 pass**

```bash
cargo test --test numeric
```

- [ ] **Step 5: commit**

```bash
git add src/numeric.rs tests/numeric.rs
git commit -m "[css-values] CV-1: Length type with relative/absolute units (css-values-4 §5)"
```

### Task 1-3: Percentage / Number / Integer / 其他数值类型

**Files**:
- Modify: `crates/muskitty-css-values/src/numeric.rs`
- Modify: `crates/muskitty-css-values/tests/numeric.rs`

**规范**: §4.3 Integer (L1252)、§4.4 Number (L1304)、§4.5 Dimension (L1351)、§4.6 Percentage (L1417)、§6.1 Angle (L2465)、§6.2 Time (L2525)、§6.3 Frequency (L2552)、§6.4 Resolution (L2574)、§6.5 Ratio (L1541)

- [ ] **Step 1: 扩展测试**（为每个类型加 parse + unit 测试）

```rust
#[test]
fn parse_percentage() {
    let p = Percentage::parse("50%").unwrap();
    assert_eq!(p.value, 50.0);
}

#[test]
fn parse_number() {
    assert_eq!(Number::parse("42").unwrap().value, 42.0);
    assert_eq!(Number::parse("3.14").unwrap().value, 3.14);
    assert_eq!(Number::parse("-0.5").unwrap().value, -0.5);
}

#[test]
fn parse_integer() {
    assert_eq!(Integer::parse("7").unwrap().value, 7);
    assert!(Integer::parse("3.14").is_err()); // 非整数
}

#[test]
fn parse_angle_units() {
    assert_eq!(Angle::parse("90deg").unwrap().unit, AngleUnit::Deg);
    assert_eq!(Angle::parse("100grad").unwrap().unit, AngleUnit::Grad);
    assert_eq!(Angle::parse("1.5708rad").unwrap().unit, AngleUnit::Rad);
    assert_eq!(Angle::parse("0.25turn").unwrap().unit, AngleUnit::Turn);
}

#[test]
fn parse_time_units() {
    assert_eq!(Time::parse("2s").unwrap().unit, TimeUnit::S);
    assert_eq!(Time::parse("500ms").unwrap().unit, TimeUnit::Ms);
}

#[test]
fn parse_frequency_units() {
    assert_eq!(Frequency::parse("440Hz").unwrap().unit, FrequencyUnit::Hz);
    assert_eq!(Frequency::parse("44kHz").unwrap().unit, FrequencyUnit::KHz);
}

#[test]
fn parse_resolution_units() {
    assert_eq!(Resolution::parse("96dpi").unwrap().unit, ResolutionUnit::Dpi);
    assert_eq!(Resolution::parse("38dpcm").unwrap().unit, ResolutionUnit::Dpcm);
    assert_eq!(Resolution::parse("1dppx").unwrap().unit, ResolutionUnit::Dppx);
}
```

- [ ] **Step 2: 实现各类型**

```rust
/// CSS `<percentage>` 值 (§4.6)。
#[derive(Debug, Clone, PartialEq)]
pub struct Percentage {
    pub value: f64,
}

/// CSS `<number>` 值 (§4.4)。
#[derive(Debug, Clone, PartialEq)]
pub struct Number {
    pub value: f64,
}

/// CSS `<integer>` 值 (§4.3)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integer {
    pub value: i32,
}

/// CSS `<angle>` 值 (§6.1)。
#[derive(Debug, Clone, PartialEq)]
pub struct Angle {
    pub value: f64,
    pub unit: AngleUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AngleUnit { Deg, Grad, Rad, Turn }

/// CSS `<time>` 值 (§6.2)。
#[derive(Debug, Clone, PartialEq)]
pub struct Time {
    pub value: f64,
    pub unit: TimeUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeUnit { S, Ms }

/// CSS `<frequency>` 值 (§6.3)。
#[derive(Debug, Clone, PartialEq)]
pub struct Frequency {
    pub value: f64,
    pub unit: FrequencyUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyUnit { Hz, KHz }

/// CSS `<resolution>` 值 (§6.4)。
#[derive(Debug, Clone, PartialEq)]
pub struct Resolution {
    pub value: f64,
    pub unit: ResolutionUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionUnit { Dpi, Dpcm, Dppx }
```

每个类型实现 `parse(&str) -> Result<Self, ParseError>`，解析逻辑类似 `Length`（从 ComponentValue 列表提取 token）。`Percentage` 匹配 `Token::Percentage`，`Number` 匹配 `Token::Number`，`Integer` 匹配 `Token::Number` 且 `numeric.is_integer`，`Angle`/`Time`/`Frequency`/`Resolution` 匹配 `Token::Dimension` 并按单位枚举。

- [ ] **Step 3: 跑测试 + fmt + clippy + commit**

```bash
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
git add src/numeric.rs tests/numeric.rs
git commit -m "[css-values] CV-1: Percentage/Number/Integer/Angle/Time/Frequency/Resolution types (§4.3-§4.6, §6.1-§6.4)"
```

### Task 1-4: Ratio 类型

**规范**: §4.7 Ratio (L1541) — `<ratio> = <number [0,∞]> [ / <number [0,∞]> ]?`

- [ ] **Step 1: 测试**

```rust
#[test]
fn parse_ratio_single_number() {
    let r = Ratio::parse("16").unwrap();
    assert_eq!(r.width, 16.0);
    assert_eq!(r.height, 1.0); // 单数字时 height 默认 1
}

#[test]
fn parse_ratio_two_numbers() {
    let r = Ratio::parse("16 / 9").unwrap();
    assert_eq!(r.width, 16.0);
    assert_eq!(r.height, 9.0);
}
```

- [ ] **Step 2: 实现**（解析 `/` 分隔的两个 number，第二个可选）

- [ ] **Step 3: 跑测试 + commit**

```bash
git add src/numeric.rs tests/numeric.rs
git commit -m "[css-values] CV-1: Ratio type (css-values-4 §4.7)"
```

---

## CV-2: 文本数据类型

**规范**: css-values-4 §3 Textual Data Types (L580)
- §3.1 Pre-defined Keywords (L620)
- §3.2 custom-ident (L665)
- §3.3 dashed-ident (L702)
- §3.4 string (L790)
- §3.5 url (L842)

### Task 2-1: Keyword / CustomIdent / DashedIdent

**Files**:
- Create: `crates/muskitty-css-values/src/textual.rs`
- Create: `crates/muskitty-css-values/tests/textual.rs`

- [ ] **Step 1: 测试**

```rust
use muskitty_css_values::textual::{Keyword, CustomIdent, DashedIdent};

#[test]
fn parse_keyword() {
    assert_eq!(Keyword::parse("auto").unwrap().value, "auto");
    assert_eq!(Keyword::parse("block").unwrap().value, "block");
}

#[test]
fn parse_custom_ident() {
    let id = CustomIdent::parse("my-anim").unwrap();
    assert_eq!(id.value, "my-anim");
}

#[test]
fn custom_ident_rejects_css_wide_keywords() {
    // §3.2: custom-ident 不能是 initial/inherit/unset/default/none
    assert!(CustomIdent::parse("initial").is_err());
    assert!(CustomIdent::parse("inherit").is_err());
    assert!(CustomIdent::parse("unset").is_err());
    assert!(CustomIdent::parse("none").is_err());
}

#[test]
fn parse_dashed_ident() {
    let id = DashedIdent::parse("--my-var").unwrap();
    assert_eq!(id.value, "--my-var");
}

#[test]
fn dashed_ident_must_start_with_double_dash() {
    assert!(DashedIdent::parse("my-var").is_err());
    assert!(DashedIdent::parse("-my-var").is_err());
}
```

- [ ] **Step 2: 实现**（从 `Token::Ident` / `Token::Hash` 提取，custom-ident 检查 CSS-wide keyword 排除列表）

- [ ] **Step 3: 跑测试 + commit**

```bash
git add src/textual.rs tests/textual.rs
git commit -m "[css-values] CV-2: Keyword/CustomIdent/DashedIdent types (css-values-4 §3.1-§3.3)"
```

### Task 2-2: String / Url

**规范**: §3.4 string (L790)、§3.5 url (L842)

- [ ] **Step 1: 测试**

```rust
use muskitty_css_values::textual::{CssString, Url};

#[test]
fn parse_quoted_string() {
    assert_eq!(CssString::parse("\"hello\"").unwrap().value, "hello");
    assert_eq!(CssString::parse("'world'").unwrap().value, "world");
}

#[test]
fn parse_url_function() {
    let url = Url::parse("url(image.png)").unwrap();
    assert_eq!(url.value, "image.png");
}

#[test]
fn parse_quoted_url() {
    let url = Url::parse("url(\"path/to/img.png\")").unwrap();
    assert_eq!(url.value, "path/to/img.png");
}
```

- [ ] **Step 2: 实现**（从 `Token::String` 和 `Token::Url` / `ComponentValue::Function("url", ...)` 提取）

- [ ] **Step 3: 跑测试 + commit**

```bash
git add src/textual.rs tests/textual.rs
git commit -m "[css-values] CV-2: CssString/Url types (css-values-4 §3.4-§3.5)"
```

---

## CV-3: 数学函数 AST（不求值）

**规范**: css-values-4 §9 Mathematical Expressions (L2856)
- §9.1 calc() (L2883)
- §9.2 min()/max()/clamp() (L3011)
- §9.7 Syntax (L4072)
- §9.8 Type Checking (L4098)
- §9.10 Internal Representation (L4465)

### Task 3-1: MathExpression 枚举 + 常量

**Files**:
- Create: `crates/muskitty-css-values/src/math.rs`
- Create: `crates/muskitty-css-values/tests/math.rs`

- [ ] **Step 1: 定义 AST**

```rust
// src/math.rs
use crate::numeric::{Length, Percentage, Number};
use crate::var::VarReference;

/// CSS 数学表达式 AST (css-values-4 §9)。
///
/// 本阶段只构建 AST，不求值。求值留到 Cascade 阶段
/// （需要百分比解析上下文、var() 替换等）。
#[derive(Debug, Clone, PartialEq)]
pub enum MathExpression {
    /// 数值字面量：`10px`、`50%`、`3.14`
    Length(Length),
    Percentage(Percentage),
    Number(Number),
    /// 数学常量：`e`、`pi`、`infinity`、`NaN` (§9.3)
    Constant(MathConstant),
    /// var() 引用（可嵌套在 calc() 内）
    Var(VarReference),
    /// 一元取负：`-expr` (§9.1)
    Negate(Box<MathExpression>),
    /// 加法：`a + b` (§9.1)
    Sum(Box<MathExpression>, Box<MathExpression>),
    /// 乘法：`a * b` (§9.1)
    Product(Box<MathExpression>, Box<MathExpression>),
    /// 除法：`a / b` (§9.1，b 必须是 number)
    Quotient(Box<MathExpression>, Box<MathExpression>),
    /// min(a, b, ...) (§9.2)
    Min(Vec<MathExpression>),
    /// max(a, b, ...) (§9.2)
    Max(Vec<MathExpression>),
    /// clamp(min, val, max) (§9.2)
    Clamp {
        min: Box<MathExpression>,
        val: Box<MathExpression>,
        max: Box<MathExpression>,
    },
}

/// 数学常量 (css-values-4 §9.3)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathConstant {
    /// e ≈ 2.71828
    E,
    /// π ≈ 3.14159
    Pi,
    /// ∞
    Infinity,
    /// NaN
    NaN,
}
```

- [ ] **Step 2: commit AST 定义**

```bash
git add src/math.rs
git commit -m "[css-values] CV-3: MathExpression AST + MathConstant (css-values-4 §9)"
```

### Task 3-2: calc() 语法解析

**规范**: §9.1 calc() (L2883)、§9.7 Syntax (L4072)

calc() 语法（§9.7 L4080）：`calc( <calc-sum> )`，`<calc-sum> = <calc-product> [ '+' | '-' ] <calc-product>)*`，`<calc-product> = <calc-value> [ '*' | '/' <calc-value> )*`，`<calc-value> = <number> | <dimension> | <percentage> | <calc-constant> | ( <calc-sum> )`

- [ ] **Step 1: 写测试**

```rust
use muskitty_css_values::math::{MathExpression, MathConstant, parse_calc};

#[test]
fn calc_simple_length() {
    let expr = parse_calc("calc(10px)").unwrap();
    assert!(matches!(expr, MathExpression::Length(_)));
}

#[test]
fn calc_sum() {
    let expr = parse_calc("calc(10px + 5px)").unwrap();
    assert!(matches!(expr, MathExpression::Sum(_, _)));
}

#[test]
fn calc_product() {
    let expr = parse_calc("calc(10px * 2)").unwrap();
    assert!(matches!(expr, MathExpression::Product(_, _)));
}

#[test]
fn calc_quotient() {
    let expr = parse_calc("calc(100px / 2)").unwrap();
    assert!(matches!(expr, MathExpression::Quotient(_, _)));
}

#[test]
fn calc_negate() {
    let expr = parse_calc("calc(-10px)").unwrap();
    assert!(matches!(expr, MathExpression::Negate(_)));
}

#[test]
fn calc_nested_parens() {
    let expr = parse_calc("calc((10px + 5px) * 2)").unwrap();
    assert!(matches!(expr, MathExpression::Product(_, _)));
}

#[test]
fn calc_constant_e() {
    let expr = parse_calc("calc(e)").unwrap();
    assert!(matches!(expr, MathExpression::Constant(MathConstant::E)));
}

#[test]
fn calc_complex_expression() {
    let expr = parse_calc("calc(100% - 20px)").unwrap();
    assert!(matches!(expr, MathExpression::Sum(_, _)));
}

#[test]
fn calc_mixed_operations() {
    // calc(10px + 5px * 2) → 乘法优先于加法
    let expr = parse_calc("calc(10px + 5px * 2)").unwrap();
    // 顶层应该是 Sum(10px, Product(5px, 2))
    assert!(matches!(expr, MathExpression::Sum(_, _)));
}

#[test]
fn calc_rejects_empty() {
    assert!(parse_calc("calc()").is_err());
}

#[test]
fn calc_rejects_trailing_operator() {
    assert!(parse_calc("calc(10px +)").is_err());
}
```

- [ ] **Step 2: 实现 calc() 解析器**（递归下降：calc-sum → calc-product → calc-value，处理运算符优先级和括号）

解析器接收 `ComponentValue` 列表（通过 `muskitty_css::parse_list_of_component_values` 获取），用递归下降构建 `MathExpression` 树。关键是：
1. 识别 `calc(...)` 外层 function（名称 ASCII case-insensitive）
2. 内部按 calc-sum → calc-product → calc-value 递归下降
3. `+`/`-` 是低优先级（左结合），`*`/`/` 是高优先级（左结合）
4. 空格在 `+`/`-` 两侧是**必需的**（§9.1 规定），`*`/`/` 两侧可选

- [ ] **Step 3: 跑测试 + fmt + clippy + commit**

```bash
cargo test --test math
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
git add src/math.rs tests/math.rs
git commit -m "[css-values] CV-3: calc() parser with recursive descent (css-values-4 §9.1, §9.7)"
```

### Task 3-3: min()/max()/clamp() 解析

**规范**: §9.2 (L3011)

- [ ] **Step 1: 测试**

```rust
#[test]
fn min_function() {
    let expr = parse_math_function("min(10px, 20px, 5px)").unwrap();
    assert!(matches!(expr, MathExpression::Min(args) if args.len() == 3));
}

#[test]
fn max_function() {
    let expr = parse_math_function("max(10px, 20px)").unwrap();
    assert!(matches!(expr, MathExpression::Max(args) if args.len() == 2));
}

#[test]
fn clamp_three_args() {
    let expr = parse_math_function("clamp(10px, 50px, 100px)").unwrap();
    assert!(matches!(expr, MathExpression::Clamp { .. }));
}

#[test]
fn clamp_rejects_two_args() {
    assert!(parse_math_function("clamp(10px, 50px)").is_err());
}

#[test]
fn min_with_calc_inside() {
    let expr = parse_math_function("min(calc(10px + 5px), 20px)").unwrap();
    assert!(matches!(expr, MathExpression::Min(args) if args.len() == 2));
}
```

- [ ] **Step 2: 实现**（复用 calc-sum 解析器解析每个参数，逗号分隔）

- [ ] **Step 3: 跑测试 + commit**

```bash
git add src/math.rs tests/math.rs
git commit -m "[css-values] CV-3: min()/max()/clamp() parsers (css-values-4 §9.2)"
```

---

## CV-4: var() 语法解析（不求值）

**规范**: css-variables-1 §3 Using Cascading Variables: var() (L450)，替换算法 L628-660

### Task 4-1: VarReference 类型 + 解析

**Files**:
- Create: `crates/muskitty-css-values/src/var.rs`
- Create: `crates/muskitty-css-values/tests/var.rs`

- [ ] **Step 1: 定义 VarReference**

```rust
// src/var.rs
use muskitty_css::ComponentValue;

/// var() 引用的语法解析结果 (css-variables-1 §3)。
///
/// 只解析语法结构，不求值。求值（查 custom property 值、
/// 循环检测、fallback 激活）留到 Cascade 阶段。
#[derive(Debug, Clone, PartialEq)]
pub struct VarReference {
    /// 自定义属性名（如 `--foo`）。
    pub name: String,
    /// 可选的 fallback 值（逗号后的 component values）。
    /// `None` 表示无 fallback；`Some(vec![])` 表示空 fallback
    /// （bare comma，§3 规定合法）。
    pub fallback: Option<Vec<ComponentValue>>,
}
```

- [ ] **Step 2: 测试**

```rust
use muskitty_css_values::var::VarReference;

#[test]
fn var_simple() {
    let v = VarReference::parse("var(--foo)").unwrap();
    assert_eq!(v.name, "--foo");
    assert_eq!(v.fallback, None);
}

#[test]
fn var_with_fallback() {
    let v = VarReference::parse("var(--foo, 10px)").unwrap();
    assert_eq!(v.name, "--foo");
    assert!(v.fallback.is_some());
}

#[test]
fn var_empty_fallback_bare_comma() {
    // §3: bare comma with nothing following is valid (empty fallback)
    let v = VarReference::parse("var(--foo,)").unwrap();
    assert_eq!(v.name, "--foo");
    assert_eq!(v.fallback, Some(vec![]));
}

#[test]
fn var_nested_in_fallback() {
    // fallback 内可嵌套 var()
    let v = VarReference::parse("var(--foo, var(--bar, 10px))").unwrap();
    assert_eq!(v.name, "--foo");
    assert!(v.fallback.is_some());
}

#[test]
fn var_rejects_no_args() {
    assert!(VarReference::parse("var()").is_err());
}

#[test]
fn var_rejects_non_custom_property_name() {
    assert!(VarReference::parse("var(foo)").is_err());
    assert!(VarReference::parse("var(-foo)").is_err()); // 单 dash 不是 custom property
}
```

- [ ] **Step 3: 实现解析**

解析逻辑：
1. 识别 `var(` function（名称 ASCII case-insensitive）
2. 第一个参数必须是 `--<ident>` 形式（custom-property-name，§2 定义）
3. 如果有逗号，逗号后的所有 component values 是 fallback
4. bare comma（逗号后无值）→ `Some(vec![])`

```rust
impl VarReference {
    pub fn parse(input: &str) -> Result<Self, crate::numeric::ParseError> {
        let cvs = muskitty_css::parse_list_of_component_values(input);
        Self::parse_from_cvs(&cvs)
    }

    pub fn parse_from_cvs(cvs: &[ComponentValue]) -> Result<Self, crate::numeric::ParseError> {
        // 找到 var( function
        let func = cvs.iter().find_map(|cv| {
            if let ComponentValue::Function(f) = cv {
                if f.name.eq_ignore_ascii_case("var") {
                    return Some(f);
                }
            }
            None
        }).ok_or_else(|| crate::numeric::ParseError::new("expected var() function"))?;

        let args = &func.value;
        // 过滤首尾 whitespace
        let args: Vec<_> = args.iter()
            .filter(|cv| !matches!(cv, ComponentValue::PreservedToken(muskitty_css::Token::Whitespace)))
            .collect();

        if args.is_empty() {
            return Err(crate::numeric::ParseError::new("var() requires at least one argument"));
        }

        // 第一个参数：custom-property-name (--ident)
        let name = match &args[0] {
            ComponentValue::PreservedToken(muskitty_css::Token::Ident(s)) => {
                if is_custom_property_name(s) {
                    s.clone()
                } else {
                    return Err(crate::numeric::ParseError::new(
                        format!("var() first argument must be a custom property name, got: {s}")
                    ));
                }
            }
            _ => return Err(crate::numeric::ParseError::new(
                "var() first argument must be an ident"
            )),
        };

        // 查找逗号分隔 fallback
        let fallback = if args.len() == 1 {
            None
        } else {
            // args[1] 应该是逗号
            if !matches!(args[1], ComponentValue::PreservedToken(muskitty_css::Token::Comma)) {
                return Err(crate::numeric::ParseError::new(
                    "var() arguments must be separated by comma"
                ));
            }
            // 逗号后的原始 component values（保留原始顺序，包括 whitespace）
            // 需要从 func.value（未过滤 whitespace 的原始列表）中提取
            let comma_idx = func.value.iter().position(|cv| {
                matches!(cv, ComponentValue::PreservedToken(muskitty_css::Token::Comma))
            });
            match comma_idx {
                Some(idx) => Some(func.value[idx + 1..].to_vec()),
                None => Some(vec![]),
            }
        };

        Ok(VarReference { name, fallback })
    }
}

/// 检查字符串是否是 custom-property-name（§2：以 `--` 开头）。
fn is_custom_property_name(s: &str) -> bool {
    s.starts_with("--") && s.len() > 2
}
```

注意：`is_custom_property_name` 的完整定义在 css-syntax §5.5.6，但这里只需检查 `--` 前缀（tokenizer 已保证后续是合法 ident sequence）。

- [ ] **Step 4: 跑测试 + fmt + clippy + commit**

```bash
cargo test --test var
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
git add src/var.rs tests/var.rs
git commit -m "[css-values] CV-4: VarReference parser with fallback + nested var() (css-variables-1 §3)"
```

---

## CV-5: Grammar hook + 序列化

**规范**: css-values-4 §8.1 Serialization of Functional Notations (L2803)、§9.7 calc-serialize、§9.8 Type Checking (L4098)

### Task 5-1: 实现 ValuesGrammar

**Files**:
- Create: `crates/muskitty-css-values/src/grammar.rs`
- Create: `crates/muskitty-css-values/tests/integration.rs`

- [ ] **Step 1: 实现 Grammar trait**

```rust
// src/grammar.rs
use muskitty_css_parser::{ComponentValue, Grammar, ParseError as CssParseError};
use crate::math::MathExpression;
use crate::numeric::{Length, Percentage, Number, ParseError};
use crate::var::VarReference;

/// CSS Values grammar，用于通过 §5.4.1 `parse_a_grammar` 入口
/// 解析类型化值。
///
/// 根据 `ValueKind` 决定解析目标类型。
pub struct ValuesGrammar {
    pub kind: ValueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Length,
    Percentage,
    Number,
    Angle,
    Time,
    Frequency,
    Resolution,
    Calc,
    Var,
    Keyword,
    CustomIdent,
    DashedIdent,
    String,
    Url,
}

impl Grammar for ValuesGrammar {
    type Output = CssValue;

    fn parse(&self, input: &[ComponentValue]) -> Result<Self::Output, CssParseError> {
        match self.kind {
            ValueKind::Length => Length::from_cvs(input)
                .map(CssValue::Length)
                .map_err(|e| CssParseError::new(e.message)),
            // ... 其他 kind 类似
            ValueKind::Calc => MathExpression::from_cvs(input)
                .map(CssValue::Calc)
                .map_err(|e| CssParseError::new(e.message)),
            ValueKind::Var => VarReference::from_cvs(input)
                .map(CssValue::Var)
                .map_err(|e| CssParseError::new(e.message)),
            // ...
        }
    }
}

/// 解析出的类型化 CSS 值。
#[derive(Debug, Clone, PartialEq)]
pub enum CssValue {
    Length(Length),
    Percentage(Percentage),
    Number(Number),
    Angle(crate::numeric::Angle),
    Time(crate::numeric::Time),
    Frequency(crate::numeric::Frequency),
    Resolution(crate::numeric::Resolution),
    Calc(MathExpression),
    Var(VarReference),
    Keyword(crate::textual::Keyword),
    CustomIdent(crate::textual::CustomIdent),
    DashedIdent(crate::textual::DashedIdent),
    String(crate::textual::CssString),
    Url(crate::textual::Url),
}
```

- [ ] **Step 2: 端到端测试**

```rust
// tests/integration.rs
use muskitty_css_parser::parse_a_grammar;
use muskitty_css_values::grammar::{ValuesGrammar, ValueKind, CssValue};

#[test]
fn parse_length_via_grammar() {
    let g = ValuesGrammar { kind: ValueKind::Length };
    let v = parse_a_grammar("10px", &g).unwrap();
    assert!(matches!(v, CssValue::Length(_)));
}

#[test]
fn parse_calc_via_grammar() {
    let g = ValuesGrammar { kind: ValueKind::Calc };
    let v = parse_a_grammar("calc(10px + 5px)", &g).unwrap();
    assert!(matches!(v, CssValue::Calc(_)));
}

#[test]
fn parse_var_via_grammar() {
    let g = ValuesGrammar { kind: ValueKind::Var };
    let v = parse_a_grammar("var(--foo, 10px)", &g).unwrap();
    assert!(matches!(v, CssValue::Var(_)));
}
```

- [ ] **Step 3: 给各类型加 `from_cvs` 方法**（从 ComponentValue 列表解析，复用 `parse` 内部逻辑）

- [ ] **Step 4: 跑测试 + commit**

```bash
cargo test
git add src/grammar.rs tests/integration.rs src/numeric.rs src/math.rs src/var.rs src/textual.rs
git commit -m "[css-values] CV-5: ValuesGrammar impl Grammar trait (§5.4.1 integration)"
```

### Task 5-2: 序列化

**规范**: §8.1 (L2803)、§9.7 calc-serialize

- [ ] **Step 1: 实现 Serialize trait / to_css_string 方法**

```rust
// src/serialize.rs
use crate::numeric::*;
use crate::math::*;
use crate::var::*;
use std::fmt::Write;

/// 序列化为 CSS 字符串（specified value 序列化，§8.1）。
pub trait Serialize {
    fn to_css_string(&self) -> String;
}

impl Serialize for Length {
    fn to_css_string(&self) -> String {
        // §8.1: number 后跟单位，无空格
        format!("{}{}", format_number(self.value), self.unit.to_str())
    }
}

impl Serialize for MathExpression {
    fn to_css_string(&self) -> String {
        match self {
            MathExpression::Length(l) => l.to_css_string(),
            MathExpression::Percentage(p) => format!("{}%", format_number(p.value)),
            MathExpression::Number(n) => format_number(n.value),
            MathExpression::Constant(c) => c.to_str().to_string(),
            MathExpression::Var(v) => v.to_css_string(),
            MathExpression::Negate(e) => format!("-{}", e.to_css_string()),
            MathExpression::Sum(a, b) => format!("{} + {}", a.to_css_string(), b.to_css_string()),
            MathExpression::Product(a, b) => format!("{} * {}", a.to_css_string(), b.to_css_string()),
            MathExpression::Quotient(a, b) => format!("{} / {}", a.to_css_string(), b.to_css_string()),
            MathExpression::Min(args) => format!("min({})", serialize_args(args)),
            MathExpression::Max(args) => format!("max({})", serialize_args(args)),
            MathExpression::Clamp { min, val, max } => {
                format!("clamp({}, {}, {})", min.to_css_string(), val.to_css_string(), max.to_css_string())
            }
        }
    }
}

impl Serialize for VarReference {
    fn to_css_string(&self) -> String {
        match &self.fallback {
            None => format!("var({})", self.name),
            Some(fallback) => {
                // fallback 的 component values 序列化
                let s: String = fallback.iter()
                    .map(|cv| cv_to_string(cv))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("var({}, {})", self.name, s)
            }
        }
    }
}

/// §8.1: 数字序列化——整数无小数点，浮点数保留有效数字。
fn format_number(v: f64) -> String {
    if v == v.trunc() && v.is_finite() {
        format!("{:.0}", v)
    } else {
        format!("{}", v)
    }
}

fn serialize_args(args: &[MathExpression]) -> String {
    args.iter().map(|a| a.to_css_string()).collect::<Vec<_>>().join(", ")
}
```

- [ ] **Step 2: 序列化测试**

```rust
#[test]
fn serialize_length() {
    let l = Length::parse("10px").unwrap();
    assert_eq!(l.to_css_string(), "10px");
}

#[test]
fn serialize_calc_roundtrip() {
    let expr = MathExpression::parse_calc("calc(10px + 5px)").unwrap();
    // §9.7: calc() 序列化时保留 calc() 包裹
    assert_eq!(expr.to_css_string(), "10px + 5px");
}

#[test]
fn serialize_var() {
    let v = VarReference::parse("var(--foo, 10px)").unwrap();
    assert_eq!(v.to_css_string(), "var(--foo, 10px)");
}
```

- [ ] **Step 3: 跑测试 + fmt + clippy + commit**

```bash
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
git add src/serialize.rs tests/integration.rs
git commit -m "[css-values] CV-5: serialization (§8.1 + calc-serialize §9.7)"
```

---

## CV-6: lib 顶层 API + cleanup + 剥离准备

### Task 6-1: lib.rs 顶层 API

**Files**:
- Modify: `crates/muskitty-css-values/src/lib.rs`

- [ ] **Step 1: 实现顶层便捷函数**

```rust
// src/lib.rs 顶层 API

/// 解析一个 `<length>` 值。
pub fn parse_length(input: &str) -> Result<numeric::Length, numeric::ParseError> {
    numeric::Length::parse(input)
}

/// 解析一个 calc() 数学表达式。
pub fn parse_calc(input: &str) -> Result<math::MathExpression, numeric::ParseError> {
    math::parse_calc(input)
}

/// 解析一个 var() 引用。
pub fn parse_var(input: &str) -> Result<var::VarReference, numeric::ParseError> {
    var::VarReference::parse(input)
}

/// 通过 grammar hook 解析任意类型化值。
pub fn parse_value(input: &str, kind: grammar::ValueKind) -> Result<grammar::CssValue, muskitty_css::ParseError> {
    muskitty_css::parse_a_grammar(input, &grammar::ValuesGrammar { kind })
}
```

- [ ] **Step 2: doctest**

```rust
/// ```
/// use muskitty_css_values::parse_length;
/// let len = parse_length("10px").unwrap();
/// assert_eq!(len.value, 10.0);
/// ```
```

- [ ] **Step 3: 跑 doctest + commit**

```bash
cargo test --doc
git add src/lib.rs
git commit -m "[css-values] CV-6: lib top-level API + doctests"
```

### Task 6-2: README + Cargo.toml keywords + 准备剥离

**Files**:
- Create: `crates/muskitty-css-values/README.md`
- Modify: `crates/muskitty-css-values/Cargo.toml`（确认 keywords/categories 完整）

- [ ] **Step 1: 写 README**（crate 概述 + 规范覆盖 + 使用示例 + 测试命令）

- [ ] **Step 2: 创建 CI 文件**（按项目 extraction discipline）

- Create: `.github/workflows/ci.yml`（6 job：Check/Tests/Integration/Format/Clippy/MSRV）
- Create: `.github/workflows/publish.yml`（tag-triggered，幂等）
- Create: `scripts/setup-deps.sh`（克隆 path 依赖到 `../`）

- [ ] **Step 3: commit**

```bash
git add README.md .github/ scripts/
git commit -m "[css-values] CV-6: README + CI workflows + setup-deps.sh"
```

### Task 6-3: 全量回归 + PROGRESS.md 更新

- [ ] **Step 1: 全量测试**

```bash
cd crates/muskitty-css-values
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo check
```

- [ ] **Step 2: 跨 crate 联合测试**（确保 css-parser v0.2.0 + css v0.5.0 + css-values v0.1.0 协同）

```bash
cd crates/muskitty-selectors
cargo test  # 确保 selectors 不受 css-parser 升级影响
```

- [ ] **Step 3: 更新 PROGRESS.md**

在 PROGRESS.md 的总览表加 `muskitty-css-values` 行，Phase 2 规划的子阶段 3 标记为 ✅ 完成。

- [ ] **Step 4: commit + 准备推送/发布**

```bash
# css-values crate
cd crates/muskitty-css-values
git add -A
git commit -m "[css-values] mark Phase 2 子阶段 3 (CSS Values Module) complete"

# 主仓库
cd d:/Muskitty
git add PROGRESS.md .gitignore Cargo.toml
git commit -m "[workspace] update PROGRESS.md for muskitty-css-values completion"
```

---

## 自检

### 规范覆盖

| 规范章节 | 覆盖 task | 状态 |
|---------|----------|------|
| CSS Syntax §5.3 TokenStream source tracking | CV-0b Task 0b-1 | ✅ |
| CSS Syntax §5.5.6 original_text | CV-0b Task 0b-3 | ✅ |
| CSS Values §3.1 keywords | CV-2 Task 2-1 | ✅ |
| CSS Values §3.2 custom-ident | CV-2 Task 2-1 | ✅ |
| CSS Values §3.3 dashed-ident | CV-2 Task 2-1 | ✅ |
| CSS Values §3.4 string | CV-2 Task 2-2 | ✅ |
| CSS Values §3.5 url | CV-2 Task 2-2 | ✅ |
| CSS Values §4.3 integer | CV-1 Task 1-3 | ✅ |
| CSS Values §4.4 number | CV-1 Task 1-3 | ✅ |
| CSS Values §4.6 percentage | CV-1 Task 1-3 | ✅ |
| CSS Values §4.7 ratio | CV-1 Task 1-4 | ✅ |
| CSS Values §5 length (relative+absolute) | CV-1 Task 1-2 | ✅ |
| CSS Values §6.1 angle | CV-1 Task 1-3 | ✅ |
| CSS Values §6.2 time | CV-1 Task 1-3 | ✅ |
| CSS Values §6.3 frequency | CV-1 Task 1-3 | ✅ |
| CSS Values §6.4 resolution | CV-1 Task 1-3 | ✅ |
| CSS Values §8.1 serialization | CV-5 Task 5-2 | ✅ |
| CSS Values §9.1 calc() | CV-3 Task 3-2 | ✅ |
| CSS Values §9.2 min/max/clamp | CV-3 Task 3-3 | ✅ |
| CSS Values §9.3 constants | CV-3 Task 3-1 | ✅ |
| CSS Values §9.7 syntax | CV-3 Task 3-2 | ✅ |
| CSS Variables §2 custom properties | CV-0b (original_text) | ✅ |
| CSS Variables §3 var() syntax | CV-4 Task 4-1 | ✅ |
| CSS Variables §3 var() 求值 | 推迟到子阶段 5 | ⬜ 明确推迟 |
| CSS Values §9.8-§9.10 type checking | CV-5 Task 5-1 (subset) | ✅ 子集 |

### 推迟项（明确）

1. **calc()/min()/max()/clamp() 求值** — 需要百分比解析上下文，推迟到子阶段 5 Cascade。
2. **var() 替换求值（§3 的 4 步算法）** — 需要元素上下文 + custom property 计算值表 + 循环检测，推迟到子阶段 5。
3. **三角/指数/round/mod/rem/sign/abs 函数** — 布局用不到，按需补。
4. **§5.5.6 unicode-range re-tokenization** — 仍推迟（需要 source-text re-tokenization，本阶段只补了 original_text）。
5. **WPT 集成** — 拆仓后做。

### 风险点

1. **CV-0a 的 `position()` 语义** — `CssTokenizer.pos` 在 reconsume 场景下可能回退，需验证 `next_token_with_span` 的 span 是否准确。Task 0a-1 Step 5 的测试覆盖了这个。
2. **CV-0b 的 original_text 包含前导空格** — `source_slice(value_start_index, value_end_index)` 会包含 colon 后的 whitespace（如 `--foo: 10px` 的 original_text 是 ` 10px` 而非 `10px`）。这与规范一致——§5.5.6 说 original_text 是 "the concatenation of the values' representations"，但实际浏览器实现会 trim 前导空格。测试已反映这个行为（`Some(" 10px solid red")`）。如果 Cascade 阶段需要 trimmed 版本，可在求值时 trim。
3. **CV-3 的运算符优先级** — `+`/`-` 低优先级，`*`/`/` 高优先级。递归下降解析器需正确处理左结合。测试 `calc_mixed_operations` 覆盖。
4. **CV-3 的 whitespace 规则** — §9.1 规定 `+`/`-` 两侧**必须**有 whitespace（否则 `10px-5px` 会被解析成 dimension `10px` + `-5px`），`*`/`/` 两侧可选。解析器需检查这个约束。

### 依赖发版顺序

```
muskitty-css-tokenizer v0.2.0 (CV-0a)
    ↓
muskitty-css-parser v0.2.0 (CV-0b)
    ↓
muskitty-css v0.5.0 (re-export bump)
    ↓
muskitty-css-values v0.1.0 (CV-1~CV-6)
```

每个发版前确认 crates.io 上前序依赖已发布。CV-0a/b 发版后才能开始 CV-1（CV-1 依赖 css v0.5.0）。
