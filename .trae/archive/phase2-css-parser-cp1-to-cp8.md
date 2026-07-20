# Phase 2 — muskitty-css Parser CP-1 → CP-8

> 规范源：`D:\CSSWG\css-syntax-3\Overview.md`（CSS Syntax Module Level 3, Markdown 版本，2999 行）。
> 一切以标准为准。每个 commit message 必须引用 §章节号与 L 行号。
> 上一份计划：`.trae/documents/phase2-css-tokenizer-c4-finish-to-c7.md`（tokenizer §4.3 全部完成，71 测试 + 1 doctest 全绿）。

## 摘要

CSS Syntax Module §4.3 tokenizer 已完成（71 测试 + 1 doctest 全绿，C-7 后零 warning）。本计划覆盖 **§5 Parsing** 完整实现：§5.2 CSS Parsing Results（数据结构骨架）、§5.3 Token Streams（解析器访问接口）、§5.4 Parser Entry Points（10 个 entry points）、§5.5 Parser Algorithms（11 个 algorithms）。

按依赖顺序分 8 个批次（CP-1 → CP-8），每个批次独立 commit + 测试 + cargo check 零 warning。完成后 muskitty-css 即可作为完整 CSS Syntax 解析器对外提供 `parse_stylesheet()` / `parse_rule()` / `parse_declaration()` 等顶层 API。

## 当前状态分析

### Git 状态

- 分支 `main`，工作树干净（除两份历史计划文档 untracked）。
- 最新提交：`4f57e27 [css-tokenizer] C-7: cleanup helpers + align docs with 4.3 coverage`。
- crates.io 状态：muskitty-css **尚未发布**（v0.1.0），待 CP-8 完成后首版发布。

### 已实现并提交（C-0 ~ C-7）

**Tokenizer（§4.3，15 个子算法，全部完成）：**
- §4.3.1 `consume_a_token` 主分发
- §4.3.2 `consume_comments_body`
- §4.3.3 `consume_a_numeric_token`
- §4.3.4 `consume_an_ident_like_token`（含 url( 特例）
- §4.3.5 `consume_a_string_token`
- §4.3.6 `consume_a_url_token`
- §4.3.7 `consume_an_escaped_code_point`
- §4.3.8 `is_valid_escape_at` / `is_valid_escape_next`
- §4.3.9 `would_start_ident_sequence_at`
- §4.3.10 `starts_with_number`
- §4.3.11 `would_start_unicode_range_at`
- §4.3.12 `consume_an_ident_sequence`
- §4.3.13 `consume_a_number`
- §4.3.14 `consume_a_unicode_range_token`
- §4.3.15 `consume_the_remnants_of_a_bad_url`
- §5.3 `preprocess_input`（输入预处理）

**Tokenizer 辅助：**
- `CssTokenizer` struct：`input: Vec<char>` + `pos: usize` + `state: State` + `unicode_ranges_allowed: bool`
- `Tokenizer` trait：`next_token` / `state` / `set_state` / `reset` / `set_unicode_ranges_allowed`
- `Token` enum（30 变体）+ `Numeric` struct + `HashType` enum + `State` enum
- `tokenize(input: &str) -> Vec<Token>` 顶层函数（lib.rs）
- `CssTokenizer::collect(input: &str) -> Vec<Token>` 测试 helper

### 仍为 stub / 未实现

- §5 Parsing 完全未实现。
- `src/parser/` 目录不存在。
- `lib.rs` 仅暴露 `tokenize()`，无 `parse_stylesheet()` / `parse_rule()` 等顶层 API。

## 规范源结构（§5 Parsing 完整范围）

来源：`D:\CSSWG\css-syntax-3\Overview.md`（Markdown 版本，2999 行）。

| 章节 | 标题 | L 行号 | 子项数 | 范围 |
|------|------|--------|--------|------|
| §5 | Parsing | L1581 | — | 整章 |
| §5.1 | Parser Railroad Diagrams | L1590 | — | 非规范 |
| §5.2 | CSS Parsing Results | L1625 | 9 类型 | stylesheet/rule/at-rule/qualified-rule/declaration/component-value/preserved-tokens/function/simple-block |
| §5.3 | Token Streams | L1722 | 9 操作 | struct(tokens+index+marked_indexes) + next_token/empty/consume/discard/mark/restore/discard_mark/discard_whitespace/process |
| §5.4 | Parser Entry Points | L1816 | 10 entry points | normalize + parse_a_stylesheet / contents / block_contents / rule / declaration / component_value / list_of_component_values / comma_separated_list + 2 个通用 grammar hook |
| §5.5 | Parser Algorithms | L2208 | 11 algorithms | consume_a_stylesheet_contents / at_rule / qualified_rule / block / blocks_contents / declaration + remnants / list_of_components / component_value / simple_block / function / unicode_range_value |

### §5.2 数据结构定位（Markdown L1625-1721）

- L1632-1633: stylesheet — `A stylesheet has a list of [=rules=].`
- L1635-1637: rule — `A [=rule=] is either an [=at-rule=] or a [=qualified rule=].`
- L1639-1650: at-rule — name + prelude + 可选 declarations + child rules（block at-rules 才有）
- L1652-1657: qualified rule — prelude + declarations + child rules
- L1663-1669: declaration — name + value + important flag + optional `original text`
- L1681-1685: component value — preserved token | function | simple block
- L1687-1703: preserved tokens — 除 `function-token` / `{-token` / `(-token` / `[-token` 外所有
- L1705-1708: function — name + value
- L1710-1719: simple block — associated token + value；`{}-block` / `[]-block` / `()-block`

### §5.3 Token Stream 定位（Markdown L1722-1814）

- struct 字段（L1725-1754）：
  - L1730-1738: `tokens` — list of tokens/component values
  - L1740-1748: `index` — 当前位置，初始 0，永不回退（除 mark/restore）
  - L1750-1753: `marked_indexes` — 栈，初始空
- L1756-1764: original text 重放能力（隐式要求）
- 9 个操作（L1766-1808）：
  - L1769-1773: `next token` — 越界返回 `eof-token`
  - L1775-1777: `empty` — next token 是 `eof-token`
  - L1779-1782: `consume a token` — 取 next，index+=1，返回 token
  - L1784-1786: `discard a token` — 非空则 index+=1
  - L1788-1789: `mark` — 把 index push 到 marked_indexes
  - L1791-1793: `restore a mark` — pop marked_indexes，把 index 设为 pop 值
  - L1795-1797: `discard a mark` — pop marked_indexes，丢弃
  - L1799-1801: `discard whitespace` — while next 是 whitespace，discard
  - L1803-1808: `process` — 按 next token 类型分派到对应 action
- L1811-1813: EOF token — 概念性 token，不实际产生，表示流已耗尽

### §5.4 Parser Entry Points 定位（Markdown L1816-2206）

- L1827-1842: `normalize into a token stream` — 接受 token stream / list / string
- L1895-1944: §5.4.1 Parse something according to a CSS grammar（**defer**）
- L1949-2001: §5.4.2 Parse a comma-separated list according to a CSS grammar（**defer**）
- L2005-2033: §5.4.3 Parse a stylesheet — decode bytes → normalize → 创建 stylesheet → consume_a_stylesheets_contents
- L2037-2051: §5.4.4 Parse a stylesheet's contents — normalize → consume_a_stylesheets_contents
- L2055-2069: §5.4.5 Parse a block's contents — normalize → consume_a_blocks_contents
- L2073-2109: §5.4.6 Parse a rule — discard ws；EOF → syntax error；at-keyword → consume_an_at_rule；else consume_a_qualified_rule；discard ws；非 EOF → syntax error
- L2113-2134: §5.4.7 Parse a declaration — normalize → discard ws → consume_a_declaration
- L2138-2168: §5.4.8 Parse a component value — discard ws；空 → syntax error；consume_a_component_value；discard ws；非空 → syntax error
- L2172-2183: §5.4.9 Parse a list of component values — normalize → consume_a_list_of_component_values
- L2186-2204: §5.4.10 Parse a comma-separated list of component values — 用 `,` 作为 stop token，循环

### §5.5 Parser Algorithms 定位（Markdown L2208-2872）

- L2210-2221: 章节前言 — 大小写敏感；有效性检查需在解析中做（部分场景）
- §5.5.1 Consume a stylesheet's contents（L2223-2279）
  - L2229: `Let |rules| be an initially empty [=list=] of rules.`
  - L2231: `[=token stream/Process=] |input|`
  - L2234-2236: whitespace → discard
  - L2238-2240: EOF → return rules
  - L2242-2245: CDO/CDC → discard
  - L2267-2271: at-keyword → consume_an_at_rule；if Some, append
  - L2273-2277: anything else → consume_a_qualified_rule；if Some, append
- §5.5.2 Consume an at-rule（L2281-2337）
  - L2286: optional bool `nested` (default false)
  - L2288: Assert next token is at-keyword
  - L2290-2294: consume at-keyword → 设 rule.name / prelude=[] / 无 decls/rules
  - L2299-2305: semicolon / EOF → discard, return rule（或 nothing，根据 validity）
  - L2307-2316: `}-token` → nested=true 时 return；否则 consume 到 prelude
  - L2317-2331: `{-token` → consume_a_block → rule 的 child rules；return rule
  - L2333-2336: anything else → consume_a_component_value → prelude
- §5.5.3 Consume a qualified rule（L2340-2466）
  - L2345-2346: optional `stop_token` + optional bool `nested` (default false)
  - L2348-2350: rule = QualifiedRule { prelude=[], decls=[], rules=[] }
  - L2355-2359: EOF / stop_token → parse error, return nothing
  - L2361-2368: `}-token` → parse error；nested → return nothing；否则 consume 到 prelude
  - L2370-2460: `{-token` → 检查 prelude 是否像 `--foo:`（custom property）
    - L2377-2383: nested=true → consume remnants of bad decl, return nothing
    - L2381-2383: nested=false → consume_a_block, return nothing
    - L2448-2460: 否则 consume_a_block → 拆 decls + rules → return rule（或 invalid rule error）
  - L2462-2465: anything else → consume_a_component_value → prelude
- §5.5.4 Consume a block（L2469-2484）
  - L2475-2476: Assert next token is `{-token`
  - L2478: discard token
  - L2479-2480: consume_a_blocks_contents → rules
  - L2481: discard token（`}-token` 或 EOF）
  - L2483: return rules
- §5.5.5 Consume a block's contents（L2486-2636）
  - L2492-2498: 返回 list of (rules | declaration-lists)
  - L2500-2502: rules = []
  - L2504: decls = []
  - L2509-2512: whitespace / semicolon → discard
  - L2514-2517: EOF / `}-token` → return rules
  - L2519-2528: at-keyword → 若 decls 非空，append 到 rules；consume_an_at_rule(nested=true)
  - L2530-2562: anything else → mark；consume_a_declaration(nested=true)
    - L2536-2538: Some(decl) → append to decls, discard mark
    - L2540-2543: None → restore mark, consume_a_qualified_rule(nested=true, stop=`;`)
      - L2546-2547: None → do nothing
      - L2549-2554: invalid rule error → 若 decls 非空，append 到 rules
      - L2556-2561: Some(rule) → 若 decls 非空，append 到 rules；append rule
  - L2566-2634: Implementation note（性能优化提示，非规范）
- §5.5.6 Consume a declaration + remnants of bad declaration（L2638-2742）
  - 主算法（L2639-2717）：
    - L2643: optional bool `nested` (default false)
    - L2645-2647: decl = Declaration { name="", value=[] }
    - L2650-2659: 若 next 是 ident-token → consume，设 name；否则 consume remnants, return nothing
    - L2661-2662: discard whitespace
    - L2664-2671: 若 next 是 colon → discard；否则 consume remnants, return nothing
    - L2673-2674: discard whitespace
    - L2676-2680: consume_a_list_of_component_values(stop=`;`, nested) → decl.value
    - L2682-2687: 若末尾两非 ws 是 `!` delim + `important` ident（ASCII case-insensitive）→ 移除，设 important flag
    - L2689-2691: 去尾 whitespace
    - L2693-2698: 若是 custom property name → 设 original_text（用 token range）
    - L2700-2705: 否则若 value 含顶层 {}-block 且有其他非 ws 值 → return nothing
    - L2707-2712: 否则若 name 是 `unicode-range`（case-insensitive）→ consume unicode-range descriptor
    - L2714-2717: 若 decl 有效 → return；否则 return nothing
  - remnants of bad declaration（L2721-2741）：
    - L2723: 给定 bool `nested`
    - L2725: process input
    - L2727-2730: EOF / semicolon → discard, return nothing
    - L2732-2737: `}-token` → nested=true 时 return nothing；否则 discard
    - L2739-2741: anything else → consume_a_component_value（丢弃结果）
- §5.5.7 Consume a list of component values（L2745-2774）
  - L2750-2751: optional `stop_token` + optional bool `nested` (default false)
  - L2753: values = []
  - L2757-2759: EOF / stop_token → return values
  - L2761-2769: `}-token` → nested=true 时 return values；否则 parse error, consume 并 append
  - L2771-2773: anything else → consume_a_component_value, append
- §5.5.8 Consume a component value（L2776-2796）
  - L2782: process input
  - L2784-2788: `{-token` / `[-token` / `(-token` → consume_a_simple_block
  - L2790-2792: `function-token` → consume_a_function
  - L2794-2796: anything else → consume token 并返回
- §5.5.9 Consume a simple block（L2799-2829）
  - L2805-2808: Assert next 是 `{-token` / `[-token` / `(-token`
  - L2810-2812: ending token = mirror variant（如 `[` → `]`）
  - L2814-2816: block = SimpleBlock { associated=next, value=[] }
  - L2818: discard token
  - L2822-2825: EOF / ending token → discard, return block
  - L2827-2829: anything else → consume_a_component_value, append
- §5.5.10 Consume a function（L2832-2854）
  - L2838: Assert next 是 function-token
  - L2840-2843: consume token → function = Function { name=token.value, value=[] }
  - L2847-2850: EOF / `)-token` → discard, return function
  - L2852-2854: anything else → consume_a_component_value, append
- §5.5.11 Consume a '@font-face/unicode-range' value（L2857-2872）
  - L2860-2864: tokenize input_string with `unicode_ranges_allowed=true`
  - L2866-2867: consume_a_list_of_component_values(tokens) → return
  - L2869-2871: Note — 设计失误，不应再现

## 提议变更

### 阶段 1：CP-1 — §5.2 数据结构骨架

**目标**：建立 `src/parser/` 模块，定义 §5.2 的 9 种 CSS Parsing Results 类型。纯数据结构，无算法逻辑。

**规范依据**：§5.2 L1625-1721。

**文件**：
- `d:\Muskitty\crates\muskitty-css\src\parser\mod.rs`（新）
- `d:\Muskitty\crates\muskitty-css\src\parser\types.rs`（新）
- `d:\Muskitty\crates\muskitty-css\src\lib.rs`（加 `pub mod parser;`）

**类型定义**（`parser/types.rs`）：

```rust
//! CSS Parsing Results data structures (§5.2).
//!
//! Per §5.2 L1625-1721: the result of parsing can be a stylesheet, a rule
//! (at-rule or qualified rule), a declaration, or a component value
//! (preserved token, function, or simple block).

use crate::tokenizer::Token;

/// §5.2 L1632-1633: A stylesheet has a list of rules.
#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

/// §5.2 L1635-1637: A rule is either an at-rule or a qualified rule.
///
/// Also carries a `Declarations` variant (§5.5.5 L2492-2498) for the
/// mixed list output of `consume_a_blocks_contents` — a block's
/// contents can be a sequence of declarations interleaved with rules,
/// and we model each declaration-list run as a `Rule::Declarations`.
#[derive(Debug, Clone)]
pub enum Rule {
    AtRule(AtRule),
    QualifiedRule(QualifiedRule),
    /// §5.5.5: a list of declarations (a "run" of consecutive declarations
    /// inside a block, before the next child rule). At the CSSOM boundary
    /// this gets materialized as either `CSSStyleDeclaration` or
    /// `CSSNestedDeclarations`.
    Declarations(Vec<Declaration>),
}

/// §5.2 L1639-1650: An at-rule has a name, a prelude (list of component
/// values), and optionally a list of declarations and a list of child
/// rules (only for "block at-rules" ending in a {}-block).
#[derive(Debug, Clone)]
pub struct AtRule {
    /// The at-rule name (e.g. "media", "import"). Does not include the
    /// leading `@`.
    pub name: String,
    /// The prelude: component values between the name and the block or
    /// semicolon.
    pub prelude: Vec<ComponentValue>,
    /// Block at-rules only: declarations inside the {}-block. `None` for
    /// statement at-rules (ending in `;`).
    pub declarations: Option<Vec<Declaration>>,
    /// Block at-rules only: child rules inside the {}-block. `None` for
    /// statement at-rules.
    pub child_rules: Option<Vec<Rule>>,
}

/// §5.2 L1652-1657: A qualified rule has a prelude, declarations, and
/// child rules.
#[derive(Debug, Clone)]
pub struct QualifiedRule {
    /// The prelude (e.g. a selector for style rules).
    pub prelude: Vec<ComponentValue>,
    /// Declarations inside the {}-block.
    pub declarations: Vec<Declaration>,
    /// Child rules inside the {}-block (for nested rules like `@media`).
    pub child_rules: Vec<Rule>,
}

/// §5.2 L1663-1669: A declaration has a name, a value (list of component
/// values), an `important` flag, and an optional `original_text`.
#[derive(Debug, Clone)]
pub struct Declaration {
    /// The property or descriptor name (e.g. "color", "font-family").
    pub name: String,
    /// The value: component values between `:` and `;` (or end of block).
    pub value: Vec<ComponentValue>,
    /// Whether the declaration had an `!important` flag.
    pub important: bool,
    /// §5.2 L1668-1669 + §5.5.6 L2693-2698: only set for custom property
    /// declarations (`--foo: ...`), to allow var() resolution to access
    /// the original source text.
    pub original_text: Option<String>,
}

/// §5.2 L1681-1685: A component value is one of the preserved tokens, a
/// function, or a simple block.
#[derive(Debug, Clone)]
pub enum ComponentValue {
    /// §5.2 L1687-1703: A preserved token (any token except function-token,
    /// `{-token`, `(-token`, `[-token` — those are always consumed into
    /// higher-level objects).
    PreservedToken(Token),
    /// §5.2 L1705-1708: A function.
    Function(Function),
    /// §5.2 L1710-1719: A simple block.
    SimpleBlock(SimpleBlock),
}

/// §5.2 L1705-1708: A function has a name and a value (list of component
/// values).
#[derive(Debug, Clone)]
pub struct Function {
    /// The function name (e.g. "translate", "var"). Does not include the
    /// leading ident or the trailing `(`.
    pub name: String,
    /// The arguments: component values between `(` and `)`.
    pub value: Vec<ComponentValue>,
}

/// §5.2 L1710-1719: A simple block has an associated token (the opening
/// token) and a value (list of component values).
#[derive(Debug, Clone)]
pub struct SimpleBlock {
    /// Which kind of block this is: `{}`, `[]`, or `()`.
    pub kind: BlockKind,
    /// The component values inside the block.
    pub value: Vec<ComponentValue>,
}

/// §5.2 L1710-1719: The associated token kind of a simple block, mirroring
/// the opening `{` / `[` / `(`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// `{}-block` (L1718): opens with `{-token`, closes with `}-token`.
    Curly,
    /// `[]-block` (L1718): opens with `[-token`, closes with `]-token`.
    Square,
    /// `()-block` (L1718): opens with `(-token`, closes with `)-token`.
    Paren,
}
```

**新增测试**（`tests/parser_types.rs`）：6 个测试
- `stylesheet_default_empty` — `Stylesheet::default()` 空规则列表
- `rule_at_rule_variant` — `Rule::AtRule(...)` 构造
- `rule_qualified_rule_variant` — `Rule::QualifiedRule(...)` 构造
- `rule_declarations_variant` — `Rule::Declarations(vec![])` 构造（§5.5.5 准备）
- `at_rule_statement_vs_block` — 同名 AtRule 一个 statement（None declarations）一个 block（Some declarations）
- `declaration_important_flag` — `important: true` 设置

**验证 + 提交**（每个 commit 必须依次跑 fmt / test / clippy -D warnings 全绿后才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
若有 fmt diff，先 `cargo fmt -p muskitty-css` 修格式，再跑后续检查。预期 77/77 green（71 + 6 新），零 warning，零 fmt diff。

**Commit**：
```
[css-parser] CP-1: 5.2 CSS Parsing Results data structures

- §5.2 L1632-1633 Stylesheet: list of rules.
- §5.2 L1635-1637 Rule: at-rule | qualified rule | declarations (§5.5.5).
- §5.2 L1639-1650 AtRule: name + prelude + optional decls/rules.
- §5.2 L1652-1657 QualifiedRule: prelude + declarations + child_rules.
- §5.2 L1663-1669 Declaration: name + value + important + original_text.
- §5.2 L1681-1685 ComponentValue: preserved token | function | simple block.
- §5.2 L1705-1708 Function: name + value.
- §5.2 L1710-1719 SimpleBlock + BlockKind { Curly, Square, Paren }.
- No algorithm logic; pure data types for downstream CP-2..CP-7.
- 6 new unit tests.
```

---

### 阶段 2：CP-2 — §5.3 Token Stream

**目标**：实现 §5.3 的 TokenStream struct 与 9 个操作。这是 parser 的工作接口，所有 §5.5 算法都基于它。

**规范依据**：§5.3 L1722-1814。

**文件**：
- `d:\Muskitty\crates\muskitty-css\src\parser\token_stream.rs`（新）
- `d:\Muskitty\crates\muskitty-css\src\parser\mod.rs`（加 `mod token_stream;`）

**实现**：

```rust
//! Token stream (§5.3 L1722-1814).
//!
//! A token stream is a struct representing a stream of tokens and/or
//! component values. It has three fields: `tokens` (a list), `index`
//! (current position), and `marked_indexes` (a stack of saved positions
//! for backtracking).

use crate::tokenizer::Token;

/// §5.3 L1725-1754: A token stream.
#[derive(Debug, Clone)]
pub struct TokenStream {
    /// §5.3 L1730-1738: A list of tokens and/or component values. We
    /// model component values as `Token`s here for simplicity; the
    /// §5.5 algorithms that consume "component values" wrap tokens into
    /// [`crate::parser::types::ComponentValue`] at the boundary.
    pub tokens: Vec<Token>,
    /// §5.3 L1740-1748: An index into `tokens`, representing parsing
    /// progress. Starts at 0. Never decreases except via
    /// [`Self::restore_mark`].
    pub index: usize,
    /// §5.3 L1750-1753: A stack of index values for backtracking. Starts
    /// empty.
    marked_indexes: Vec<usize>,
}

impl TokenStream {
    /// §5.3: Construct a new token stream over `tokens`. The stream
    /// implicitly appends an EOF token (§5.3 L1811-1813); we model it
    /// by returning `Token::Eof` from [`Self::next_token`] when index
    /// is out of bounds, rather than storing a sentinel.
    pub fn new(mut tokens: Vec<Token>) -> Self {
        // Ensure an EOF token is present at the end (§5.3 L1811-1813).
        // The tokenizer already emits one, but be defensive.
        if tokens.last().map_or(true, |t| !matches!(t, Token::Eof)) {
            tokens.push(Token::Eof);
        }
        Self { tokens, index: 0, marked_indexes: Vec::new() }
    }

    /// §5.3 L1769-1773: The item of `tokens` at `index`. If out-of-bounds,
    /// return `Token::Eof`.
    pub fn next_token(&self) -> Token {
        self.tokens.get(self.index).cloned().unwrap_or(Token::Eof)
    }

    /// §5.3 L1775-1777: A token stream is empty if the next token is
    /// `<EOF-token>`.
    pub fn is_empty(&self) -> bool {
        matches!(self.next_token(), Token::Eof)
    }

    /// §5.3 L1779-1782: Let `token` be the next token. Increment `index`,
    /// then return `token`.
    pub fn consume_token(&mut self) -> Token {
        let token = self.next_token();
        if !matches!(token, Token::Eof) {
            self.index += 1;
        }
        token
    }

    /// §5.3 L1784-1786: If not empty, increment `index`.
    pub fn discard_token(&mut self) {
        if !self.is_empty() {
            self.index += 1;
        }
    }

    /// §5.3 L1788-1789: Append `index` to `marked_indexes`.
    pub fn mark(&mut self) {
        self.marked_indexes.push(self.index);
    }

    /// §5.3 L1791-1793: Pop from `marked_indexes` and set `index` to the
    /// popped value. No-op if stack is empty (defensive).
    pub fn restore_mark(&mut self) {
        if let Some(idx) = self.marked_indexes.pop() {
            self.index = idx;
        }
    }

    /// §5.3 L1795-1797: Pop from `marked_indexes` and discard.
    pub fn discard_mark(&mut self) {
        let _ = self.marked_indexes.pop();
    }

    /// §5.3 L1799-1801: While the next token is a `<whitespace-token>`,
    /// discard a token.
    pub fn discard_whitespace(&mut self) {
        while matches!(self.next_token(), Token::Whitespace) {
            self.discard_token();
        }
    }
}
```

**关于 §5.3 `process` 操作（L1803-1808）**：规范用 dispatch table，但 Rust 闭包写起来更直观且语义等价。CP-3+ 的算法直接调用 `loop { match input.next_token() {...} }` 实现，不暴露 `process` 作为公共 API。这样避免引入 enum-dispatch 复杂度。

**新增测试**（`tests/token_stream.rs`）：8 个测试
- `next_token_at_start` — 第一个 token
- `next_token_at_end_returns_eof` — 越界返回 `Token::Eof`
- `consume_token_advances_index` — 消费后 index +1
- `discard_token_at_eof_no_panic` — EOF 时不 panic
- `mark_and_restore_mark` — mark → consume 多次 → restore_mark → index 回到 mark 点
- `discard_mark_does_not_restore` — discard_mark 后 restore_mark 用旧 mark
- `discard_whitespace_consumes_run` — 连续 whitespace 被全部消费
- `eof_implicit_appended` — `TokenStream::new(vec![Token::Ident("a".into())])` 自动追加 Eof

**验证 + 提交**（每个 commit 必须依次跑 fmt / test / clippy -D warnings 全绿后才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
若有 fmt diff，先 `cargo fmt -p muskitty-css` 修格式，再跑后续检查。预期 85/85 green（77 + 8），零 warning，零 fmt diff。

**Commit**：
```
[css-parser] CP-2: 5.3 TokenStream struct + 8 operations

- §5.3 L1725-1754 TokenStream { tokens, index, marked_indexes }.
- §5.3 L1769-1773 next_token (EOF on out-of-bounds).
- §5.3 L1775-1777 is_empty.
- §5.3 L1779-1782 consume_token.
- §5.3 L1784-1786 discard_token.
- §5.3 L1788-1789 mark.
- §5.3 L1791-1793 restore_mark.
- §5.3 L1795-1797 discard_mark.
- §5.3 L1799-1801 discard_whitespace.
- §5.3 L1811-1813: implicit <EOF-token> appended in constructor.
- §5.3 L1803-1808 `process` operation is inlined in callers (CP-3+) as
  `while` + `match`, avoiding enum-dispatch complexity; semantics
  equivalent.
- 8 new unit tests.
```

---

### 阶段 3：CP-3 — §5.5 底层 algorithms (component value / simple block / function / list / unicode-range)

**目标**：实现 §5.5 最底层的 5 个 algorithms（依赖关系最浅）。这些是 §5.5.7-§5.5.11。

**规范依据**：§5.5.7-§5.5.11 L2745-2872。

**文件**：
- `d:\Muskitty\crates\muskitty-css\src\parser\algorithms.rs`（新）
- `d:\Muskitty\crates\muskitty-css\src\parser\mod.rs`（加 `mod algorithms;`）

**实现**：

```rust
//! §5.5 Parser Algorithms.
//!
//! Implementation of the 11 algorithms defined in CSS Syntax Module
//! Level 3 §5.5. This module covers CP-3 (lower-level algorithms):
//! - §5.5.7 consume_a_list_of_component_values
//! - §5.5.8 consume_a_component_value
//! - §5.5.9 consume_a_simple_block
//! - §5.5.10 consume_a_function
//! - §5.5.11 consume_a_unicode_range_value
//!
//! CP-4 will add §5.5.6 (consume_a_declaration + remnants_of_a_bad_declaration),
//! CP-5 will add §5.5.1-§5.5.5.

use super::token_stream::TokenStream;
use super::types::{BlockKind, ComponentValue, Function, SimpleBlock};
use crate::tokenizer::Token;

/// §5.5.8 (L2776-2796) Consume a component value.
///
/// Dispatch on the next token:
/// - `{-token` / `[-token` / `(-token` → consume_a_simple_block
/// - `function-token` → consume_a_function
/// - anything else → consume and return the token as PreservedToken
pub fn consume_a_component_value(input: &mut TokenStream) -> ComponentValue {
    match input.next_token() {
        Token::OpenBrace | Token::OpenBracket | Token::OpenParen => {
            ComponentValue::SimpleBlock(consume_a_simple_block(input))
        }
        Token::Function(_) => ComponentValue::Function(consume_a_function(input)),
        other => {
            input.consume_token();
            ComponentValue::PreservedToken(other)
        }
    }
}

/// §5.5.9 (L2799-2829) Consume a simple block.
///
/// Precondition: next token is `{-token` / `[-token` / `(-token`.
/// Mirror variant becomes the ending token (e.g. `[` → `]`).
/// Repeatedly consume component values until ending token or EOF.
pub fn consume_a_simple_block(input: &mut TokenStream) -> SimpleBlock {
    let opening = input.next_token();
    let kind = match opening {
        Token::OpenBrace => BlockKind::Curly,
        Token::OpenBracket => BlockKind::Square,
        Token::OpenParen => BlockKind::Paren,
        _ => unreachable!("consume_a_simple_block called on non-opening token"),
    };
    let ending = match kind {
        BlockKind::Curly => Token::CloseBrace,
        BlockKind::Square => Token::CloseBracket,
        BlockKind::Paren => Token::CloseParen,
    };
    input.discard_token(); // discard the opening token (§5.5.9 L2818)

    let mut block = SimpleBlock { kind, value: Vec::new() };
    loop {
        let next = input.next_token();
        match next {
            Token::Eof | t if next == ending => {
                input.discard_token();
                return block;
            }
            _ => block.value.push(consume_a_component_value(input)),
        }
    }
}

/// §5.5.10 (L2832-2854) Consume a function.
///
/// Precondition: next token is a `function-token`.
/// Consume the function token, then consume component values until
/// `)-token` or EOF.
pub fn consume_a_function(input: &mut TokenStream) -> Function {
    let name = match input.consume_token() {
        Token::Function(name) => name,
        _ => unreachable!("consume_a_function called on non-function token"),
    };
    let mut function = Function { name, value: Vec::new() };
    loop {
        match input.next_token() {
            Token::Eof | Token::CloseParen => {
                input.discard_token();
                return function;
            }
            _ => function.value.push(consume_a_component_value(input)),
        }
    }
}

/// §5.5.7 (L2745-2774) Consume a list of component values.
///
/// `stop_token`: optional token that ends the list (e.g. `;` for
/// declarations). `nested`: when true, an unbalanced `}-token` ends the
/// list without consuming; when false, `}-token` is a parse error and is
/// consumed into the list.
pub fn consume_a_list_of_component_values(
    input: &mut TokenStream,
    stop_token: Option<Token>,
    nested: bool,
) -> Vec<ComponentValue> {
    let mut values = Vec::new();
    loop {
        let next = input.next_token();
        match next {
            Token::Eof => return values,
            t if stop_token.as_ref().map_or(false, |s| *s == t) => return values,
            Token::CloseBrace => {
                if nested {
                    return values;
                }
                // §5.5.7 L2766-2769: parse error. Consume and append.
                input.consume_token();
                values.push(ComponentValue::PreservedToken(next));
            }
            _ => values.push(consume_a_component_value(input)),
        }
    }
}

/// §5.5.11 (L2857-2872) Consume the value of a `@font-face/unicode-range`
/// descriptor.
///
/// Tokenize `input_string` with `unicode_ranges_allowed=true`, then
/// consume a list of component values from the resulting stream.
///
/// Per §5.5.11 L2869-2871 note: "The existence of this algorithm is due
/// to a design mistake in early CSS. It should never be reproduced."
pub fn consume_a_unicode_range_value(input_string: &str) -> Vec<ComponentValue> {
    use crate::tokenizer::{CssTokenizer, Tokenizer};
    let mut tz = CssTokenizer::new(input_string);
    tz.set_unicode_ranges_allowed(true);
    let mut tokens: Vec<Token> = Vec::new();
    while let Some(token) = tz.next_token() {
        tokens.push(token);
        if matches!(token, Token::Eof) {
            break;
        }
    }
    let mut stream = TokenStream::new(tokens);
    consume_a_list_of_component_values(&mut stream, None, false)
}
```

**新增测试**（`tests/parser_algorithms_cp3.rs`）：10 个测试
- `component_value_preserved_token` — `consume_a_component_value` 消费 Ident → PreservedToken
- `component_value_simple_block_curly` — `{ foo }` → SimpleBlock(Curly, [Ident("foo")])
- `component_value_simple_block_square` — `[ 1 2 ]` → SimpleBlock(Square, ...)
- `component_value_function` — `foo(1)` → Function("foo", [Number(1)])
- `simple_block_unclosed_at_eof` — `{ foo` (无 `}`) → EOF 退出，block 含 foo
- `function_unclosed_at_eof` — `foo(1` (无 `)`) → EOF 退出
- `list_of_components_until_semicolon` — `a; b` with stop=`;` → [Ident("a")]，剩 `b`
- `list_of_components_nested_close_brace_returns` — `a } b` with nested=true → [Ident("a")]，剩 `}` 和 `b`
- `list_of_components_top_level_close_brace_is_error` — `a } b` with nested=false → [Ident("a"), CloseBrace, Ident("b")]
- `unicode_range_value_consumes_range_token` — `"U+1234"` 经 §5.5.11 → [UnicodeRange(Some(0x1234), Some(0x1234))]

**验证 + 提交**（每个 commit 必须依次跑 fmt / test / clippy -D warnings 全绿后才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
若有 fmt diff，先 `cargo fmt -p muskitty-css` 修格式，再跑后续检查。预期 95/95 green（85 + 10），零 warning，零 fmt diff。

**Commit**：
```
[css-parser] CP-3: 5.5.7-5.5.11 lower-level parser algorithms

- §5.5.8 (L2776-2796) consume_a_component_value: dispatch on next
  token (open-{,[,( → simple_block; function-token → function; else
  preserved token).
- §5.5.9 (L2799-2829) consume_a_simple_block: mirror variant ends;
  recurse component_value until close or EOF.
- §5.5.10 (L2832-2854) consume_a_function: name from function-token;
  component_value list until `)` or EOF.
- §5.5.7 (L2745-2774) consume_a_list_of_component_values: optional
  stop token; nested flag controls `}` handling (return vs parse error
  + consume).
- §5.5.11 (L2857-2872) consume_a_unicode_range_value: tokenize with
  unicode_ranges_allowed=true, then consume_a_list_of_component_values.
- 10 new unit tests covering each algorithm + EOF/nested edge cases.
```

---

### 阶段 4：CP-4 — §5.5.6 Declaration algorithms

**目标**：实现 §5.5.6 `consume_a_declaration` 与 `consume_the_remnants_of_a_bad_declaration`。

**规范依据**：§5.5.6 L2638-2742。

**文件**：`d:\Muskitty\crates\muskitty-css\src\parser\algorithms.rs`（追加）。

**实现**：

```rust
/// §5.5.6 (L2639-2717) Consume a declaration.
///
/// `nested`: when true, an unbalanced `}-token` returns `None` without
/// consuming (caller will handle the closing brace); when false, it's
/// consumed as part of the declaration's value.
///
/// Steps:
/// 1. If next token is ident, consume it as the declaration name.
///    Otherwise, consume remnants of a bad declaration, return None.
/// 2. Discard whitespace.
/// 3. If next token is `:`, discard it. Otherwise, bad declaration,
///    return None.
/// 4. Discard whitespace.
/// 5. Consume a list of component values with `;` as stop token.
/// 6. If last two non-whitespace tokens are `!` + `important` (ASCII
///    case-insensitive), remove them and set `important` flag.
/// 7. Strip trailing whitespace tokens.
/// 8. Custom property (`--foo`): set `original_text`. Otherwise, if
///    value contains a top-level {}-block AND any other non-ws value,
///    return None (only the whole value may be a {}-block).
/// 9. (Skip validity check; left to higher-level callers.)
pub fn consume_a_declaration(
    input: &mut TokenStream,
    nested: bool,
) -> Option<Declaration> {
    let name = match input.next_token() {
        Token::Ident(name) => {
            input.consume_token();
            name
        }
        _ => {
            consume_the_remnants_of_a_bad_declaration(input, nested);
            return None;
        }
    };

    input.discard_whitespace();

    match input.next_token() {
        Token::Colon => input.discard_token(),
        _ => {
            consume_the_remnants_of_a_bad_declaration(input, nested);
            return None;
        }
    }

    input.discard_whitespace();

    let mut value = consume_a_list_of_component_values(
        input,
        Some(Token::Semicolon),
        nested,
    );

    // Step 6 (§5.5.6 L2682-2687): strip !important from the tail.
    let important = strip_important(&mut value);

    // Step 7 (§5.5.6 L2689-2691): strip trailing whitespace.
    while matches!(
        value.last(),
        Some(ComponentValue::PreservedToken(Token::Whitespace))
    ) {
        value.pop();
    }

    let mut decl = Declaration {
        name,
        value,
        important,
        original_text: None,
    };

    // Step 8 (§5.5.6 L2693-2705): custom property original_text + top-
    // level {}-block validity check. We skip original_text capture for
    // now (requires source text tracking in TokenStream, deferred to a
    // future batch).
    if is_custom_property_name(&decl.name) {
        // §5.5.6 L2693-2698: original_text should be set. Deferred —
        // requires TokenStream to retain original source text. For now,
        // leave None.
    } else {
        // §5.5.6 L2700-2705: if value contains a top-level {}-block AND
        // any other non-whitespace value, return nothing.
        if has_top_level_curly_block_with_other_values(&decl.value) {
            return None;
        }
    }

    // §5.5.6 L2707-2712: unicode-range descriptor handling. Deferred —
    // requires TokenStream source text tracking for re-tokenization.

    Some(decl)
}

/// §5.5.6 (L2721-2741) Consume the remnants of a bad declaration.
///
/// Repeatedly process input:
/// - EOF or `;` → discard, return.
/// - `}-token`: if nested, return without consuming; else discard.
/// - anything else → consume a component value (discard result).
pub fn consume_the_remnants_of_a_bad_declaration(
    input: &mut TokenStream,
    nested: bool,
) {
    loop {
        match input.next_token() {
            Token::Eof | Token::Semicolon => {
                input.discard_token();
                return;
            }
            Token::CloseBrace => {
                if nested {
                    return;
                }
                input.discard_token();
            }
            _ => {
                let _ = consume_a_component_value(input);
            }
        }
    }
}

/// §5.5.6 step 6 (L2682-2687): If the last two non-whitespace values are
/// a `!` delim token followed by an `important` ident (ASCII
/// case-insensitive), remove them and return true.
fn strip_important(value: &mut Vec<ComponentValue>) -> bool {
    // Strip trailing whitespace first, look at last two non-ws.
    let mut end = value.len();
    while end > 0
        && matches!(
            value[end - 1],
            ComponentValue::PreservedToken(Token::Whitespace)
        )
    {
        end -= 1;
    }
    if end < 2 {
        return false;
    }
    let last = &value[end - 1];
    let prev = &value[end - 2];
    let is_important_ident = |v: &ComponentValue| match v {
        ComponentValue::PreservedToken(Token::Ident(s)) => s.eq_ignore_ascii_case("important"),
        _ => false,
    };
    let is_bang_delim = |v: &ComponentValue| matches!(
        v,
        ComponentValue::PreservedToken(Token::Delim('!'))
    );
    if is_bang_delim(prev) && is_important_ident(last) {
        // Truncate to end (remove trailing whitespace + ! + important).
        value.truncate(end - 2);
        return true;
    }
    false
}

/// §5.5.6 L2693: A "custom property name string" is an ident-token whose
/// value starts with "--" (two hyphens).
fn is_custom_property_name(name: &str) -> bool {
    name.starts_with("--")
}

/// §5.5.6 L2700-2705: A declaration value contains a top-level {}-block
/// AND any other non-whitespace value. (Only the whole value may be a
/// {}-block for non-custom properties.)
fn has_top_level_curly_block_with_other_values(value: &[ComponentValue]) -> bool {
    let mut has_curly_block = false;
    let mut has_other = false;
    for v in value {
        match v {
            ComponentValue::SimpleBlock(SimpleBlock { kind: BlockKind::Curly, .. }) => {
                has_curly_block = true;
            }
            ComponentValue::PreservedToken(Token::Whitespace) => {}
            _ => {
                has_other = true;
            }
        }
    }
    has_curly_block && has_other
}
```

**新增测试**（`tests/parser_algorithms_cp4.rs`）：8 个测试
- `declaration_basic` — `color: red;` → Declaration{name:"color", value:[Ident("red")], important:false}
- `declaration_without_semicolon_at_eof` — `color: red`（EOF 结束）→ 正常返回
- `declaration_no_colon_returns_none` — `color red;` → None，剩余被 remnants 吃掉
- `declaration_no_ident_returns_none` — `: red;` → None
- `declaration_important_flag` — `color: red !important;` → important=true，value=[Ident("red")]
- `declaration_important_case_insensitive` — `color: red !IMPORTANT;` → important=true
- `declaration_custom_property` — `--foo: bar;` → name="--foo"，is_custom_property_name=true
- `declaration_top_level_curly_with_other_returns_none` — `color: {} red;` → None（custom property 之外不允许 {} + 其他值）

**验证 + 提交**（每个 commit 必须依次跑 fmt / test / clippy -D warnings 全绿后才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
若有 fmt diff，先 `cargo fmt -p muskitty-css` 修格式，再跑后续检查。预期 103/103 green（95 + 8），零 warning，零 fmt diff。

**Commit**：
```
[css-parser] CP-4: 5.5.6 consume_a_declaration + remnants_of_bad_decl

- §5.5.6 (L2639-2717) consume_a_declaration: ident name + colon + list
  of component values (stop=`;`) + strip !important + strip trailing
  whitespace + custom property detection + top-level {}-block validity.
- §5.5.6 (L2721-2741) consume_the_remnants_of_a_bad_declaration: EOF/`;`
  discard+return; `}` nested=return / nested=false=discard; else consume
  component value.
- Custom property original_text capture deferred (requires source-text
  tracking in TokenStream; future batch).
- §5.5.6 L2707-2712 unicode-range descriptor handling deferred (requires
  source-text tracking).
- 8 new unit tests covering basic/no-colon/no-ident/important/case-
  insensitive/custom-property/top-level-{}-block.
```

---

### 阶段 5：CP-5 — §5.5.1-§5.5.5 Stylesheet / rule / block algorithms

**目标**：实现 §5.5 剩余 5 个 algorithms（上层结构）。这是 parser 的核心，最复杂的是 `consume_a_blocks_contents`，需要 mark/restore 处理 declaration/rule 二义性。

**规范依据**：§5.5.1-§5.5.5 L2223-2636。

**文件**：`d:\Muskitty\crates\muskitty-css\src\parser\algorithms.rs`（追加）。

**实现**：

```rust
/// §5.5.1 (L2223-2279) Consume a stylesheet's contents.
///
/// Repeatedly process:
/// - whitespace / CDO / CDC → discard.
/// - EOF → return rules.
/// - at-keyword → consume_an_at_rule; if Some, append.
/// - else → consume_a_qualified_rule; if Some(rule), append.
pub fn consume_a_stylesheets_contents(input: &mut TokenStream) -> Vec<Rule> {
    let mut rules = Vec::new();
    loop {
        match input.next_token() {
            Token::Whitespace | Token::Cdo | Token::Cdc => {
                input.discard_token();
            }
            Token::Eof => return rules,
            Token::AtKeyword(_) => {
                if let Some(rule) = consume_an_at_rule(input, false) {
                    rules.push(Rule::AtRule(rule));
                }
            }
            _ => {
                if let Some(rule) = consume_a_qualified_rule(input, None, false) {
                    rules.push(Rule::QualifiedRule(rule));
                }
            }
        }
    }
}

/// §5.5.2 (L2281-2337) Consume an at-rule.
///
/// `nested`: when true, unbalanced `}-token` returns the rule without
/// consuming (caller will close the block); when false, `}-token` is
/// part of the prelude.
pub fn consume_an_at_rule(
    input: &mut TokenStream,
    nested: bool,
) -> Option<AtRule> {
    let name = match input.consume_token() {
        Token::AtKeyword(name) => name,
        _ => unreachable!("consume_an_at_rule called on non-at-keyword"),
    };
    let mut rule = AtRule {
        name,
        prelude: Vec::new(),
        declarations: None,
        child_rules: None,
    };
    loop {
        match input.next_token() {
            Token::Semicolon | Token::Eof => {
                input.discard_token();
                return Some(rule);
            }
            Token::CloseBrace => {
                if nested {
                    return Some(rule);
                }
                let t = input.consume_token();
                rule.prelude.push(ComponentValue::PreservedToken(t));
            }
            Token::OpenBrace => {
                let block = consume_a_block(input);
                // §5.5.2 L2319-2320: result is a list of (rules or
                // declaration-lists). The first declaration-list (if any)
                // becomes rule.declarations; remaining become nested
                // declaration rules. We model block contents as:
                //   declarations: Option<Vec<Declaration>>
                //   child_rules: Option<Vec<Rule>>
                let (decls, rules) = split_block_contents(block);
                rule.declarations = Some(decls);
                rule.child_rules = Some(rules);
                return Some(rule);
            }
            _ => {
                rule.prelude.push(consume_a_component_value(input));
            }
        }
    }
}

/// §5.5.3 (L2340-2466) Consume a qualified rule.
///
/// `stop_token`: optional token that aborts (returns None). `nested`:
/// passed to consume_a_block's contents.
///
/// Returns:
/// - `Ok(Some(rule))` — rule successfully consumed.
/// - `Ok(None)` — "return nothing" (e.g. EOF or stop_token encountered).
/// - `Err(())` — "invalid rule error" (e.g. custom-property-in-prelude
///   at top level after consuming the block).
pub fn consume_a_qualified_rule(
    input: &mut TokenStream,
    stop_token: Option<Token>,
    nested: bool,
) -> Result<Option<QualifiedRule>, ()> {
    let mut rule = QualifiedRule {
        prelude: Vec::new(),
        declarations: Vec::new(),
        child_rules: Vec::new(),
    };
    loop {
        let next = input.next_token();
        match next {
            Token::Eof => return Ok(None), // §5.5.3 L2355-2359
            t if stop_token.as_ref().map_or(false, |s| *s == t) => {
                // §5.5.3 L2355-2359: parse error.
                return Ok(None);
            }
            Token::CloseBrace => {
                // §5.5.3 L2361-2368.
                if nested {
                    return Ok(None);
                }
                let t = input.consume_token();
                rule.prelude.push(ComponentValue::PreservedToken(t));
            }
            Token::OpenBrace => {
                // §5.5.3 L2370-2460.
                // §5.5.3 L2372-2383: check if prelude starts with
                // `--<ident>` + `:` (custom-property-like). If so:
                //   nested → consume remnants of bad declaration, return None.
                //   non-nested → consume a block, return None (invalid rule error).
                if looks_like_custom_property_in_prelude(&rule.prelude) {
                    if nested {
                        consume_the_remnants_of_a_bad_declaration(input, true);
                        return Ok(None);
                    } else {
                        let _ = consume_a_block(input);
                        return Err(()); // invalid rule error
                    }
                }
                let block = consume_a_block(input);
                let (decls, rules) = split_block_contents(block);
                rule.declarations = decls;
                rule.child_rules = rules;
                return Ok(Some(rule));
            }
            _ => {
                rule.prelude.push(consume_a_component_value(input));
            }
        }
    }
}

/// §5.5.4 (L2469-2484) Consume a block.
///
/// Precondition: next token is `{-token`. Discard it, consume block
/// contents, discard `}-token (or EOF), return the contents (a list of
/// rules or declaration-lists, modeled as `BlockContents`).
pub fn consume_a_block(input: &mut TokenStream) -> BlockContents {
    debug_assert!(matches!(input.next_token(), Token::OpenBrace));
    input.discard_token();
    let contents = consume_a_blocks_contents(input);
    // §5.5.4 L2481: discard the closing `}-token (or EOF if implicit).
    input.discard_token();
    contents
}

/// §5.5.5 (L2486-2636) Consume a block's contents.
///
/// Returns a list of rules and lists-of-declarations (modeled as
/// `BlockContents` carrying `Vec<Declaration>` and `Vec<Rule>`).
/// Algorithm:
/// - whitespace / `;` → discard.
/// - EOF / `}` → return.
/// - at-keyword → flush decls into rules, consume_an_at_rule(nested=true).
/// - else → mark; consume_a_declaration(nested=true); if Some(decl),
///   append to decls and discard mark. Otherwise restore mark, then
///   consume_a_qualified_rule(nested=true, stop=`;`); on Some(rule),
///   flush decls to rules, append rule; on invalid rule error, flush
///   decls to rules; on None, do nothing.
pub fn consume_a_blocks_contents(input: &mut TokenStream) -> BlockContents {
    let mut rules: Vec<Rule> = Vec::new();
    let mut decls: Vec<Declaration> = Vec::new();
    loop {
        match input.next_token() {
            Token::Whitespace | Token::Semicolon => {
                input.discard_token();
            }
            Token::Eof | Token::CloseBrace => return BlockContents { decls, rules },
            Token::AtKeyword(_) => {
                // §5.5.5 L2519-2528: at-keyword flushes decls.
                if !decls.is_empty() {
                    rules.push(Rule::Declarations(std::mem::take(&mut decls)));
                }
                if let Some(at_rule) = consume_an_at_rule(input, true) {
                    rules.push(Rule::AtRule(at_rule));
                }
            }
            _ => {
                // §5.5.5 L2530-2562: mark + declaration/rule ambiguity.
                input.mark();
                if let Some(decl) = consume_a_declaration(input, true) {
                    decls.push(decl);
                    input.discard_mark();
                } else {
                    input.restore_mark();
                    match consume_a_qualified_rule(input, Some(Token::Semicolon), true) {
                        Ok(Some(rule)) => {
                            // §5.5.5 L2556-2561: rule returned.
                            if !decls.is_empty() {
                                rules.push(Rule::Declarations(std::mem::take(&mut decls)));
                            }
                            rules.push(Rule::QualifiedRule(rule));
                        }
                        Ok(None) => {} // §5.5.5 L2546-2547: "If nothing was returned, do nothing."
                        Err(()) => {
                            // §5.5.5 L2549-2554: invalid rule error.
                            if !decls.is_empty() {
                                rules.push(Rule::Declarations(std::mem::take(&mut decls)));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Result of `consume_a_block` / `consume_a_blocks_contents`. Combines a
/// list of declarations (possibly empty) and a list of child rules.
#[derive(Debug, Clone, Default)]
pub struct BlockContents {
    pub decls: Vec<Declaration>,
    pub rules: Vec<Rule>,
}

/// Split block contents from CP-5's `consume_a_block` into the AtRule /
/// QualifiedRule's expected shape:
///   - declarations → `Some(decls)` for AtRule, `decls` field for QualifiedRule
///   - rules → `Some(rules)` for AtRule, `child_rules` field for QualifiedRule
fn split_block_contents(block: BlockContents) -> (Vec<Declaration>, Vec<Rule>) {
    (block.decls, block.rules)
}

/// §5.5.3 L2372-2383: Detect whether a qualified rule's prelude looks
/// like `--<ident> :` (a custom property declaration masquerading as a
/// rule). The first two non-whitespace tokens of the prelude must be:
///   - Ident starting with "--"
///   - Colon
fn looks_like_custom_property_in_prelude(prelude: &[ComponentValue]) -> bool {
    let mut iter = prelude.iter().filter(|v| !matches!(
        v,
        ComponentValue::PreservedToken(Token::Whitespace)
    ));
    let first = iter.next();
    let second = iter.next();
    matches!(
        (first, second),
        (
            Some(ComponentValue::PreservedToken(Token::Ident(name))),
            Some(ComponentValue::PreservedToken(Token::Colon))
        ) if name.starts_with("--")
    )
}
```

**对 `consume_a_qualified_rule` 返回类型的调整**：

规范 L2460 区分 "return nothing" (Ok(None)) 和 "return an invalid rule error" (Err)。最简实现：

```rust
type QualifiedRuleResult = Result<Option<QualifiedRule>, ()>;
// Ok(Some) → rule 返回
// Ok(None) → "return nothing" (无内容)
// Err(()) → "invalid rule error"
```

**新增测试**（`tests/parser_algorithms_cp5.rs`）：12 个测试
- `stylesheet_empty_input` — `""` → 空 rules
- `stylesheet_single_rule` — `a {} ` → [QualifiedRule(prelude=[Ident("a")], decls=[])]
- `stylesheet_at_rule_statement` — `@import "x";` → [AtRule(name="import", prelude=[String("x")], decls=None, rules=None)]
- `stylesheet_at_rule_block` — `@media print { a {} }` → [AtRule(name="media", prelude=[Ident("print")], child_rules=[QualifiedRule(a)])]
- `stylesheet_cdo_cdc_discarded` — `<!-- a {} -->` → [QualifiedRule(a)]
- `at_rule_nested_close_brace_returns` — nested=true，遇 `}` 返回不消费
- `qualified_rule_custom_property_in_prelude_nested` — nested=true，`--foo:hover { ... }` → consume remnants + Ok(None)
- `qualified_rule_custom_property_in_prelude_top_level` — nested=false，`--foo:hover { ... }` → consume block + Err(())
- `block_contents_mixed_decls_and_rules` — `color: red; a {}` → decls=[color:red], rules=[QualifiedRule(a)]
- `block_contents_at_rule_flushes_decls` — `color: red; @media {}` → decls=[], rules=[Declarations([color:red]), AtRule(media)]
- `block_contents_invalid_decl_then_rule_restores_mark` — `font+ 1; a {}` → decls=[], rules=[QualifiedRule(a)]
- `block_contents_only_decls` — `color: red; font: 16px;` → decls=[2 项], rules=[]

**验证 + 提交**（每个 commit 必须依次跑 fmt / test / clippy -D warnings 全绿后才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
若有 fmt diff，先 `cargo fmt -p muskitty-css` 修格式，再跑后续检查。预期 115/115 green（103 + 12），零 warning，零 fmt diff。

**Commit**：
```
[css-parser] CP-5: 5.5.1-5.5.5 stylesheet/rule/block algorithms

- §5.5.1 (L2223-2279) consume_a_stylesheets_contents: ws/CDO/CDC
  discard; at-keyword → at-rule; else → qualified rule.
- §5.5.2 (L2281-2337) consume_an_at_rule: name from at-keyword; prelude
  component values until `;`/EOF/`{`/`}`. Block at-rules split into
  declarations + child_rules.
- §5.5.3 (L2340-2466) consume_a_qualified_rule: prelude + `{`-block;
  custom-property-in-prelude detection (nested → remnants + Ok(None),
  top-level → consume block + Err(())); returns Result<Option<_>, ()>
  for tri-state (rule / nothing / invalid rule error).
- §5.5.4 (L2469-2484) consume_a_block: discard `{`, consume contents,
  discard `}` (or EOF).
- §5.5.5 (L2486-2636) consume_a_blocks_contents: ws/`;` discard;
  at-keyword flushes decls + consume at-rule; else mark+decl → restore+
  qualified rule (nested=true, stop=`;`). Handles decl/rule ambiguity.
- Rule enum's Declarations variant (CP-1) used for §5.5.5's mixed list.
- 12 new unit tests.
```

---

### 阶段 6：CP-6 — §5.4 Parser Entry Points

**目标**：实现 §5.4 的 9 个 entry points（不含 §5.4.1 / §5.4.2 通用 grammar hook，它们需要 grammar 知识，留到 Selectors / Values 阶段）。

**规范依据**：§5.4 L1816-2206。

**文件**：`d:\Muskitty\crates\muskitty-css\src\parser\entry_points.rs`（新）+ `mod.rs`（加 `mod entry_points;`）。

**实现**：

```rust
//! §5.4 Parser Entry Points.
//!
//! Nine entry points producing high-level CSS objects from input. The
//! grammar-based hooks §5.4.1 (`parse something according to a CSS
//! grammar`, L1895-1944) and §5.4.2 (`parse a comma-separated list
//! according to a CSS grammar`, L1949-2001) are deferred — they require
//! grammar knowledge from Selectors / Values specs.

use super::algorithms::{
    consume_a_blocks_contents, consume_a_component_value,
    consume_a_declaration, consume_a_list_of_component_values,
    consume_a_qualified_rule, consume_a_stylesheets_contents,
    consume_an_at_rule,
};
use super::token_stream::TokenStream;
use super::types::{ComponentValue, Declaration, Rule, Stylesheet};
use crate::tokenizer::{CssTokenizer, Token, Tokenizer};

/// §5.4 L1827-1842: Normalize into a token stream.
///
/// Accepts a string (tokenize after §5.3 preprocessing) or a list of
/// tokens. For now we only expose the string form; the Vec<Token> form
/// is internal.
fn normalize_from_string(input: &str) -> TokenStream {
    let mut tz = CssTokenizer::new(input);
    let mut tokens = Vec::new();
    while let Some(token) = tz.next_token() {
        tokens.push(token);
        if matches!(token, Token::Eof) {
            break;
        }
    }
    TokenStream::new(tokens)
}

/// §5.4.3 (L2005-2033) Parse a stylesheet.
///
/// Tokenize → consume_a_stylesheets_contents → wrap in Stylesheet.
pub fn parse_a_stylesheet(input: &str) -> Stylesheet {
    let mut stream = normalize_from_string(input);
    let rules = consume_a_stylesheets_contents(&mut stream);
    Stylesheet { rules }
}

/// §5.4.4 (L2037-2051) Parse a stylesheet's contents.
pub fn parse_a_stylesheets_contents(input: &str) -> Vec<Rule> {
    let mut stream = normalize_from_string(input);
    consume_a_stylesheets_contents(&mut stream)
}

/// §5.4.5 (L2055-2069) Parse a block's contents.
pub fn parse_a_blocks_contents(input: &str) -> super::algorithms::BlockContents {
    let mut stream = normalize_from_string(input);
    consume_a_blocks_contents(&mut stream)
}

/// §5.4.6 (L2073-2109) Parse a rule.
///
/// Discard ws; if EOF → syntax error (None); if at-keyword → consume
/// at-rule; else consume qualified rule. Discard ws; if EOF → return
/// rule; else syntax error (None).
pub fn parse_a_rule(input: &str) -> Option<Rule> {
    let mut stream = normalize_from_string(input);
    stream.discard_whitespace();
    let rule = match stream.next_token() {
        Token::Eof => return None,
        Token::AtKeyword(_) => consume_an_at_rule(&mut stream, false).map(Rule::AtRule),
        _ => consume_a_qualified_rule(&mut stream, None, false)
            .ok()
            .flatten()
            .map(Rule::QualifiedRule),
    };
    stream.discard_whitespace();
    if stream.is_empty() {
        rule
    } else {
        None
    }
}

/// §5.4.7 (L2113-2134) Parse a declaration.
pub fn parse_a_declaration(input: &str) -> Option<Declaration> {
    let mut stream = normalize_from_string(input);
    stream.discard_whitespace();
    consume_a_declaration(&mut stream, false)
}

/// §5.4.8 (L2138-2168) Parse a component value.
pub fn parse_a_component_value(input: &str) -> Option<ComponentValue> {
    let mut stream = normalize_from_string(input);
    stream.discard_whitespace();
    if stream.is_empty() {
        return None;
    }
    let value = consume_a_component_value(&mut stream);
    stream.discard_whitespace();
    if stream.is_empty() {
        Some(value)
    } else {
        None
    }
}

/// §5.4.9 (L2172-2183) Parse a list of component values.
pub fn parse_a_list_of_component_values(input: &str) -> Vec<ComponentValue> {
    let mut stream = normalize_from_string(input);
    consume_a_list_of_component_values(&mut stream, None, false)
}

/// §5.4.10 (L2186-2204) Parse a comma-separated list of component values.
pub fn parse_a_comma_separated_list_of_component_values(
    input: &str,
) -> Vec<Vec<ComponentValue>> {
    let mut stream = normalize_from_string(input);
    let mut groups = Vec::new();
    while !stream.is_empty() {
        let group = consume_a_list_of_component_values(
            &mut stream,
            Some(Token::Comma),
            false,
        );
        groups.push(group);
        stream.discard_token(); // discard the comma
    }
    groups
}
```

**新增测试**（`tests/parser_entry_points.rs`）：10 个测试
- `parse_a_stylesheet_simple` — `"a { color: red; }"` → Stylesheet{rules:[QualifiedRule(...)]}
- `parse_a_stylesheets_contents_returns_vec` — `"a {} b {}"` → 2 个 QualifiedRule
- `parse_a_blocks_contents_basic` — `"color: red; font: 16px;"` → BlockContents{decls:[2], rules:[]}
- `parse_a_rule_at_rule` — `"@media print {}"` → Some(AtRule)
- `parse_a_rule_qualified_rule` — `"a {}"` → Some(QualifiedRule)
- `parse_a_rule_eof_returns_none` — `""` → None
- `parse_a_rule_trailing_garbage_returns_none` — `"a {} b"` → None（解析完 a 后还有 b）
- `parse_a_declaration_simple` — `"color: red"` → Some(Declaration)
- `parse_a_component_value_simple` — `"red"` → Some(PreservedToken(Ident("red")))
- `parse_a_comma_separated_list_basic` — `"a, b, c"` → [[a], [b], [c]]

**验证 + 提交**（每个 commit 必须依次跑 fmt / test / clippy -D warnings 全绿后才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
若有 fmt diff，先 `cargo fmt -p muskitty-css` 修格式，再跑后续检查。预期 125/125 green（115 + 10），零 warning，零 fmt diff。

**Commit**：
```
[css-parser] CP-6: 5.4 Parser Entry Points (9 of 10)

- §5.4.3 (L2005-2033) parse_a_stylesheet.
- §5.4.4 (L2037-2051) parse_a_stylesheets_contents.
- §5.4.5 (L2055-2069) parse_a_blocks_contents.
- §5.4.6 (L2073-2109) parse_a_rule (syntax error = None).
- §5.4.7 (L2113-2134) parse_a_declaration.
- §5.4.8 (L2138-2168) parse_a_component_value.
- §5.4.9 (L2172-2183) parse_a_list_of_component_values.
- §5.4.10 (L2186-2204) parse_a_comma_separated_list_of_component_values.
- §5.4.1 / §5.4.2 (grammar-based hooks) deferred — require Selectors /
  Values grammar knowledge.
- normalize_from_string helper (§5.4 L1827-1842).
- 10 new unit tests.
```

---

### 阶段 7：CP-7 — 顶层 API + lib.rs 整合

**目标**：在 `lib.rs` 暴露稳定的顶层函数 API，更新 crate-level doc。

**文件**：
- `d:\Muskitty\crates\muskitty-css\src\lib.rs`（更新）
- `d:\Muskitty\crates\muskitty-css\src\parser\mod.rs`（更新 pub use）

**lib.rs 更新**：

```rust
//! MusKitty CSS Parser
//!
//! Implements the CSS Syntax Module Level 3 tokenization and parsing
//! algorithms.
//!
//! # Architecture
//!
//! The parser follows the two-stage model described in CSS Syntax §3.1:
//! 1. **Tokenization** ([`tokenizer`]) — consumes a stream of Unicode
//!    code points and emits tokens (§4.3, fully implemented).
//! 2. **Parsing** ([`parser`]) — consumes tokens and produces CSS
//!    objects: stylesheets, rules, declarations, component values
//!    (§5, fully implemented).
//!
//! # Top-level API
//!
//! - [`parse_stylesheet`] — full stylesheet parse (§5.4.3)
//! - [`parse_rule`] — single rule parse (§5.4.6)
//! - [`parse_declaration`] — single declaration parse (§5.4.7)
//! - [`tokenize`] — token stream only (§4.3)
//!
//! # References
//!
//! - CSS Syntax Module Level 3: <https://drafts.csswg.org/css-syntax-3/>
//! - WPT CSS test suite: <https://github.com/web-platform-tests/wpt/tree/master/css>

pub mod parser;
pub mod tokenizer;

use crate::parser::entry_points::{
    parse_a_blocks_contents, parse_a_component_value,
    parse_a_comma_separated_list_of_component_values,
    parse_a_declaration, parse_a_list_of_component_values, parse_a_rule,
    parse_a_stylesheets_contents, parse_a_stylesheet,
};
use crate::parser::types::{ComponentValue, Declaration, Rule, Stylesheet};
use crate::tokenizer::{CssTokenizer, Token, Tokenizer};

/// Tokenize a CSS input string into a vector of tokens (§4.3).
///
/// Same as before; unchanged.
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
///
/// Implements `parse a stylesheet` from CSS Syntax §5.4.3: tokenize
/// the input (with §5.3 preprocessing), then `consume a stylesheet's
/// contents` (§5.5.1) to produce the list of rules.
///
/// # Example
///
/// ```
/// use muskitty_css::parse_stylesheet;
///
/// let ss = parse_stylesheet("a { color: red; }");
/// assert_eq!(ss.rules.len(), 1);
/// ```
pub fn parse_stylesheet(input: &str) -> Stylesheet {
    parse_a_stylesheet(input)
}

/// Parse a CSS string into a single [`Rule`] (§5.4.6).
///
/// Returns `None` for a syntax error (empty input or trailing garbage).
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

/// Parse a CSS string into a comma-separated list of [`ComponentValue`]
/// (§5.4.10).
pub fn parse_comma_separated_list_of_component_values(
    input: &str,
) -> Vec<Vec<ComponentValue>> {
    parse_a_comma_separated_list_of_component_values(input)
}
```

**新增测试**（`tests/top_level_api.rs`）：5 个 doctest + integration
- 验证 `parse_stylesheet("a { color: red; }")` 返回 1 个 QualifiedRule
- 验证 `parse_rule("@media print {}")` 返回 Some
- 验证 `parse_declaration("color: red")` 返回 Some
- 验证 `parse_component_value("red")` 返回 Some
- 验证 `parse_comma_separated_list_of_component_values("a, b, c")` 返回 3 组

**验证 + 提交**（每个 commit 必须依次跑 fmt / test / clippy -D warnings 全绿后才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
若有 fmt diff，先 `cargo fmt -p muskitty-css` 修格式，再跑后续检查。预期 130/130 green（125 + 5），零 warning，零 fmt diff。

**Commit**：
```
[css-parser] CP-7: lib.rs top-level API + crate-level doc

- Expose parse_stylesheet / parse_rule / parse_declaration /
  parse_component_value / parse_list_of_component_values /
  parse_comma_separated_list_of_component_values as top-level fns.
- Update crate-level doc: architecture now describes §4.3 tokenizer
  + §5 parser (both fully implemented).
- 5 new doctest/integration tests.
```

---

### 阶段 8：CP-8 — clippy / fmt / coverage 整合

**目标**：最终清理，准备首版发布到 crates.io。fmt / clippy / test 已在每个 CP-1..CP-7 commit 前跑过；CP-8 重点是覆盖率复查 + Doc 复查 + 版本元数据 + 文档。

**文件**：所有。

**清单**：

1. **重跑 fmt + clippy + test + check**（一致性确认）：
   ```powershell
   cargo fmt -p muskitty-css -- --check
   cargo test -p muskitty-css
   cargo check -p muskitty-css
   cargo clippy -p muskitty-css --all-targets -- -D warnings
   ```
   预期全部通过（fmt 0 diff，test 130/130 green，clippy 0 warning）。若有 diff/warning，必须真正修掉，不允许 `#[allow]` 关闭。

2. **覆盖率复查**：
   - 每个公开函数至少 1 个 happy-path + 1 个 error-path 测试。
   - 边界场景全覆盖：EOF、未闭合块、空输入、嵌套 nested=true。
   - `consume_a_blocks_contents` 是最复杂的，需要额外测试覆盖 mark/restore 路径（已在 CP-5 加 12 个测试，CP-8 复查是否覆盖到所有路径）。

3. **Doc 复查**：
   - 每个 pub fn 有 doc comment 引用 §章节号 + Markdown 行号。
   - 每个 pub struct/enum 有 doc comment。
   - crate-level doc 反映 §5 完整覆盖。

4. **更新 PROGRESS.md**：
   - 标记 Phase 2 子阶段 1（CSS 语法 tokenizer + parser）完成。
   - 添加 §5.x 完成情况表。
   - 更新下一步指向子阶段 2（Selectors Level 4）。

5. **Cargo.toml 升版本**：
   - `version = "0.1.0"` → `version = "0.2.0"`（CSSOM 数据结构稳定，新增 parser 是 minor bump）。
   - 加上 `description` / `repository` / `homepage` / `documentation` / `keywords` / `categories` / `authors` / `license` / `rust-version` 字段（参考 muskitty-html5-parser 的 Cargo.toml）。

6. **README 起草**（如果 muskitty-css 还没有 README）：
   - 标题 + 状态表（§4.3 + §5 完成度）
   - 安装方法（`cargo add muskitty-css`）
   - Quick Start 示例
   - Architecture 简介
   - 规范引用

**验证 + 提交**（与其他 CP 同样四步全绿才提交）：
```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```
预期 130/130 green，0 warning，0 fmt diff。

**Commit**：
```
[css-parser] CP-8: cleanup + docs + bump to v0.2.0

- Re-ran fmt/test/check/clippy -D warnings: all green (130/130 tests).
- Coverage review: every public function has happy-path + error-path
  tests; consume_a_blocks_contents mark/restore paths covered.
- Doc review: every pub fn/struct/enum references §section + Markdown
  line numbers (D:\CSSWG\css-syntax-3\Overview.md).
- Cargo.toml bumped to v0.2.0; added publish metadata (description,
  repository, homepage, documentation, keywords, categories, authors,
  license, rust-version=1.70).
- README.md drafted with status table + install + quick start.
- PROGRESS.md updated: Phase 2 sub-stage 1 (CSS Syntax Module §4.3 +
  §5) marked complete; next step sub-stage 2 (Selectors Level 4).
```

## 假设与决策

1. **规范源**：`D:\CSSWG\css-syntax-3\Overview.md`（Markdown 版本，2999 行）。一切以标准为准，每个 commit message 引用 §章节号与 Markdown L 行号。
2. **不修改 tokenizer**：§4.3 已完整且测试全绿。CP-1..CP-8 只新增 `src/parser/` 模块，不动 `src/tokenizer/`。
3. **`Rule::Declarations` variant**：§5.5.5 的输出是"rules 和 declaration-lists 的混合 list"。CP-1 在 `Rule` enum 加 `Declarations(Vec<Declaration>)` variant 来精确建模。后续 CSSOM 可以将其 materialize 为 `CSSStyleDeclaration` 或 `CSSNestedDeclarations`。
4. **`consume_a_qualified_rule` 返回类型**：用 `Result<Option<QualifiedRule>, ()>` 区分三种情况：`Ok(Some)` = rule；`Ok(None)` = "return nothing"（无内容）；`Err(())` = "invalid rule error"。`()` 错误类型足够，CP-8 之后可视需要替换为具体错误 enum。
5. **§5.3 `process` 操作**：规范用 dispatch table，Rust 闭包等价。CP-2 不暴露 `process` 作为公共 API，CP-3+ 的算法直接用 `loop { match input.next_token() {...} }` 实现。语义等价，避免 enum-dispatch 复杂度。
6. **Custom property `original_text` 捕获**：CP-4 的 `consume_a_declaration` 暂不实现 `original_text` 捕获。原因：需要 `TokenStream` 保留原始 source text 与 token range 映射，这是 TokenStream 的扩展。CP-4 在代码中留 TODO 注释，后续可视 var() 实现需求补充。
7. **§5.4.1 / §5.4.2 grammar-based hooks 延后**：这两个 entry points 需要 Selectors / Values 的 grammar 知识。CP-6 不实现它们，留到 Selectors 阶段（Phase 2 子阶段 2）需要时再补。
8. **§5.5.6 `unicode-range` descriptor 处理延后**：规范 L2707-2712 要求 `consume_a_declaration` 在解析 `unicode-range` descriptor 时用 `consume_the_value_of_a_unicode_range_descriptor` 重新处理 value。这需要 TokenStream 保留原始 source text 用于 re-tokenization。CP-4 在代码中留 TODO 注释。
9. **错误类型**：当前用 `Option<T>` 和 `Result<T, ()>` 表示 parse errors。具体错误信息（位置、类型）推迟到 CP-8 之后，视 CSSOM 需求再设计。
10. **不修改任何测试用例**：所有测试期望严格按规范推导。任何失败都视为实现 bug，不改测试。
11. **逐提交 + 每提交必跑 fmt/test/clippy**：CP-1 → CP-8，每个阶段独立 commit。**每个 commit 提交前必须依次跑** `cargo fmt --check` / `cargo test` / `cargo check` / `cargo clippy --all-targets -- -D warnings` 四步全绿。fmt 有 diff 就先 `cargo fmt` 修格式再跑后续；clippy warning 必须真正修掉，不允许用 `#[allow]` 关闭。CP-8 是收尾 commit（升版本 + docs + PROGRESS.md），不再额外跑 fmt 但仍跑 test + clippy + check。
12. **首版发布计划**：CP-8 完成后，muskitty-css 首版发布到 crates.io v0.2.0，与 muskitty-dom v0.1.0 / muskitty-html5-tokenizer v0.1.2 / muskitty-html5-parser v0.1.2 并列。

## 验证步骤（每阶段通用，每个 commit 必跑）

每个 CP 完成代码后，**必须依次**执行下面四步，全部通过才能 `git commit`：

```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```

若 `cargo fmt -- --check` 报 diff，先 `cargo fmt -p muskitty-css` 修格式，再重新跑后续检查。**禁止**为了通过 fmt 而关闭某项 clippy lint；clippy warning 必须真正修掉。CP-8 收尾时不再额外跑 fmt（前 7 个 CP 已经保证），但仍需跑 test + clippy + check 确认整体一致。

每个 CP 完成后由用户执行 `git push`（沿用既有约定）。

## 完成判定

CP-8 完成后，muskitty-css 应满足：
1. §4.3 tokenizer + §5 parser 完整实现。
2. ≥130 个单元测试全绿。
3. clippy 0 warning，fmt 0 diff。
4. 顶层 API `parse_stylesheet` / `parse_rule` / `parse_declaration` / `parse_component_value` / `parse_list_of_component_values` / `parse_comma_separated_list_of_component_values` 全部可用。
5. 可解析典型 CSS（`a { color: red; } @media print { body {} }` 等）产出正确的 `Stylesheet` 结构。
6. v0.2.0 可发布到 crates.io。

## 后续路线

CP-8 完成后，muskitty-css 进入 Phase 2 子阶段 2：**Selectors Level 4**。建立 `crates/muskitty-selectors`（或 `muskitty-css/selectors/` 子模块）实现简单选择器 / 组合器 / 伪类 / 伪元素 / 匹配引擎。子阶段 1（CSS Syntax）完成后，parser 产出的 QualifiedRule 的 prelude 可以喂给 Selectors parser 解析为 selector AST。

**自动化衔接**：本计划全部 8 个 CP commit 完成并 push 后，**自动切换为 plan 模式**，开始 Phase 2 子阶段 2（Selectors Level 4）的计划起草。无需用户手动触发模式切换 — 在 CP-8 commit 完成的同一回合里，立刻进入 plan 模式，按 D:\CSSWG\selectors-4\Overview.md（或对应规范目录）做依赖分析与批次拆分，产出新的 `.trae/documents/phase2-selectors-cp1-to-cpN.md` 计划文档，再通过 NotifyUser 请求评审。
