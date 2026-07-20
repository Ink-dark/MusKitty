# Phase 2 启动计划：muskitty-css crate 骨架 + CSS Syntax Module §5 tokenizer

> 创建日期：2026-07-17
> 状态：待用户批准
> 关联文档：[PROGRESS.md](file:///d:/Muskitty/PROGRESS.md) §"Phase 2 规划"、[roadmap](file:///d:/Muskitty/.trae/documents/muskitty-browser-roadmap.md) §四 Layer 2

## 摘要

按 [PROGRESS.md](file:///d:/Muskitty/PROGRESS.md) Phase 2 子阶段 1，建立 `crates/muskitty-css/` 独立 crate 骨架，实现 CSS Syntax Module Level 3 §5 "Tokenizer" 的完整状态机。这是 Phase 2 的第一个 commit batch，对齐 [muskitty-html5-parser tokenizer 阶段](file:///d:/Muskitty/crates/muskitty-html5-parser/src/tokenizer/) 的工程模板。

本计划**仅覆盖子阶段 1（CSS Syntax tokenizer）**。子阶段 2–5（选择器 / 值解析 / CSSOM / Cascade）后续各自独立规划。

## 当前状态分析

### 已就位（Phase 1 收尾）

- **HTML 解析层 100%**：tokenizer 99.8% (7022/7036)，tree construction 100% (1716/1716)
- **muskitty-dom crate**：`Rc<RefCell<Node>>` 模型，`ElementData` / `Attribute` / `Namespace` 类型已就位（[src/lib.rs](file:///d:/Muskitty/crates/muskitty-dom/src/lib.rs)）
- **Phase 2 入场门槛已满足**：Layer 1 通过率 ≥80% ✅，DOM Core API 完整 ✅
- **工程模板可复刻**：[tokenizer/](file:///d:/Muskitty/crates/muskitty-html5-parser/src/tokenizer/) 5 文件结构（mod.rs / types.rs / trait_def.rs / impls.rs / entities.rs），trait+impl 分离，枚举穷尽性 = 规范完整性

### 缺失

- **`crates/muskitty-css/` 目录不存在**（workspace Cargo.toml 仅注释预留）
- **CSS 规范不在 `D:\whatwg`**：CSSWG 规范（css-syntax-3 等）由 drafts.csswg.org 维护，不在 WHATWG 镜像范围内。实现时需在线参考 <https://drafts.csswg.org/css-syntax-3/> 或预先拉取
- **WPT CSS 测试 fixtures 空白**：项目内无任何 `*.css` 测试文件，需从 <https://github.com/web-platform-tests/wpt/tree/master/css> 拉取 css-syntax 子集
- **DOM API 表面需确认**：CSS 选择器匹配引擎需要 `ElementData::tag_name` / `ElementData::attributes` / `ElementData::namespace` 等字段，需读取 [element.rs](file:///d:/Muskitty/crates/muskitty-dom/src/element.rs) 确认（子阶段 2 的事，本计划不涉及）

### CSS Syntax Module §5 规范要点

规范：<https://drafts.csswg.org/css-syntax-3/#tokenization>

§5 Tokenizer 核心产出：
- **输入预处理**（§5.3）：CR + LF + FF 统一规整为 LF
- **Token 种类**（§5.1）：ident / function / at-keyword / hash / string / url / number / percentage / dimension / unicode-range / whitespace / comment / colon / semicolon / comma / delim / bracket 系列等
- **状态机**（§5.4）：约 14 个 tokenizer state（DataState 暴露给上层 Parser）
- **算法原语**（§5.2 + §4.3）：consume an input code point / consume a name / consume a numeric / consume an escaped code point / consume the remnants of a bad url 等
- **EOF 处理**：token 流以 `<EOF-token>` 结尾

## 提议改动

### 1. 创建 crate 骨架

**新建文件**：

| 文件 | 内容 | 模板参考 |
|------|------|---------|
| [crates/muskitty-css/Cargo.toml](file:///d:/Muskitty/crates/muskitty-css/Cargo.toml) | `name = "muskitty-css"`，edition 2021，零运行时依赖（dev-deps: serde_json） | [html-parser/Cargo.toml](file:///d:/Muskitty/crates/muskitty-html5-parser/Cargo.toml) |
| [crates/muskitty-css/.gitignore](file:///d:/Muskitty/crates/muskitty-css/.gitignore) | `target/` | 同上 |
| [crates/muskitty-css/src/lib.rs](file:///d:/Muskitty-css/src/lib.rs) | crate root，`pub mod tokenizer;` + 顶层 `parse_stylesheet(input: &str)` 入口（暂返回 `Vec<Token>`） | [html-parser/src/lib.rs](file:///d:/Muskitty/crates/muskitty-html5-parser/src/lib.rs) |

**修改文件**：

| 文件 | 改动 |
|------|------|
| [Cargo.toml](file:///d:/Muskitty/Cargo.toml) | workspace `members` 取消注释 `"crates/muskitty-css"` |

### 2. CSS Syntax tokenizer 模块（§5）

**新建文件**（复刻 tokenizer/ 5 文件结构）：

| 文件 | 内容 | 规范依据 |
|------|------|---------|
| [crates/muskitty-css/src/tokenizer/mod.rs](file:///d:/Muskitty/crates/muskitty-css/src/tokenizer/mod.rs) | 模块声明 + `pub use` re-export | — |
| [crates/muskitty-css/src/tokenizer/types.rs](file:///d:/Muskitty/crates/muskitty-css/src/tokenizer/types.rs) | `Token` enum（§5.1 全部 token 变体，含 `<EOF-token>`）+ `State` enum（§5.4 全部 tokenizer state）+ 辅助类型（`HashType`、`NumberType`、`Numeric`、`Position`） | §5.1, §5.4 |
| [crates/muskitty-css/src/tokenizer/trait_def.rs](file:///d:/Muskitty/crates/muskitty-css/src/tokenizer/trait_def.rs) | `Tokenizer` trait：`next_token() -> Option<Token>` + `state()` / `set_state()` / `reset()` | §5 |
| [crates/muskitty-css/src/tokenizer/impls.rs](file:///d:/Muskitty/crates/muskitty-css/src/tokenizer/impls.rs) | `CssTokenizer` struct + 状态机实现 + §4.3/§5.2 算法原语（consume_name / consume_numeric / consume_escaped / consume_bad_url_remnants） | §5.4, §4.3, §5.2 |

**Token enum 变体**（穷尽 CSS Syntax §5.1）：

```rust
pub enum Token {
    Ident(String),
    Function(String),                    // §5.1: name + "("
    AtKeyword(String),                   // §5.1: "@" + name
    Hash(String, HashType),              // §5.1
    String(String),                      // §5.1: quoted
    BadString,                           // §5.1
    Url(String),                         // §5.1: unquoted url
    BadUrl,                              // §5.1
    Delim(char),                         // §5.1
    Number(Numeric),                     // §5.1
    Percentage(Numeric),                 // §5.1
    Dimension(Numeric, String),          // §5.1: number + unit
    UnicodeRange(Option<u32>, Option<u32>),  // §5.1: start, end
    Whitespace,                          // §5.1
    Comment(String),                     // §5.1
    Colon, Semicolon, Comma,             // §5.1
    OpenBracket, CloseBracket,           // [ ]
    OpenParen, CloseParen,               // ( )
    OpenBrace, CloseBrace,               // { }
    Eof,                                 // <EOF-token>
}
```

派生 `Debug, Clone, PartialEq`（对齐 html-parser 约定，无 `Eq` 因含 `String`）。

**State enum 变体**（穷尽 CSS Syntax §5.4）：

```rust
pub enum State {
    Data,                                // §5.4.1
    Ident,                               // §5.4.2
    Function,                            // §5.4.3
    AtKeyword,                           // §5.4.4
    Hash,                                // §5.4.5
    String,                              // §5.4.6 (with string_quote)
    Url,                                 // §5.4.9
    UrlBadEscape,                        // §5.4.10
    Number,                              // §5.4.11 (内部，由 Data 转移)
    NumberRest,                          // §5.4.12
    NumberFraction,                      // §5.4.13
    Dimension,                           // §5.4.14
    SciNotation,                         // §5.4.15
    UnicodeRange,                        // §5.4.16
    UnicodeRangeRest,                    // §5.4.17
    UnicodeRangeEnd,                     // §5.4.18
}
```

派生 `Debug, Clone, Copy, PartialEq, Eq`（对齐 html-parser State 约定）。

**CssTokenizer struct**（参考 [html-parser impls.rs](file:///d:/Muskitty/crates/muskitty-html5-parser/src/tokenizer/impls.rs) 字段组织）：

```rust
pub struct CssTokenizer {
    input: Vec<char>,            // 输入码点流（§5.3 预处理后）
    pos: usize,
    state: State,
    eof_emitted: bool,
    reconsume: bool,             // §5.4 "reconsume" 约定
    pending_tokens: Vec<Token>,  // 单次 transition 多 token 发射队列
    string_quote: Option<char>,  // §5.4.6 当前字符串引号（' or "）
    // §5.4.5 HashType 跟踪
    // §5.4.11–§5.4.15 Numeric 累积字段
}
```

### 3. 状态机实现策略

按 CSS Syntax §5.4 状态顺序实现，每个状态一个 handler 函数，匹配 [html-parser impls.rs](file:///d:/Muskitty/crates/muskitty-html5-parser/src/tokenizer/impls.rs) 的 `handle_xxx_state` 命名约定。

**实现顺序**（每批独立 commit，对齐 P3.x 节奏）：

| 批次 | 状态 | 规范 |
|------|------|------|
| C-1 | Data state + 基础 delim/bracket/colon/semicolon/comma/whitespace | §5.4.1 |
| C-2 | Ident + Function + AtKeyword + Hash | §5.4.2–§5.4.5 |
| C-3 | String + BadString | §5.4.6–§5.4.8 |
| C-4 | Number + Percentage + Dimension + SciNotation | §5.4.11–§5.4.15 |
| C-5 | Url + BadUrl + UrlBadEscape | §5.4.9–§5.4.10 |
| C-6 | UnicodeRange 系列 | §5.4.16–§5.4.18 |
| C-7 | 算法原语（consume_escaped / consume_name / consume_numeric / consume_bad_url_remnants）回填 | §4.3 / §5.2 |

**注意**：算法原语实际在各状态实现中已经需要，C-7 批次是对已完成代码的整理与 doc 对齐，不是后置实现。

### 4. 测试 harness

**新建文件**：

| 文件 | 内容 |
|------|------|
| [crates/muskitty-css/tests/css_syntax_tokenizer.rs](file:///d:/Muskitty/crates/muskitty-css/tests/css_syntax_tokenizer.rs) | 内联单元测试 + 整合测试，对齐 [html5lib_tokenizer.rs](file:///d:/Muskitty/crates/muskitty-html5-parser/tests/html5lib_tokenizer.rs) 的 harness 形态 |

**测试数据**：

CSS Syntax 没有像 html5lib 那样的官方 `.test` fixture，但 WPT `css/css-syntax/parsing/` 下有大量 `.html` + `.ini` 形态的测试。**本阶段先用内联单元测试覆盖每个状态的主要路径**，WPT fixtures 拉取推迟到 C-7 之后单独一个 commit（避免阻塞状态机实现）。

测试分类（对齐 html5lib_gap_report.md 结构）：
- Data state：基础码点、whitespace、delim、EOF
- Ident：合法标识符、`-` 开头、转义、非 ASCII
- Function：`name(` 形态
- AtKeyword：`@name` 形态
- Hash：`#id`、`#hex`、`#unknown`
- String：双引号 / 单引号 / 转义 / 换行 / BadString
- Number：整数 / 小数 / 负数 / 科学计数法
- Dimension / Percentage：数字 + 单位
- Url：`url(...)` 合法 + BadUrl
- UnicodeRange：`U+1234` / `U+12-34` / `U+12?`

### 5. Gap report

**新建文件**：

| 文件 | 内容 |
|------|------|
| [crates/muskitty-css/tests/css_syntax_gap_report.md](file:///d:/Muskitty/crates/muskitty-css/tests/css_syntax_gap_report.md) | 初始 gap report，记录实现进度与已知失败 |

## 假设与决策

### 假设

1. **规范版本**：CSS Syntax Module Level 3（<https://drafts.csswg.org/css-syntax-3/>），即 CSSWG 最新 CR
2. **测试基线**：先内联单元测试，WPT fixtures 拉取推迟
3. **不实现 CSSOM 增量解析**：本阶段 tokenizer 一次性消费完，不支持暂停/恢复（与 html-parser reentrancy 不同，CSS 通常不需要）
4. **不实现 parse error 上报**：CSS Syntax §4.2 定义了 parse error（如 bad string、bad url），本阶段只标记 `BadString` / `BadUrl` token，不收集 ParseError 结构（与 html-parser 早期阶段一致，error 模块推迟）

### 决策

1. **依赖关系**：`muskitty-css` 不依赖 `muskitty-dom`（CSS tokenizer 只产出 Token 流，不接触 DOM）。子阶段 2（选择器）才会引入 dom 依赖
2. **crate 名称**：`muskitty-css`（与 workspace Cargo.toml 预留注释一致，非 `muskitty-css-parser`）
3. **lib.rs 入口**：`parse_stylesheet(input: &str) -> Vec<Token>` —— 本阶段只产 token 流，不产 stylesheet AST（AST 在子阶段 4 CSSOM）
4. **commit 节奏**：按 C-1 ~ C-7 分批，每批一个 commit，message 格式 `[css-tokenizer] ...`（对齐 CLAUDE.md commit 约定）
5. **edition 2021**：与 html-parser / dom 一致

## 验证步骤

每批 commit 后：

1. `cargo check -p muskitty-css` 零 warning
2. `cargo test -p muskitty-css` 全绿
3. `cargo check --workspace` 仍通过（不破坏其他 crate）

本阶段全部完成（C-7 + WPT fixtures）后：

4. `cargo test -p muskitty-css -- --nocapture` 打印通过率
5. 更新 [PROGRESS.md](file:///d:/Muskitty/PROGRESS.md) "总览"表：muskitty-css 状态由 ⬜ Phase 2 改为 🔄 进行中（CSS Syntax tokenizer ✅）
6. 更新 [css_syntax_gap_report.md](file:///d:/Muskitty/crates/muskitty-css/tests/css_syntax_gap_report.md) 记录最终通过率

## 工作量预估

按 html-parser tokenizer 阶段类比（80 状态 ~3900 行 impls.rs）：

- CSS Syntax §5 约 18 个 state（含子状态），状态数约为 HTML tokenizer 的 1/4
- 算法原语（consume_name / consume_numeric / consume_escaped / consume_bad_url）约 4 个，每个 30–80 行
- 预计 impls.rs 约 800–1200 行，types.rs 约 150 行，trait_def.rs 约 30 行
- 测试代码约 600–800 行

## 未涵盖（明确排除）

- 选择器解析（子阶段 2）
- 值解析（子阶段 3）
- CSSOM / Stylesheet AST（子阶段 4）
- Cascade + Computed values（子阶段 5）
- muskitty-dom 依赖（本阶段不需要）
- WPT fixtures 自动化 harness（推迟到状态机完成）
- HTML parser 的 11 个 `<?...>` 处理指令边界失败修复（Phase 2 期间顺带，但不属于本计划）

## 下一步

本计划批准后立即开始 C-1（Data state + 基础 token）。
