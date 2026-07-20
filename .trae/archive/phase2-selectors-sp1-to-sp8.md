# Phase 2 子阶段 2 — Selectors Level 4 实现计划 (SP-1 → SP-8)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 基于 [Selectors Level 4](https://drafts.csswg.org/selectors-4/) 规范实现一个完整的选择器解析器与匹配引擎，覆盖语法解析、特异性计算、以及基于 `muskitty-dom` 的元素匹配。

**Architecture:** 选择器实现放在新建的 `crates/muskitty-selectors` crate 中（独立无内部依赖；通过 trait 抽象与 `muskitty-dom` 解耦，匹配引擎针对任意实现 trait 的元素树工作）。解析层复用 `muskitty-css` 的 `TokenStream` 与 tokenizer。匹配层使用 `muskitty-dom` 的 `Node` / `ElementData` / `Attribute` 类型作为参考实现。

**Tech Stack:** Rust 2021, MSRV 1.82, 零外部依赖（与 muskitty-css/dom 一致），仅依赖 muskitty-css（用于 tokenizer）。

---

## 假设与决策

1. **规范源**：`D:\CSSWG\selectors-4\Overview.md`（Markdown 版本，4784 行）。一切以标准为准，每个 commit message 引用 §章节号与 Markdown L 行号。
2. **新建 crate**：`crates/muskitty-selectors/`。理由：与 `muskitty-css` 解耦——`muskitty-css` 是纯语法层（无 DOM 依赖），`muskitty-selectors` 同时依赖 `muskitty-css`（复用 tokenizer）和 `muskitty-dom`（提供参考元素树用于匹配）。
3. **匹配抽象**：定义 `Element` trait（提供 `local_name`/`namespace_uri`/`id`/`classes`/`attributes`/`parent_element`/`previous_sibling`/`children` 等只读视图）。`muskitty-dom` 的 `Rc<RefCell<Node>>` 实现该 trait；其他用户可为自定义元素树实现该 trait。这避免匹配引擎对 DOM 类型的硬依赖。
4. **Parser 复用**：直接复用 `muskitty-css::tokenize` 与 `muskitty-css::parser::TokenStream`。不重写 tokenizer。
5. **Pseudo-class 范围**：结构性伪类（§13）完整实现匹配；UI / 动态 / 资源状态 / 元素显示状态 / 输入伪类（§9-§12）**仅解析**，匹配返回 `false` stub。Linguistic（§7）/ Location（§8）伪类解析 + `:lang()`/`:dir()` 提供参数化 stub。
6. **Pseudo-elements**：解析所有合法伪元素名（§14 + legacy `:before`/`:after`/`:first-line`/`:first-letter`）；匹配仅 stub（伪元素不在 DOM 树中）。
7. **`:has()` relational**：实现完整解析；匹配实现支持后代/子代关系子选择器，兄弟关系子选择器延后（spec 仍处于较不稳定状态）。
8. **`-webkit-` quirks**（Appendix B）：暂不实现 web-compat quirks，留到浏览器集成阶段。
9. **WPT 集成**：SP-8 加入 WPT 选择器测试子集（`css/selectors/*` 目录）作为回归测试，类似 muskitty-html5-parser 的 html5lib 集成。
10. **质量门禁（强制）**：每个 SP commit 前依次执行以下四步，任一失败即禁止提交：

    ```powershell
    cargo fmt -p muskitty-selectors -- --check
    cargo test -p muskitty-selectors
    cargo check -p muskitty-selectors
    cargo clippy -p muskitty-selectors --all-targets -- -D warnings
    ```

    - **fmt**：必须 `--check` 模式零 diff；若 fmt 报告需要格式化，应先手动跑 `cargo fmt -p muskitty-selectors` 修正后再跑 `--check` 验证。
    - **test**：所有测试必须通过（无 `#[ignore]` 跳过；新增测试不能 `panic!("todo")`）。
    - **check**：必须零 warning（与 clippy 互补，主要捕获 unused imports / dead code）。
    - **clippy**：`--all-targets`（含 tests/、benches/、examples/）+ `-D warnings`（warning 升级为 error）。常见需要规避的 lint：`result_unit_err`、`incompatible_msrv`、`doc_lazy_continuation`、`doc_overindented_list_items`。
    - 如该 SP 涉及 workspace 根 `Cargo.toml` 或其他 crate（仅 SP-1 / SP-8 可能），需同时跑 `cargo fmt --check`（全 workspace）+ `cargo check --workspace` 确保未破坏其他 crate。
11. **Commit 风格**：`[selectors] SP-N: <what + why>`，例：`[selectors] SP-1: §3 selector data model + parser framework`。Commit message body 列出本批新增/修改的文件与覆盖的规范章节 + 行号。
12. **Plan 完成后自动切换 plan 模式**：SP-8 完成后自动进入下一轮 plan（CSS Values Module 值解析，依据 `D:\CSSWG\css-values-3\Overview.md` 或 `css-values-4`）。

---

## 文件结构

```
d:\Muskitty\
├── Cargo.toml                          (workspace，新增 muskitty-selectors 成员)
├── crates/
│   ├── muskitty-selectors/             (新 crate)
│   │   ├── Cargo.toml                  (依赖 muskitty-css；dev-dep muskitty-dom)
│   │   ├── README.md                   (SP-8 起草)
│   │   ├── src/
│   │   │   ├── lib.rs                  (顶层 API: parse_selector_list / matches / query_selector / query_selector_all)
│   │   │   ├── types.rs                (§3 数据模型：Selector/SimpleSelector/CompoundSelector/ComplexSelector/Combinator/SelectorList)
│   │   │   ├── parser/
│   │   │   │   ├── mod.rs              (parse_a_selector / parse_a_relative_selector entry points)
│   │   │   │   ├── simple.rs           (type/universal/class/id/attribute/pseudo-class/pseudo-element parsing)
│   │   │   │   ├── compound.rs        (compound-selector-unit parsing)
│   │   │   │   ├── complex.rs         (complex-selector parsing + combinators)
│   │   │   │   ├── list.rs            (selector-list / forgiving-selector-list parsing)
│   │   │   │   └── an_plus_b.rs       (An+B notation parsing for :nth-*())
│   │   │   ├── specificity.rs          (§17 specificity calculation)
│   │   │   ├── matching/
│   │   │   │   ├── mod.rs              (Element trait + Matcher trait)
│   │   │   │   ├── simple_matcher.rs   (匹配 type/universal/class/id/attribute)
│   │   │   │   ├── pseudo_matcher.rs   (匹配 :root/:empty/:first-child/.../:nth-child()/...)
│   │   │   │   ├── combinator_matcher.rs (后代/子代/兄弟组合器匹配)
│   │   │   │   └── dom_impl.rs         (muskitty-dom Element trait impl)
│   │   │   └── error.rs                (SelectorParseError)
│   │   └── tests/
│   │       ├── parser_types.rs         (SP-1)
│   │       ├── parser_simple.rs        (SP-2)
│   │       ├── parser_attribute.rs     (SP-3)
│   │       ├── parser_pseudo_tree.rs  (SP-4)
│   │       ├── parser_logical.rs       (SP-5)
│   │       ├── parser_complex.rs       (SP-6)
│   │       ├── specificity.rs          (SP-7)
│   │       ├── matching_basic.rs       (SP-8)
│   │       ├── matching_pseudo.rs      (SP-8)
│   │       ├── matching_dom.rs        (SP-8)
│   │       └── wpt_selectors/          (SP-8，WPT 子集)
```

---

## SP-1 — §3 数据模型 + Parser 框架

**目标**：建立 `crates/muskitty-selectors` crate 骨架，定义 §3 "Selector Syntax and Structure" 的全部数据类型，搭好解析器入口框架（暂返回 `Err(NotImplemented)`）。

**规范依据**：§3 L716-1357（含 Structure and Terminology / Data Model / Scoped Selectors / Relative Selectors / Pseudo-classes / Characters and case sensitivity / Declaring Namespace Prefixes / Invalid Selectors and Error Handling / Legacy Aliases）。

**文件**：
- `crates/muskitty-selectors/Cargo.toml`（新）
- `crates/muskitty-selectors/src/lib.rs`（新）
- `crates/muskitty-selectors/src/types.rs`（新）
- `crates/muskitty-selectors/src/parser/mod.rs`（新，骨架）
- `crates/muskitty-selectors/src/error.rs`（新）
- `Cargo.toml`（更新 workspace members）
- `crates/muskitty-selectors/tests/parser_types.rs`（新，6 个测试）

**types.rs 关键类型**：

```rust
//! Selectors Level 4 §3 data model.

use muskitty_css::tokenizer::Token;

/// §3 L858-873: A selector represents a pattern of element(s) in a tree.
#[derive(Debug, Clone)]
pub struct SelectorList(pub Vec<ComplexSelector>);

/// §3 L809-826: A complex selector is a sequence of compound selectors
/// separated by combinators.
#[derive(Debug, Clone)]
pub struct ComplexSelector {
    /// Rightmost-first ordering: `units[0]` is the rightmost compound
    /// selector (the subject), `units[1]` is to its left, etc.
    pub units: Vec<ComplexSelectorUnit>,
}

/// A compound selector + the combinator linking it to the unit on its
/// left (i.e. to `units[idx+1]`). The rightmost unit has combinator
/// `None`.
#[derive(Debug, Clone)]
pub struct ComplexSelectorUnit {
    pub compound: CompoundSelector,
    pub combinator: Option<Combinator>,
}

/// §3 L746-760: A compound selector is a sequence of simple selectors
/// with no combinator between them.
#[derive(Debug, Clone, Default)]
pub struct CompoundSelector {
    /// §3 L750-752: type selector or universal selector must come first.
    pub type_selector: Option<TypeSelector>,
    /// Subclass selectors (id, class, attribute, pseudo-class).
    pub subclasses: Vec<SubclassSelector>,
    /// §3 L762-787: pseudo-compound selectors (pseudo-element + trailing
    /// pseudo-classes). Empty for non-pseudo-element selectors.
    pub pseudo_compounds: Vec<PseudoCompoundSelector>,
}

/// §3 L798-805: Combinator between two compound selectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    /// §15 L4369: descendant (whitespace).
    Descendant,
    /// §15 L4376: child (`>`).
    Child,
    /// §15 L4383: next-sibling (`+`).
    NextSibling,
    /// §15 L4390: subsequent-sibling (`~`).
    SubsequentSibling,
}

/// §5 L1808-1824: Type (tag name) selector.
#[derive(Debug, Clone)]
pub struct TypeSelector {
    /// §5 L1867-1872: optional namespace prefix.
    pub ns_prefix: Option<NsPrefix>,
    /// Tag name (lowercase for HTML; case-sensitive for XML).
    pub name: TypeSelectorName,
}

#[derive(Debug, Clone)]
pub enum TypeSelectorName {
    /// Concrete tag name (e.g. "div", "svg", "*").
    Name(String),
    /// §5 L1825-1866: Universal selector (`*`).
    Universal,
}

/// §5 L1867-1872: namespace prefix (`ns|tag` or `*|tag`).
#[derive(Debug, Clone)]
pub struct NsPrefix {
    pub prefix: NsPrefixKind,
}

#[derive(Debug, Clone)]
pub enum NsPrefixKind {
    /// `ns|tag` — named namespace.
    Named(String),
    /// `*|tag` — any namespace.
    Any,
    /// `|tag` — no namespace (empty prefix).
    None,
}

/// §3 L4674-4685: subclass-selector = id | class | attribute | pseudo-class.
#[derive(Debug, Clone)]
pub enum SubclassSelector {
    /// §6.6 L2463-2533: `#id`.
    Id(IdSelector),
    /// §6.5 L2376-2462: `.class`.
    Class(ClassSelector),
    /// §6 L1996-2533: `[attr=value]`.
    Attribute(AttributeSelector),
    /// §13/§7-§12: `:pseudo-class` or `:pseudo-class(args)`.
    PseudoClass(PseudoClass),
}

/// §6.6 L2463-2533: ID selector.
#[derive(Debug, Clone)]
pub struct IdSelector {
    pub id: String,
}

/// §6.5 L2376-2462: Class selector.
#[derive(Debug, Clone)]
pub struct ClassSelector {
    pub class: String,
}

/// §6 L1996-2533: Attribute selector (full representation, parsed once).
#[derive(Debug, Clone)]
pub struct AttributeSelector {
    pub name: WqName,
    pub matcher: Option<AttrMatcher>,
    pub value: Option<AttrValue>,
    pub modifier: Option<AttrModifier>,
}

/// §6.1 L2023-2135: `[attr]` / `[attr=value]` / `[attr~=value]` / ...
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrMatcher {
    /// `[attr=value]` (L2037-2054).
    Exact,
    /// `[attr~=value]` (L2137-2162).
    Includes,
    /// `[attr|=value]` (L2055-2080).
    DashMatch,
    /// `[attr^=value]` (L2137-2162).
    Prefix,
    /// [`attr$=value`] (L2137-2162).
    Suffix,
    /// `[attr*=value]` (L2137-2162).
    Substring,
}

/// §6 L2193-2264: case-sensitivity modifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrModifier {
    /// `i` — case-insensitive matching.
    CaseInsensitive,
    /// `s` — case-sensitive matching (default).
    CaseSensitive,
}

/// §5 L4679-4685: wq-name = ns-prefix? ident-token.
#[derive(Debug, Clone)]
pub struct WqName {
    pub ns_prefix: Option<NsPrefix>,
    pub local_name: String,
}

/// §6 L1996-2533: attribute value (string-token or ident-token).
#[derive(Debug, Clone)]
pub enum AttrValue {
    String(String),
    Ident(String),
}

/// §13/§4 pseudo-class.
#[derive(Debug, Clone)]
pub struct PseudoClass {
    pub name: String,
    pub argument: Option<PseudoClassArgument>,
}

#[derive(Debug, Clone)]
pub enum PseudoClassArgument {
    /// For :nth-child(An+B), :nth-last-child(An+B), etc.
    AnPlusB(AnPlusB),
    /// For :is(), :not(), :where(), :has() — a selector list.
    SelectorList(SelectorList),
    /// For :lang(*), :dir(*), :current(*), etc. — preserved component values.
    Raw(Vec<Token>),
}

/// §13.5 An+B notation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnPlusB {
    pub a: i64,
    pub b: i64,
}

/// §3 L762-787: pseudo-compound selector (pseudo-element + trailing
/// pseudo-classes).
#[derive(Debug, Clone)]
pub struct PseudoCompoundSelector {
    pub pseudo_element: PseudoElement,
    pub trailing_pseudo_classes: Vec<PseudoClass>,
}

#[derive(Debug, Clone)]
pub struct PseudoElement {
    pub name: String,
    pub legacy: bool, // §14 legacy single-colon form
}

/// §3 L1317-1347: Invalid selector — kept for error reporting.
#[derive(Debug, Clone, Default)]
pub struct InvalidSelector;
```

**parser/mod.rs 骨架**（仅声明入口点，全部返回 `Err(NotImplemented)`）：

```rust
//! Selectors Level 4 parser entry points.

use crate::error::SelectorParseError;
use crate::types::SelectorList;

/// §API-Hooks L4828-4849: Parse A Selector.
pub fn parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    Err(SelectorParseError::NotImplemented)
}

/// §API-Hooks L4853-4875: Parse A Relative Selector.
pub fn parse_a_relative_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    Err(SelectorParseError::NotImplemented)
}
```

**error.rs**：

```rust
//! Selector parse errors.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorParseError {
    NotImplemented,
    UnexpectedToken(String),
    InvalidSelector(String),
    UnclosedBlock,
    InvalidAnPlusB,
    UnknownPseudoClass(String),
    UnknownPseudoElement(String),
    EmptySelector,
}
```

**测试**（`tests/parser_types.rs`，6 个）：
- `selector_list_default_empty` — `SelectorList::default()` (加 Default impl) 是空 vec
- `compound_selector_default_no_type` — `CompoundSelector::default()` type=None, subclasses=[]
- `combinator_equality` — 4 个 Combinator variant 互不相等
- `attr_matcher_variants` — 6 个 AttrMatcher variant 可构造
- `pseudo_class_argument_variants` — 3 个 PseudoClassArgument variant 可构造
- `type_selector_universal_vs_named` — `TypeSelectorName::Universal` 与 `Name("div")` 不相等

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

SP-1 涉及 workspace 根 `Cargo.toml`（新增 muskitty-selectors 成员），额外执行：

```powershell
cargo fmt --check
cargo check --workspace
```

**Commit**：
```
[selectors] SP-1: §3 selector data model + parser framework skeleton

- New crate crates/muskitty-selectors/.
- §3 L716-1357 data model: SelectorList, ComplexSelector,
  ComplexSelectorUnit, CompoundSelector, Combinator, TypeSelector,
  NsPrefix, SubclassSelector, IdSelector, ClassSelector,
  AttributeSelector, AttrMatcher, AttrModifier, WqName, AttrValue,
  PseudoClass, PseudoClassArgument, AnPlusB, PseudoCompoundSelector,
  PseudoElement, InvalidSelector.
- Parser entry points parse_a_selector / parse_a_relative_selector
  skeleton (return NotImplemented).
- SelectorParseError type.
- Workspace Cargo.toml: add muskitty-selectors member.
- 6 unit tests for data-structure construction.
```

---

## SP-2 — §5 + §6.5-§6.6 Elemental / Class / ID 解析

**目标**：实现 type-selector / universal-selector / class-selector / id-selector 的解析（不含 attribute，留到 SP-3）。

**规范依据**：
- §5 L1805-1995 Elemental selectors（含 §5.3 Namespaces in Elemental Selectors L1867-1872，§5.4 :defined L1956-1995 延后到 SP-4 一起做伪类）
- §6.5 L2376-2462 Class selectors
- §6.6 L2463-2533 ID selectors
- §3 L1245-1306 Characters and case sensitivity（ident-token 大小写规则）
- §3 L1307-1316 Declaring Namespace Prefixes（解析时只识别语法，不验证命名空间声明，未声明前缀 → SP-8 报告 invalid selector）

**文件**：
- `crates/muskitty-selectors/src/parser/mod.rs`（更新入口点：调用 parse_selector_list）
- `crates/muskitty-selectors/src/parser/simple.rs`（新）
- `crates/muskitty-selectors/src/parser/compound.rs`（新，骨架）
- `crates/muskitty-selectors/src/parser/complex.rs`（新，骨架，返回单 compound 单元）
- `crates/muskitty-selectors/src/parser/list.rs`（新）
- `crates/muskitty-selectors/tests/parser_simple.rs`（新，10 个测试）

**关键函数签名**：

```rust
// parser/simple.rs
pub fn parse_type_selector(stream: &mut TokenStream) -> Result<Option<TypeSelector>, SelectorParseError>;
pub fn parse_universal_selector(stream: &mut TokenStream) -> Result<Option<TypeSelector>, SelectorParseError>;
pub fn parse_class_selector(stream: &mut TokenStream) -> Result<Option<ClassSelector>, SelectorParseError>;
pub fn parse_id_selector(stream: &mut TokenStream) -> Result<Option<IdSelector>, SelectorParseError>;
pub fn parse_ns_prefix(stream: &mut TokenStream) -> Result<Option<NsPrefix>, SelectorParseError>;

// parser/compound.rs (骨架，仅处理 type + subclass)
pub fn parse_compound_selector(stream: &mut TokenStream) -> Result<CompoundSelector, SelectorParseError>;

// parser/complex.rs (骨架，单 compound，no combinator)
pub fn parse_complex_selector(stream: &mut TokenStream) -> Result<ComplexSelector, SelectorParseError>;

// parser/list.rs
pub fn parse_selector_list(stream: &mut TokenStream) -> Result<SelectorList, SelectorParseError>;
pub fn parse_forgiving_selector_list(stream: &mut TokenStream) -> Result<SelectorList, SelectorParseError>;
```

**解析逻辑要点**：
- `parse_ns_prefix`：`ident-token '|'` 或 `'*' '|'` 或 `'|'`（empty）。返回 NsPrefix。
- `parse_type_selector`：先尝试 ns_prefix，再读 ident-token 作为 name（或 `*` 作为 Universal）。注意 `*|*` 是 universal-selector with Any ns prefix。
- `parse_class_selector`：`'.' ident-token`。
- `parse_id_selector`：`hash-token` whose value would-start-an-ident-sequence（即 `HashType::Id`）。如果 hash 是 unrestricted 类型（hex digits），不是合法 id-selector（§6.6 L2463-2533）。
- `parse_compound_selector`：type-selector? subclass-selector*（subclass 这里仅 class/id，attribute/pseudo-class 留到 SP-3/SP-4）。
- `parse_selector_list`：`complex-selector#`，comma-separated，trailing comma 错误。
- `parse_forgiving_selector_list`：每个 complex-selector 独立解析，失败的跳过。

**测试**（10 个）：
- `type_selector_simple_div` — `"div"` → TypeSelector{ns=None, name=Name("div")}
- `type_selector_universal_star` — `"*"` → Universal
- `type_selector_ns_named` — `"svg|rect"` → ns=Named("svg"), name=Name("rect")
- `type_selector_ns_any` — `"*|div"` → ns=Any, name=Name("div")
- `type_selector_ns_none` — `"|div"` → ns=None(空), name=Name("div")
- `class_selector_simple` — `".foo"` → ClassSelector{class="foo"}
- `class_selector_after_type` — `"div.foo"` → compound with both
- `id_selector_simple` — `"#main"` → IdSelector{id="main"}
- `id_selector_hex_digits_rejected` — `"#123abc"` → Err（hash 不是 Id type）
- `selector_list_two_comma_separated` — `"a, b"` → 2 complex selectors

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

**Commit**：
```
[selectors] SP-2: §5/§6.5/§6.6 type/class/id selector parsing
```

---

## SP-3 — §6 Attribute selectors 解析

**目标**：完整实现 §6 attribute selector 解析（presence + 6 种 matcher + i/s modifier + namespace 前缀）。

**规范依据**：
- §6.1 L2023-2135 Attribute presence and value selectors
- §6.2 L2137-2162 Substring matching attribute selectors
- §6.3 L2193-2264 Case-sensitivity
- §6.4 L2266-2313 Attribute selectors and namespaces
- §6 L2315-2375 Default attribute values in DTDs（仅解析 `attr?`，DTD 解析延后）

**文件**：
- `crates/muskitty-selectors/src/parser/simple.rs`（加 `parse_attribute_selector`）
- `crates/muskitty-selectors/src/parser/compound.rs`（subclass 分支接入 attribute）
- `crates/muskitty-selectors/tests/parser_attribute.rs`（新，10 个测试）

**关键函数**：

```rust
pub fn parse_attribute_selector(stream: &mut TokenStream) -> Result<AttributeSelector, SelectorParseError>;
```

**解析逻辑**（§6.1 + §6.2 + §6.3）：
1. 必须 `[-token`，否则返回 Err（注：调用方应先 peek）。
2. 丢弃 `[`。
3. Discard whitespace。
4. 解析 `wq-name`（ns-prefix? + ident-token）。注：`*|attr`、`ns|attr`、`|attr`、`attr` 都合法。
5. Discard whitespace。
6. 如果下一个是 `]-token`：丢弃 `]`，返回 presence selector（matcher=None）。
7. 否则解析 `attr-matcher`：`[~|^|$|*]? =`。
8. Discard whitespace。
9. 解析 value：string-token 或 ident-token。
10. Discard whitespace。
11. 可选 `attr-modifier`：ident-token "i" 或 "s"（大小写不敏感比较）。
12. Discard whitespace。
13. 必须 `]-token`，否则 Err(UnclosedBlock)。

**测试**（10 个）：
- `attr_presence` — `"[disabled]"` → matcher=None
- `attr_exact_string_value` — `"[lang=\"en\"]"` → Exact + String("en")
- `attr_exact_ident_value` — `"[lang=en]"` → Exact + Ident("en")
- `attr_includes` — `"[class~=\"foo\"]"` → Includes
- `attr_dash_match` — `"[lang|=en]"` → DashMatch
- `attr_prefix` — `"[href^=\"https\"]"` → Prefix
- `attr_suffix` — `"[href$=\".pdf\"]"` → Suffix
- `attr_substring` — `"[class*=\"btn\"]"` → Substring
- `attr_modifier_i` — `"[attr=value i]"` → CaseInsensitive
- `attr_modifier_s` — `"[attr=value s]"` → CaseSensitive
- `attr_with_ns_prefix` — `"[svg|href]"` → WqName{ns=Named("svg"), local="href"}

（共 11 个，超出 plan 数 10 是因为 ns-prefix 测试独立验证。）

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

**Commit**：
```
[selectors] SP-3: §6 attribute selector parsing (presence + 6 matchers + i/s modifier + ns prefix)
```

---

## SP-4 — §13 Tree-Structural pseudo-classes + An+B + :defined + :scope

**目标**：实现 §13 tree-structural pseudo-classes 的完整解析（含 An+B 数学），以及 §5.4 :defined、§8 :scope（先解析，匹配留 SP-8）。

**规范依据**：
- §13 L3792-4359：:root / :empty / :first-child / :last-child / :only-child / :first-of-type / :last-of-type / :only-of-type / :nth-child() / :nth-last-child() / :nth-of-type() / :nth-last-of-type() / :empty
- §13.5 An+B 数学（nth-* 共享）
- §5.4 L1956-1995 :defined
- §8 L2817-3007 :scope（解析仅；匹配需 scoping root 上下文，SP-8）

**文件**：
- `crates/muskitty-selectors/src/parser/an_plus_b.rs`（新）
- `crates/muskitty-selectors/src/parser/simple.rs`（加 `parse_pseudo_class`、`parse_pseudo_element`）
- `crates/muskitty-selectors/src/parser/compound.rs`（subclass 分支接入 pseudo-class；末尾 pseudo_compounds 分支接入 pseudo-element）
- `crates/muskitty-selectors/tests/parser_pseudo_tree.rs`（新，12 个测试）

**An+B 解析（§13.5）**：

合法形式（per spec）：
- `odd` → (2, 1)
- `even` → (2, 0)
- `<integer>` → (0, n)
- `<n>` → (1, 0)
- `<n> <signed-integer>` → (1, signed_int)
- `<n> ['+'|'-'] <signless-integer>` → (1, signed_int)
- `<n> ['+'|'-']? <signless-integer>` 等价
- `[<signed-integer>] <n> [<signed-integer>]`（反向形式）
- `[<signless-integer>] <n> [<signless-integer>]`

注意：CSS tokenizer 把 `n`、`-n`、`+n` 都视为 ident-token（或其变体）。需要特殊处理。

```rust
pub fn parse_an_plus_b(stream: &mut TokenStream) -> Result<AnPlusB, SelectorParseError>;
```

**pseudo-class / pseudo-element 解析**：

```rust
pub fn parse_pseudo_class(stream: &mut TokenStream) -> Result<PseudoClass, SelectorParseError>;
pub fn parse_pseudo_element(stream: &mut TokenStream) -> Result<PseudoCompoundSelector, SelectorParseError>;
```

`:ident` → 简单伪类。
`:function(args)` → 带参数伪类。An+B 参数调 parse_an_plus_b；selector-list 参数调 parse_forgiving_selector_list（SP-5 接入 is/not/where/has）；其他参数保留 component values 为 Raw。
`::ident` 或 `::ident(args)` → 伪元素。
legacy: `:before`/`:after`/`:first-line`/`:first-letter`（单冒号）→ legacy=true 的伪元素。

**已知伪类白名单**（解析时验证名字，未知 → Err(UnknownPseudoClass)）：

Tree-structural: root, empty, first-child, last-child, only-child, first-of-type, last-of-type, only-of-type, nth-child, nth-last-child, nth-of-type, nth-last-of-type

Defined: defined

Scope: scope

Linguistic (SP-5 之后接入): lang, dir

Logical (SP-5): is, not, where, has

User Action / Resource / Display / Input (解析 + 匹配 stub): hover, active, focus, focus-visible, focus-within, playing, paused, seeking, buffering, stalled, muted, volume-locked, enabled, disabled, read-only, read-write, placeholder-shown, default, checked, indeterminate, valid, invalid, in-range, out-of-range, required, optional, blank, blank

Location (解析 + 匹配 stub): any-link, link, visited, local-link, target, target-within, current, past, future, scope, host, host-context

**测试**（12 个）：
- `pseudo_class_root` — `":root"` → PseudoClass{name="root", arg=None}
- `pseudo_class_empty` — `":empty"` → PseudoClass{name="empty", arg=None}
- `pseudo_class_nth_child_simple` — `":nth-child(2)"` → AnPlusB{a=0, b=2}
- `pseudo_class_nth_child_odd` — `":nth-child(odd)"` → AnPlusB{a=2, b=1}
- `pseudo_class_nth_child_even` — `":nth-child(even)"` → AnPlusB{a=2, b=0}
- `pseudo_class_nth_child_n` — `":nth-child(n)"` → AnPlusB{a=1, b=0}
- `pseudo_class_nth_child_2n_plus_1` — `":nth-child(2n+1)"` → AnPlusB{a=2, b=1}
- `pseudo_class_nth_child_negative_n` — `":nth-child(-n+3)"` → AnPlusB{a=-1, b=3}
- `pseudo_class_unknown_rejected` — `":foobar"` → Err(UnknownPseudoClass)
- `pseudo_element_simple` — `"::before"` → PseudoElement{name="before", legacy=false}
- `pseudo_element_legacy_single_colon` — `":before"` → PseudoElement{name="before", legacy=true}
- `pseudo_element_legacy_rejected_as_pseudo_class` — `":after"` 走 pseudo-class 路径 → Err（必须走 legacy pseudo-element 路径）

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

**Commit**：
```
[selectors] SP-4: §13 tree-structural pseudo-classes + An+B + :defined + :scope parsing
```

---

## SP-5 — §4 Logical Combinations (:is / :not / :where / :has) 解析

**目标**：实现 §4 的 4 个逻辑组合伪类解析，含 forgiving-selector-list 行为。

**规范依据**：
- §4 L1358-1804 Logical Combinations
- §4.1 L1383-1442 Selector Lists（基础 + forgiving 行为）
- §4.2 L1443-1525 :is() Matches-Any Pseudo-class
- §4.3 L1526-1592 :not() Negation Pseudo-class
- §4.4 L1593-1649 :where() Specificity-adjustment Pseudo-class
- §4.5 L1650-1804 :has() Relational Pseudo-class
- §3 L4765-4813 forgiving-selector-list 语义

**文件**：
- `crates/muskitty-selectors/src/parser/simple.rs`（pseudo-class 带 selector-list 参数的分支）
- `crates/muskitty-selectors/src/parser/list.rs`（forgiving-selector-list 完成）
- `crates/muskitty-selectors/src/parser/relative.rs`（新，:has() 用 relative-selector-list）
- `crates/muskitty-selectors/tests/parser_logical.rs`（新，10 个测试）

**关键设计**：

`:is()`/`:where()` 参数为 `forgiving-selector-list`（§4.2/§4.4 L1497-1499 + L1617）：失败的 selector 被丢弃，剩余的组成 selector-list。

`:not()` 参数为 `selector-list`（**非 forgiving**），但 Level 4 允许 complex selector 作为参数（Level 3 仅 simple）。

`:has()` 参数为 `relative-selector-list`（§4.5 L1700）。relative-selector 以可选 combinator 开头，缺省为 descendant。

```rust
// parser/relative.rs
pub fn parse_relative_selector(stream: &mut TokenStream) -> Result<ComplexSelector, SelectorParseError>;
pub fn parse_relative_selector_list(stream: &mut TokenStream) -> Result<SelectorList, SelectorParseError>;
```

**pseudo-class 参数分发**（更新 parse_pseudo_class）：

```rust
match name.as_str() {
    "is" | "where" => {
        let list = parse_forgiving_selector_list(stream)?;
        Ok(PseudoClass { name, argument: Some(PseudoClassArgument::SelectorList(list)) })
    }
    "not" => {
        let list = parse_selector_list(stream)?; // non-forgiving
        Ok(PseudoClass { name, argument: Some(PseudoClassArgument::SelectorList(list)) })
    }
    "has" => {
        let list = parse_relative_selector_list(stream)?;
        // :has() 的 list 包装为 SelectorList（subject 是 :has() 的元素本身，
        // 内部 complex-selector 以 implicit descendant combinator 开头）
        Ok(PseudoClass { name, argument: Some(PseudoClassArgument::SelectorList(list)) })
    }
    // ... 其他伪类 ...
}
```

**测试**（10 个）：
- `is_simple` — `":is(.a, .b)"` → PseudoClass{name="is", arg=SelectorList([Class("a"), Class("b")])}
- `is_forgiving_drops_invalid` — `":is(.a, invalid syntax, .b)"` → list 中只保留 .a 和 .b
- `where_zero_specificity_marker` — `":where(.a)"` → 与 :is 结构相同（specificity 在 SP-7 才区分）
- `not_simple` — `":not(.a)"` → arg=SelectorList([Class("a")])
- `not_complex_selector_arg` — `":not(.a > .b)"` → arg=SelectorList([ComplexSelector with child combinator])
- `not_non_forgiving_invalid_fails` — `":not(.a, invalid syntax)"` → Err（non-forgiving）
- `has_descendant_default` — `":has(.a)"` → arg=SelectorList，内部以 Descendant combinator
- `has_child_explicit` — `":has(> .a)"` → arg=SelectorList with Child combinator
- `has_next_sibling` — `":has(+ .a)"` → NextSibling combinator
- `has_subsequent_sibling` — `":has(~ .a)"` → SubsequentSibling combinator

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

**Commit**：
```
[selectors] SP-5: §4 logical combinations (:is/:not/:where/:has) parsing + forgiving-selector-list
```

---

## SP-6 — §15 Combinators + 完整 complex/compound selector 解析

**目标**：把 SP-2..SP-5 的零散解析逻辑组合为完整的 `complex-selector` 解析器，处理 4 种 combinator 与 trailing combinator 错误。

**规范依据**：
- §15 L4360-4532 Combinators
- §3 L4640-4815 Grammar（complex-selector = complex-selector-unit [ combinator? complex-selector-unit ]*）
- §3 L4704-4741 解析规则（whitespace 禁止/必需的边界）
- §3 L1317-1347 Invalid Selectors and Error Handling

**文件**：
- `crates/muskitty-selectors/src/parser/compound.rs`（完成 compound-selector-unit 解析，含 pseudo-compound）
- `crates/muskitty-selectors/src/parser/complex.rs`（完成 complex-selector 解析，含 combinator + implicit descendant）
- `crates/muskitty-selectors/src/parser/list.rs`（接入完整流程，parse_a_selector 调用 parse_selector_list）
- `crates/muskitty-selectors/tests/parser_complex.rs`（新，12 个测试）

**关键解析流程**：

```rust
// parser/complex.rs
pub fn parse_complex_selector(stream: &mut TokenStream) -> Result<ComplexSelector, SelectorParseError> {
    let mut units: Vec<ComplexSelectorUnit> = Vec::new();
    let first_compound = parse_compound_selector_unit(stream)?;
    units.push(ComplexSelectorUnit { compound: first_compound, combinator: None });
    
    loop {
        // 检查 combinator 或 implicit descendant
        let combinator = parse_optional_combinator(stream)?;
        if combinator.is_none() && is_terminator(stream.next_token()) {
            break;
        }
        let next_compound = parse_compound_selector_unit(stream)?;
        // 前一个 unit 的 combinator 字段填上 combinator（缺省 Descendant）
        let effective_combinator = combinator.unwrap_or(Combinator::Descendant);
        units.last_mut().unwrap().combinator = Some(effective_combinator);
        units.push(ComplexSelectorUnit { compound: next_compound, combinator: None });
    }
    
    Ok(ComplexSelector { units })
}

fn parse_optional_combinator(stream: &mut TokenStream) -> Result<Option<Combinator>, SelectorParseError> {
    // 显式 combinator: `>` / `+` / `~`（可能前后有 whitespace）
    // 隐式 combinator: 仅 whitespace（→ Descendant）
    // 无 combinator: 无 whitespace 且非 combinator 字符 → 终止
}
```

**combinator 解析细节**（§15 L4369-4398）：

| 输入 | combinator |
|------|-------------|
| ` ` (whitespace) | Descendant |
| `>` (可能前后 ws) | Child |
| `+` (可能前后 ws) | NextSibling |
| `~` (可能前后 ws) | SubsequentSibling |

trailing combinator（如 `"a >"`）→ Err(InvalidSelector)。

**完整 selector-list 解析**（接入 parse_a_selector）：

```rust
// parser/mod.rs
pub fn parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    let tokens = muskitty_css::tokenize(source);
    let mut stream = TokenStream::new(tokens);
    let list = parse_selector_list(&mut stream)?;
    // 检查未消费的 tokens（trailing garbage）
    stream.discard_whitespace();
    if !stream.is_empty() {
        return Err(SelectorParseError::InvalidSelector("trailing garbage".into()));
    }
    Ok(list)
}
```

**测试**（12 个）：
- `single_compound` — `"div.foo"` → 1 unit
- `descendant_whitespace` — `"a b"` → 2 units with Descendant
- `child_explicit` — `"a > b"` → Child
- `next_sibling` — `"a + b"` → NextSibling
- `subsequent_sibling` — `"a ~ b"` → SubsequentSibling
- `three_part_descendant` — `"a b c"` → 3 units, 2 Descendant combinators
- `mixed_combinators` — `"a > b + c"` → 3 units: Child, NextSibling
- `combinator_with_pseudo_class` — `"a > b:hover"` → pseudo-class 在 rightmost compound
- `trailing_combinator_fails` — `"a >"` → Err
- `selector_list_three_items` — `"a, b, c"` → 3 complex selectors
- `trailing_comma_fails` — `"a,"` → Err
- `empty_string_fails` — `""` → Err(EmptySelector)

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

**Commit**：
```
[selectors] SP-6: §15 combinators + complete complex/compound selector parsing
```

---

## SP-7 — §17 Specificity 计算

**目标**：实现 §17 specificity 计算（A/B/C triplet）。

**规范依据**：
- §17 L4533-4639 Calculating a selector's specificity
- §4.4 :where() 的 zero-specificity 行为（L1593-1649）
- §4.2 :is() 取参数中最高的 specificity（L1443-1525）
- §4.3 :not() 取参数中最高的 specificity（L1526-1592）
- §4.5 :has() 取参数中最高的 specificity（L1650-1804）
- §6 pseudo-class `:nth-child(an+b of S)` / `:nth-last-child(an+b of S)` 的 specificity（spec L4079-4087）

**文件**：
- `crates/muskitty-selectors/src/specificity.rs`（新）
- `crates/muskitty-selectors/src/lib.rs`（导出 Specificity）
- `crates/muskitty-selectors/tests/specificity.rs`（新，12 个测试）

**Specificity 数据类型**：

```rust
/// §17 L4536-4552: Specificity is a triplet (A, B, C) compared
/// lexicographically.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity {
    /// ID selector count.
    pub a: u32,
    /// Class selector, attribute selector, pseudo-class count.
    pub b: u32,
    /// Type selector, pseudo-element count.
    pub c: u32,
}

impl Specificity {
    pub fn add(&mut self, other: Specificity) {
        self.a += other.a;
        self.b += other.b;
        self.c += other.c;
    }
}
```

**计算规则**（§17 L4555-4624）：

| 选择器组件 | 计入 |
|-----------|------|
| `#id` | A += 1 |
| `.class` / `[attr]` / `[attr=...]` / `:pseudo-class` | B += 1 |
| `div` / `*`（universal 不计）/ `::pseudo-element` | C += 1 |
| `:is(args)` / `:not(args)` / `:has(args)` | 取 args 中最高的 specificity（**整个 list 内最高的 complex-selector**） |
| `:where(args)` | 不计入（zero specificity） |
| `:nth-child(an+b of S)` / `:nth-last-child(an+b of S)` | An+B 部分 → B+=1；of S 部分 → 取 S 中最高的 specificity |

注：universal selector (`*`) **不计 specificity**（§17 L4559）。但 ns prefix `ns|*` 也不计。

**关键函数**：

```rust
pub fn compute_specificity(selector: &SelectorList) -> Specificity;
fn complex_selector_specificity(cs: &ComplexSelector) -> Specificity;
fn compound_selector_specificity(comp: &CompoundSelector) -> Specificity;
fn subclass_specificity(s: &SubclassSelector) -> Specificity;
fn pseudo_class_specificity(pc: &PseudoClass) -> Specificity;
fn pseudo_compound_specificity(pc: &PseudoCompoundSelector) -> Specificity;
```

**测试**（12 个）：
- `id_only` — `"#main"` → (1, 0, 0)
- `class_only` — `".foo"` → (0, 1, 0)
- `attr_only` — `"[disabled]"` → (0, 1, 0)
- `pseudo_class_only` — `":hover"` → (0, 1, 0)
- `type_only` — `"div"` → (0, 0, 1)
- `pseudo_element_only` — `"::before"` → (0, 0, 1)
- `universal_zero` — `"*"` → (0, 0, 0)
- `compound_sum` — `"div.foo#main"` → (1, 1, 1)
- `descendant_sum` — `"#a .b div"` → (1, 1, 1)
- `where_zero_specificity` — `":where(#main)"` → (0, 0, 0)
- `is_takes_max` — `":is(#a, .b)"` → (1, 0, 0)
- `nth_child_of_s` — `":nth-child(2n+1 of .foo, #bar)"` → (1, 1, 0)

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

**Commit**：
```
[selectors] SP-7: §17 specificity calculation (A/B/C triplet + :is/:not/:has max + :where zero)
```

---

## SP-8 — §18 Matching 引擎 + lib.rs 顶层 API + WPT 子集

**目标**：实现匹配引擎（基于 `muskitty-dom`），暴露 `parse_selector_list` / `matches` / `query_selector` / `query_selector_all` 顶层 API，并集成 WPT 选择器测试子集作为回归测试。

**规范依据**：
- §18 L4816-5026 API Hooks（Match a Selector Against an Element / Pseudo-element / Tree）
- §3 L858-965 Data Model（5 个 aspects：type/namespace/id/classes/attributes）
- §5/§6/§13 各 selector 的 matching rules
- §15 L4360-4532 Combinators matching rules

**文件**：
- `crates/muskitty-selectors/src/matching/mod.rs`（新，Element trait + Matcher trait + 公共 API）
- `crates/muskitty-selectors/src/matching/simple_matcher.rs`（新）
- `crates/muskitty-selectors/src/matching/pseudo_matcher.rs`（新）
- `crates/muskitty-selectors/src/matching/combinator_matcher.rs`（新）
- `crates/muskitty-selectors/src/matching/dom_impl.rs`（新，muskitty-dom 适配）
- `crates/muskitty-selectors/src/lib.rs`（更新顶层 API）
- `crates/muskitty-selectors/Cargo.toml`（dev-dep 加 muskitty-dom）
- `crates/muskitty-selectors/tests/matching_basic.rs`（新）
- `crates/muskitty-selectors/tests/matching_pseudo.rs`（新）
- `crates/muskitty-selectors/tests/matching_dom.rs`（新，端到端用 muskitty-dom）
- `crates/muskitty-selectors/tests/wpt_selectors/`（新，WPT 子集 fixture，先手动选 20 个简单测试）
- `crates/muskitty-selectors/README.md`（新）
- `crates/muskitty-selectors/Cargo.toml`（版本升 0.1.0）
- `PROGRESS.md`（更新）

**Element trait**（匹配抽象）：

```rust
/// §3 L858-873 + §18 L4879-4900: The 5 aspects of an element matched
/// against selectors. Implementors provide a read-only view; the
/// matching engine never mutates.
pub trait Element {
    /// §3 L870: type (tag name). Lowercase for HTML.
    fn local_name(&self) -> &str;
    /// §3 L871: namespace URI (None for no namespace).
    fn namespace_uri(&self) -> Option<&str>;
    /// §3 L872: ID attribute value (None if absent).
    fn id(&self) -> Option<&str>;
    /// §3 L873: classes (space-separated list, may be empty).
    fn classes(&self) -> ClassIter<'_>;
    /// §3 L874: attribute lookup by name (HTML namespace: ASCII-case-insensitive).
    fn get_attribute(&self, name: &str) -> Option<&str>;
    /// Parent element (None for root / detached).
    fn parent_element(&self) -> Option<Self> where Self: Sized;
    /// Previous sibling element (None if first child or no parent).
    fn previous_sibling_element(&self) -> Option<Self> where Self: Sized;
    /// Next sibling element (None if last child or no parent).
    fn next_sibling_element(&self) -> Option<Self> where Self: Sized;
    /// Iterate child elements (excluding text/comment).
    fn child_elements(&self) -> Box<dyn Iterator<Item = Self> + '_> where Self: Sized;
    /// Whether this is the document root (no parent element).
    fn is_root(&self) -> bool where Self: Sized { self.parent_element().is_none() }
    /// Whether the element has any child nodes (text, element, comment).
    fn is_empty(&self) -> bool;
    /// Index among siblings of the same type (1-based).
    fn index_among_type(&self) -> usize where Self: Sized;
    /// Total count of siblings of the same type.
    fn count_among_type(&self) -> usize where Self: Sized;
    /// Index among all sibling elements (1-based).
    fn index_among_siblings(&self) -> usize where Self: Sized;
    /// Total count of all sibling elements.
    fn count_among_siblings(&self) -> usize where Self: Sized;
}

pub struct ClassIter<'a> { /* ... */ }
```

**muskitty-dom Element impl**（`matching/dom_impl.rs`）：

```rust
use muskitty_dom::node::{Node, NodeType};
use std::cell::RefCell;
use std::rc::Rc;

impl Element for Rc<RefCell<Node>> {
    fn local_name(&self) -> &str {
        // 需要 borrow() — 但 trait 要求 &str 不能跨 borrow 边界返回
        // 解决：用 ElementHandle 拥有 Rc，返回 'static 不可行
        // 方案：用 &self 借用 Rc<RefCell<Node>>，内部 borrow 取值，返回 'self 的 &str
        //       但 RefCell 的 borrow 是运行时的，&str 生命周期挂在 borrow guard 上
        // → 用 Ref<'_, Node> wrapper 或返回 String
        unimplemented!() // 见下方实现策略
    }
    // ...
}
```

**实现策略**（解决 RefCell 借用问题）：

trait `Element` 改为返回 owned `String` 而非 `&str`，避免 `Rc<RefCell<Node>>` 的 borrow lifetime 问题：

```rust
pub trait Element {
    fn local_name(&self) -> String;
    fn namespace_uri(&self) -> Option<String>;
    fn id(&self) -> Option<String>;
    fn classes(&self) -> Vec<String>;
    fn get_attribute(&self, name: &str) -> Option<String>;
    fn parent_element(&self) -> Option<Rc<RefCell<Node>>>; // 改为具体的 Rc<RefCell<Node>> 类型
    // ... 或者用 associated type
}
```

或用 associated type：

```rust
pub trait Element {
    type Handle: Element; // self-referencing associated type
    fn local_name(&self) -> String;
    fn parent_element(&self) -> Option<Self::Handle>;
    // ...
}
```

第二种更类型安全，SP-8 实现时选择。计划文档中两种都列出，由执行者根据 clippy/test 反馈选定。

**Matching 算法**（§18 L4902-4919 right-to-left 匹配）：

```rust
pub fn matches<E: Element>(selector: &SelectorList, element: &E) -> bool {
    selector.0.iter().any(|cs| matches_complex(cs, element))
}

fn matches_complex<E: Element>(cs: &ComplexSelector, element: &E) -> bool {
    // §18 L4902-4919: right-to-left
    // units[0] 是 subject（rightmost），从它开始匹配
    let mut current = element;
    for (i, unit) in cs.units.iter().enumerate() {
        if !matches_compound(&unit.compound, &current) {
            return false;
        }
        if let Some(combinator) = &unit.combinator {
            // 找到符合 combinator 的前一个元素，递归匹配 cs.units[i+1..]
            let next_compound = &cs.units[i + 1].compound;
            match combinator {
                Combinator::Descendant => {
                    // 任何祖先元素
                    let mut ancestor = current.parent_element();
                    while let Some(parent) = ancestor {
                        if matches_complex_remaining(&cs.units[i + 1..], &parent) {
                            return true;
                        }
                        ancestor = parent.parent_element();
                    }
                    return false;
                }
                Combinator::Child => {
                    if let Some(parent) = current.parent_element() {
                        return matches_complex_remaining(&cs.units[i + 1..], &parent);
                    }
                    return false;
                }
                Combinator::NextSibling => {
                    if let Some(prev) = current.previous_sibling_element() {
                        return matches_complex_remaining(&cs.units[i + 1..], &prev);
                    }
                    return false;
                }
                Combinator::SubsequentSibling => {
                    let mut prev = current.previous_sibling_element();
                    while let Some(sibling) = prev {
                        if matches_complex_remaining(&cs.units[i + 1..], &sibling) {
                            return true;
                        }
                        prev = sibling.previous_sibling_element();
                    }
                    return false;
                }
            }
        }
    }
    true // 所有 units 都匹配
}
```

**简单 selector 匹配**（simple_matcher.rs）：

```rust
fn matches_type<E: Element>(sel: &TypeSelector, e: &E) -> bool;
fn matches_class<E: Element>(sel: &ClassSelector, e: &E) -> bool;
fn matches_id<E: Element>(sel: &IdSelector, e: &E) -> bool;
fn matches_attribute<E: Element>(sel: &AttributeSelector, e: &E) -> bool;
```

**伪类匹配**（pseudo_matcher.rs，结构性完整，其他 stub）：

```rust
fn matches_pseudo_class<E: Element>(pc: &PseudoClass, e: &E) -> bool {
    match pc.name.as_str() {
        "root" => e.is_root(),
        "empty" => e.is_empty(),
        "first-child" => e.index_among_siblings() == 1,
        "last-child" => e.index_among_siblings() == e.count_among_siblings(),
        "only-child" => e.count_among_siblings() == 1,
        "first-of-type" => e.index_among_type() == 1,
        "last-of-type" => e.index_among_type() == e.count_among_type(),
        "only-of-type" => e.count_among_type() == 1,
        "nth-child" => matches_nth_child(pc, e, /* of_S */ false, /* from_last */ false),
        "nth-last-child" => matches_nth_child(pc, e, false, true),
        "nth-of-type" => matches_nth_of_type(pc, e, false, false),
        "nth-last-of-type" => matches_nth_of_type(pc, e, false, true),
        // Stub: always false
        "hover" | "active" | "focus" | "focus-visible" | "focus-within" |
        "link" | "visited" | "any-link" | "local-link" | "target" | "target-within" |
        "playing" | "paused" | "seeking" | "buffering" | "stalled" | "muted" | "volume-locked" |
        "enabled" | "disabled" | "read-only" | "read-write" | "placeholder-shown" |
        "default" | "checked" | "indeterminate" | "valid" | "invalid" |
        "in-range" | "out-of-range" | "required" | "optional" | "blank" |
        "defined" | "scope" | "host" | "host-context" |
        "current" | "past" | "future" | "lang" | "dir" => false,
        "is" | "where" => matches_is_where(pc, e),
        "not" => !matches_is_where(pc, e),  // :not(arg) = !:is(arg)
        "has" => matches_has(pc, e),
        _ => false,
    }
}
```

**An+B 匹配**（:nth-child(An+B)）：

```rust
fn matches_nth_an_plus_b(anb: AnPlusB, index: usize) -> bool {
    // index = a*k + b 的非负整数 k 是否存在
    // a == 0: index == b
    // a != 0: (index - b) / a 是非负整数
    if anb.a == 0 {
        index as i64 == anb.b
    } else {
        let diff = index as i64 - anb.b;
        diff >= 0 && diff % anb.a == 0
    }
}
```

**lib.rs 顶层 API**：

```rust
//! MusKitty Selectors — Selectors Level 4 parser & matcher.

pub mod error;
pub mod matching;
pub mod parser;
pub mod specificity;
pub mod types;

pub use error::SelectorParseError;
pub use matching::{matches, query_selector, query_selector_all, Element};
pub use parser::{parse_a_relative_selector, parse_a_selector};
pub use specificity::Specificity;
pub use types::*;

/// Parse a selector list and return it (§API-Hooks Parse A Selector).
///
/// # Examples
///
/// ```
/// use muskitty_selectors::parse_selector_list;
///
/// let list = parse_selector_list("div.foo, .bar").unwrap();
/// assert_eq!(list.0.len(), 2);
/// ```
pub fn parse_selector_list(source: &str) -> Result<SelectorList, SelectorParseError> {
    parse_a_selector(source)
}

/// Compute the specificity of a selector list (the max across all
/// complex selectors).
pub fn specificity_of(list: &SelectorList) -> Specificity {
    specificity::compute_specificity(list)
}
```

**测试**（25 个，分 3 文件）：
- `matching_basic.rs` (8 个): type/class/id/attribute/universal 单 selector 匹配
- `matching_pseudo.rs` (10 个): :root/:empty/:first-child/:last-child/:only-child/:first-of-type/:nth-child(2)/:nth-child(odd)/:nth-child(2n+1)/:not(.foo)
- `matching_dom.rs` (7 个): 端到端，构造 muskitty-dom 树，调用 matches / query_selector / query_selector_all

**WPT 子集**（手动选 20 个简单的）：
- 从 `https://github.com/web-platform-tests/wpt/tree/master/css/selectors` 选 20 个无外部依赖的 .html 测试，把 selector + 期望匹配的元素 ID 抽取为 fixture
- 放在 `tests/wpt_selectors/` 下，每行 `selector|expected_ids`（CSV-like）
- 测试 harness 读取 fixture，对每个 selector 在固定 DOM 树上 querySelectorAll，比较结果

**Cargo.toml 升级**：

```toml
[package]
name = "muskitty-selectors"
version = "0.1.0"
edition = "2021"
description = "Selectors Level 4 parser and matching engine for Rust"
license = "Apache-2.0"
repository = "https://github.com/Ink-dark/MusKitty"
homepage = "https://github.com/Ink-dark/MusKitty"
documentation = "https://docs.rs/muskitty-selectors"
keywords = ["css", "selectors", "parser", "matcher", "web"]
categories = ["parser-implementations", "web-programming"]
rust-version = "1.82"

[dependencies]
muskitty-css = { path = "../muskitty-css", version = "0.2.0" }

[dev-dependencies]
muskitty-dom = { path = "../muskitty-dom" }
```

**README.md**（新）：
- 状态表（解析：§3-§6/§13/§15/§17 完整；匹配：§3/§5/§6/§13 完整，§7-§12 stub）
- 安装 + Quick Start
- Architecture（解析层用 muskitty-css tokenizer，匹配层用 Element trait 抽象）
- Spec 引用

**PROGRESS.md 更新**：
- 标记 Phase 2 子阶段 2（Selectors Level 4）完成
- 添加 SP-1..SP-8 完成情况表
- 更新下一步指向子阶段 3（CSS Values Module 值解析）

**质量门禁**（依次执行，任一失败禁止提交）：

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

SP-8 涉及 `Cargo.toml` 升级版本号 + README + PROGRESS.md（workspace 范围影响），额外执行：

```powershell
cargo fmt --check
cargo check --workspace
```

预期全绿，约 90 个测试通过（SP-1: 6 + SP-2: 10 + SP-3: 11 + SP-4: 12 + SP-5: 10 + SP-6: 12 + SP-7: 12 + SP-8: 25 + WPT 20 ≈ 118）。

**Commit**：
```
[selectors] SP-8: §18 matching engine + lib.rs API + WPT subset + v0.1.0
```

---

## 假设与决策（再确认）

1. **解析与匹配分层**：SP-1..SP-7 完成解析（不依赖 DOM），SP-8 完成匹配（依赖 DOM via trait）。这样 SP-1..SP-7 可以独立通过测试，不引入 DOM 依赖。
2. **muskitty-dom 仅 dev-dep**：`muskitty-selectors` 的 lib 本身不依赖 `muskitty-dom`，匹配抽象用 trait。`muskitty-dom` 作为 dev-dep 提供 `Rc<RefCell<Node>>` 的 Element trait impl，仅用于测试和作为参考实现。
3. **RefCell 借用处理**：`Rc<RefCell<Node>>` 实现 `Element` trait 时，因 `RefCell::borrow()` 返回的 `Ref` 不能跨函数返回，trait 方法返回 owned `String`（而非 `&str`）。性能不是 MVP 关注点；后续可加 interner 优化。
4. **An+B 反向形式**：spec 允许 `[<signed-integer>] <n> [<signed-integer>]`（如 `5n` 等价于 `5n`），需要在 SP-4 的 parse_an_plus_b 中处理。
5. **`:has()` 兄弟 combinator 子选择器**：spec 仍处于较不稳定状态。SP-5 解析完整支持，SP-8 匹配仅实现 descendant/child，兄弟延后。
6. **WPT 测试子集选择**：手动选择 20 个简单测试，避免 DOM 树构造复杂性。完整 WPT 集成留到下一阶段。
7. **`:scope` 与 `:host`**：解析完整，匹配 stub。`:scope` 在没有 scoping root 上下文时匹配 root；`:host` 仅在 shadow DOM 中有意义，stub 返回 false。

---

## 总结

8 个 SP batch，每个独立 commit + 质量门禁：

| SP | 内容 | 文件数 | 测试数 |
|----|------|--------|--------|
| SP-1 | §3 数据模型 + parser 框架 | 5 新 + 1 改 | 6 |
| SP-2 | §5/§6.5/§6.6 type/class/id 解析 | 4 新/改 | 10 |
| SP-3 | §6 attribute selectors 解析 | 2 改 + 1 新 | 11 |
| SP-4 | §13 tree-structural pseudo + An+B 解析 | 2 改 + 1 新 | 12 |
| SP-5 | §4 logical combinations 解析 | 3 改 + 1 新 | 10 |
| SP-6 | §15 combinators + 完整 complex 解析 | 3 改 + 1 新 | 12 |
| SP-7 | §17 specificity 计算 | 1 新 + 1 改 | 12 |
| SP-8 | §18 matching + lib API + WPT + v0.1.0 | 5 新 + 3 改 + README + PROGRESS | 25 + 20 WPT |
| **总计** | | | **~118 测试** |

SP-8 完成后自动进入下一轮 plan 模式：CSS Values Module 值解析（`D:\CSSWG\css-values-3\Overview.md` 或 `css-values-4`）。
