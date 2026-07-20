# SP-6: Combinators + Complex/Compound Selector Parsing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 验证 §15 4 种 combinator（Descendant / Child / NextSibling / SubsequentSibling）在 complex-selector 解析中的完整正确性，并修复空输入错误码，让 `parse_a_selector("")` 返回 `EmptySelector` 而非 `InvalidSelector`。

**Architecture:** SP-6 是一个**验证+收尾型** SP —— SP-2 的 `complex.rs` 实现已包含 4 种 combinator、隐式 descendant、trailing combinator 错误检测与 rightmost-first storage convention。SP-3/4/5 也都在 `complex.rs` 之上正常工作（`:not(.a > .b)` 与 `:has(> .a)` 都过测试）。SP-6 的工作是：(1) 写一组专门的 `parser_complex.rs` 测试覆盖 12 个边界用例；(2) 修复 `parse_a_selector` 在空/纯空白输入时返回 `EmptySelector`；(3) 顺便在 `complex.rs` 顶部更新文档注释，去掉"SP-6 will add ..."的过时说明。

**Tech Stack:** Rust 2021, MSRV 1.82, `muskitty-css = "0.4.0"` (for `tokenize` + `TokenStream`).

**Quality gate (per commit, sequential, all must pass):**
```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

**Auto-transition rule:** SP-6 commit 完成后**不**自动进入 SP-7 plan mode。SP-7 (§17 specificity) 的启动由用户显式触发。

---

## 现状评估（基于本次 plan 撰写时的代码快照）

| 文件 | 状态 | 说明 |
|------|------|------|
| `crates/muskitty-selectors/src/parser/complex.rs` | ✅ 完整 | 4 种 combinator + 隐式 descendant + trailing 错误检测已实现 |
| `crates/muskitty-selectors/src/parser/compound.rs` | ✅ 完整 | compound-selector + pseudo-compound 已实现 |
| `crates/muskitty-selectors/src/parser/list.rs` | ✅ 完整 | selector-list + forgiving-selector-list + trailing comma 错误已实现 |
| `crates/muskitty-selectors/src/parser/mod.rs::parse_a_selector` | ⚠️ 需小修 | 空输入返回 `InvalidSelector` 而非 `EmptySelector` |
| `crates/muskitty-selectors/src/error.rs` | ✅ 完整 | `EmptySelector` variant 已存在 |
| `crates/muskitty-selectors/src/parser/complex.rs` 顶部文档注释 | ⚠️ 过时 | 含"SP-6 will add additional edge-case handling..."需移除 |

**12 个测试预期：** 11 个应一次通过（代码已支持），1 个（`empty_string_fails`）需先修 `parse_a_selector` 才会通过。

---

## 文件结构

| 路径 | 操作 | 责任 |
|------|------|------|
| `crates/muskitty-selectors/tests/parser_complex.rs` | 新建 | 12 个 §15 combinator + complex selector 测试 |
| `crates/muskitty-selectors/src/parser/mod.rs` | 修改 | `parse_a_selector` 在 token 流仅含 EOF/whitespace 时返回 `EmptySelector` |
| `crates/muskitty-selectors/src/parser/complex.rs` | 修改（仅注释） | 移除过时的"SP-6 will add..."注释，更新现状说明 |

---

## Task 1: 写 12 个 failing/passing 测试

**Files:**
- Create: `crates/muskitty-selectors/tests/parser_complex.rs`

### 测试设计依据

- §15 L4360-4532 Combinators
- §3 L4664-4665 complex-selector grammar
- §3 L4704-4741 whitespace 边界规则
- §3 L1317-1347 Invalid selector error handling

### 右置存储约定（来自 `types::ComplexSelector` 文档）

> `units[0]` = subject (rightmost compound in source)
> `units[len-1]` = leftmost compound in source
> combinator 字段在右ward unit 上，连接它到 `units[idx+1]`
> 最左 unit (`units[len-1]`) 的 `combinator == None`

例：`a > b` 的 `units = [{ b, Some(Child) }, { a, None }]`

### - [ ] Step 1.1: 创建测试文件骨架

```rust
//! SP-6 unit tests for §15 combinators and complete complex-selector
//! parsing.
//!
//! Covers:
//! - §15 L4369: descendant (whitespace) combinator
//! - §15 L4376: child (`>`) combinator
//! - §15 L4383: next-sibling (`+`) combinator
//! - §15 L4390: subsequent-sibling (`~`) combinator
//! - §3 L4664-4665: complex-selector grammar (multiple compounds joined
//!   by combinators)
//! - §3 L1317-1347: invalid selector error handling (trailing combinator,
//!   trailing comma, empty input)
//!
//! Storage convention: `units[0]` is the subject (rightmost compound in
//! source), `units[len-1]` is the leftmost. The combinator on
//! `units[idx]` links it to `units[idx+1]` (the next leftward unit).
//! The leftmost unit has `combinator == None`.

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    Combinator, ComplexSelectorUnit, CompoundSelector, PseudoClass, SubclassSelector,
    TypeSelector, TypeSelectorName,
};

/// Helper: extract the single complex selector from a list that should
/// contain exactly one.
fn single_complex(list: &muskitty_selectors::types::SelectorList) -> &muskitty_selectors::types::ComplexSelector {
    assert_eq!(list.0.len(), 1, "expected 1 complex selector, got {}", list.0.len());
    &list.0[0]
}

/// Helper: assert the rightmost unit (subject) has the given tag name
/// and no combinator expectation (caller checks combinator separately).
fn assert_subject_tag(unit: &ComplexSelectorUnit, expected: &str) {
    let ts = unit.compound.type_selector.as_ref().expect("expected type selector");
    match &ts.name {
        TypeSelectorName::Name(n) => assert_eq!(n, expected),
        other => panic!("expected Name({:?}), got Universal", other),
    }
}

/// Helper: assert the rightmost unit (subject) has a single class subclass.
fn assert_subject_class(unit: &ComplexSelectorUnit, expected: &str) {
    assert_eq!(unit.compound.subclasses.len(), 1);
    match &unit.compound.subclasses[0] {
        SubclassSelector::Class(c) => assert_eq!(c.class, expected),
        other => panic!("expected Class, got {:?}", other),
    }
}
```

### - [ ] Step 1.2: 加入前 6 个 combinator 测试（应直接通过）

```rust
/// §3 L4664 + §15 L4360: `"div.foo"` → 1 unit, no combinator.
#[test]
fn single_compound() {
    let list = parse_a_selector("div.foo").expect("div.foo should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 1);
    assert!(cs.units[0].combinator.is_none());
    assert_subject_tag(&cs.units[0], "div");
    assert_subject_class(&cs.units[0], "foo");
}

/// §15 L4369: whitespace → Descendant combinator.
/// `"a b"` → units = [{ b, None }, { a, Some(Descendant) }]
/// Wait — storage convention: combinator on rightward unit (subject),
/// which is `b` (units[0]). Leftmost (`a`, units[1]) has None.
/// Actually re-read the convention: "combinator on units[idx] links it
/// to units[idx+1]". For 2-unit `a b`: units[0]=b (subject),
/// units[1]=a (leftmost). The combinator linking b to a is Descendant;
/// it lives on units[0] (the rightward one, b). units[1].combinator=None.
#[test]
fn descendant_whitespace() {
    let list = parse_a_selector("a b").expect("a b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    // units[0] = b (subject)
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::Descendant));
    // units[1] = a (leftmost)
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §15 L4376: `>` → Child combinator. `"a > b"` → 2 units with Child.
#[test]
fn child_explicit() {
    let list = parse_a_selector("a > b").expect("a > b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::Child));
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §15 L4383: `+` → NextSibling combinator.
#[test]
fn next_sibling() {
    let list = parse_a_selector("a + b").expect("a + b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::NextSibling));
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §15 L4390: `~` → SubsequentSibling combinator.
#[test]
fn subsequent_sibling() {
    let list = parse_a_selector("a ~ b").expect("a ~ b should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::SubsequentSibling));
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §3 L4664: three-part complex selector.
/// `"a b c"` → 3 units: [c (Desc), b (Desc), a (None)]
#[test]
fn three_part_descendant() {
    let list = parse_a_selector("a b c").expect("a b c should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 3);
    assert_subject_tag(&cs.units[0], "c");
    assert_eq!(cs.units[0].combinator, Some(Combinator::Descendant));
    assert_subject_tag(&cs.units[1], "b");
    assert_eq!(cs.units[1].combinator, Some(Combinator::Descendant));
    assert_subject_tag(&cs.units[2], "a");
    assert_eq!(cs.units[2].combinator, None);
}
```

### - [ ] Step 1.3: 加入后 6 个测试（mixed / pseudo-class / 错误）

```rust
/// §15 mixed combinators: `"a > b + c"` → 3 units.
/// Source order: a > b + c
/// Rightmost-first: units[0]=c (subject, combinator=NextSibling to b),
/// units[1]=b (combinator=Child to a), units[2]=a (None).
#[test]
fn mixed_combinators() {
    let list = parse_a_selector("a > b + c").expect("a > b + c should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 3);
    assert_subject_tag(&cs.units[0], "c");
    assert_eq!(cs.units[0].combinator, Some(Combinator::NextSibling));
    assert_subject_tag(&cs.units[1], "b");
    assert_eq!(cs.units[1].combinator, Some(Combinator::Child));
    assert_subject_tag(&cs.units[2], "a");
    assert_eq!(cs.units[2].combinator, None);
}

/// §15 + §13: pseudo-class on rightmost compound.
/// `"a > b:hover"` → 2 units, subject `b` has :hover subclass.
#[test]
fn combinator_with_pseudo_class() {
    let list = parse_a_selector("a > b:hover").expect("a > b:hover should parse");
    let cs = single_complex(&list);
    assert_eq!(cs.units.len(), 2);
    assert_subject_tag(&cs.units[0], "b");
    assert_eq!(cs.units[0].combinator, Some(Combinator::Child));
    // subject has :hover pseudo-class as a subclass
    let pseudo = cs.units[0]
        .compound
        .subclasses
        .iter()
        .find_map(|s| match s {
            SubclassSelector::PseudoClass(pc) => Some(pc),
            _ => None,
        })
        .expect("expected :hover pseudo-class");
    assert_eq!(pseudo.name, "hover");
    assert!(pseudo.argument.is_none());
    assert_subject_tag(&cs.units[1], "a");
    assert_eq!(cs.units[1].combinator, None);
}

/// §3 L1317-1347: trailing combinator is invalid.
/// `"a >"` → Err (any SelectorParseError variant).
#[test]
fn trailing_combinator_fails() {
    let result = parse_a_selector("a >");
    assert!(result.is_err(), "trailing combinator should fail");
}

/// §3 L4651-4653: selector list with 3 items.
/// `"a, b, c"` → SelectorList with 3 complex selectors.
#[test]
fn selector_list_three_items() {
    let list = parse_a_selector("a, b, c").expect("a, b, c should parse");
    assert_eq!(list.0.len(), 3);
    // Each should be a single-compound complex selector.
    for (i, cs) in list.0.iter().enumerate() {
        assert_eq!(cs.units.len(), 1);
        let expected = match i {
            0 => "a",
            1 => "b",
            2 => "c",
            _ => unreachable!(),
        };
        assert_subject_tag(&cs.units[0], expected);
    }
}

/// §3 L4651-4653: trailing comma is invalid.
/// `"a,"` → Err.
#[test]
fn trailing_comma_fails() {
    let result = parse_a_selector("a,");
    assert!(result.is_err(), "trailing comma should fail");
}

/// §3 L1317-1347 + error::EmptySelector: empty input → Err(EmptySelector).
/// This test will FAIL until Task 2 lands the parse_a_selector fix.
#[test]
fn empty_string_fails() {
    let result = parse_a_selector("");
    assert!(
        result.is_err(),
        "empty input should fail with EmptySelector"
    );
    // Strict check: must be EmptySelector specifically, not InvalidSelector.
    use muskitty_selectors::error::SelectorParseError;
    assert!(
        matches!(result, Err(SelectorParseError::EmptySelector)),
        "expected EmptySelector, got {:?}",
        result
    );
}
```

### - [ ] Step 1.4: 跑测试，预期 1 fail（empty_string_fails）

```powershell
cargo test -p muskitty-selectors --test parser_complex
```

**Expected output:**
- 11 tests pass (single_compound, descendant_whitespace, child_explicit, next_sibling, subsequent_sibling, three_part_descendant, mixed_combinators, combinator_with_pseudo_class, trailing_combinator_fails, selector_list_three_items, trailing_comma_fails)
- 1 test fails: `empty_string_fails` — actual error is `InvalidSelector("expected a compound selector, got end of input")`, not `EmptySelector`

如果出现**任何其他**测试失败，**先停下来按 spec 调试代码**，不要绕过失败。可能的失败原因与排查方向：

- 如果 `mixed_combinators` 失败：检查 `complex.rs` 第 88-102 行的 explicit combinator 分支是否正确把 combinator 放到新 unit 上而不是上一个 unit。
- 如果 `combinator_with_pseudo_class` 失败：检查 `compound.rs` 的 subclass 循环是否在 `:hover` 后停止（应继续循环直到 None）。
- 如果 `trailing_combinator_fails` 失败：检查 `complex.rs` 第 109-113 行的 `is_complex_terminator` 检查。

### - [ ] Step 1.5: Commit 测试文件

```powershell
git add crates/muskitty-selectors/tests/parser_complex.rs
git status  # 确认只有这一个新文件
```

**不**在此时 commit —— 测试还有 1 个 failing，违反"每个 commit 必须全绿"的质量门约束。先做 Task 2 修复，再一起 commit。

---

## Task 2: 修 `parse_a_selector` 让空输入返回 `EmptySelector`

**Files:**
- Modify: `crates/muskitty-selectors/src/parser/mod.rs`

### - [ ] Step 2.1: 读取当前 mod.rs 内容确认

```powershell
# Already done during planning — current parse_a_selector:
# pub fn parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
#     let tokens = muskitty_css::tokenize(source);
#     let mut stream = TokenStream::new(tokens);
#     let list = list::parse_selector_list(&mut stream)?;
#     stream.discard_whitespace();
#     if !stream.is_empty() {
#         return Err(SelectorParseError::InvalidSelector(format!(
#             "trailing tokens after selector: {:?}",
#             stream.next_token()
#         )));
#     }
#     Ok(list)
# }
```

### - [ ] Step 2.2: 修改 parse_a_selector 加入空输入预检

**Spec 依据：** §3 L1317-1347 "Invalid Selectors and Error Handling" —— 空输入或仅空白不是合法的 selector-list，应当返回结构化错误而非通用 InvalidSelector。

`error::EmptySelector` 的 doc comment 已经明确："The selector list is empty (zero-length input or all whitespace)."——所以应该覆盖**纯空白输入**也返回 `EmptySelector`，不只是 `""`。

**Edit:**
```rust
pub fn parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError> {
    let tokens = muskitty_css::tokenize(source);
    let mut stream = TokenStream::new(tokens);

    // §3 L1317-1347: empty input or whitespace-only input is not a
    // valid selector list. Distinguish this case from a structurally
    // invalid selector by returning EmptySelector.
    stream.discard_whitespace();
    if matches!(stream.next_token(), Token::Eof) {
        return Err(SelectorParseError::EmptySelector);
    }

    let list = list::parse_selector_list(&mut stream)?;
    // Reject trailing garbage (whitespace is fine).
    stream.discard_whitespace();
    if !stream.is_empty() {
        return Err(SelectorParseError::InvalidSelector(format!(
            "trailing tokens after selector: {:?}",
            stream.next_token()
        )));
    }
    Ok(list)
}
```

### - [ ] Step 2.3: 跑测试确认 empty_string_fails 通过

```powershell
cargo test -p muskitty-selectors --test parser_complex
```

**Expected:** 12 tests pass (0 failed).

### - [ ] Step 2.4: 跑全套测试确认没回归

```powershell
cargo test -p muskitty-selectors
```

**Expected:** 全部测试通过。SP-1..SP-5 的现有测试（约 49 个）+ SP-6 新增 12 个 = 约 61 个。

**潜在回归点：**
- `parser_simple.rs` 里的 `parse_a_selector("")` 旧测试如果存在且断言 `InvalidSelector`，需要更新为 `EmptySelector`。在 SP-1..SP-5 的测试中查找 `parse_a_selector("")` 调用：

```powershell
# 用 Grep 工具搜索，不要用 findstr
# pattern: parse_a_selector\(""\)
# path: d:\Muskitty\crates\muskitty-selectors\tests
```

如果找到旧测试，按同样规则更新断言为 `EmptySelector`。

---

## Task 3: 更新 complex.rs 顶部文档注释

**Files:**
- Modify: `crates/muskitty-selectors/src/parser/complex.rs`

### - [ ] Step 3.1: 读取 complex.rs 当前顶部注释

`complex.rs` 第 1-31 行的文档注释包含一句过时声明：

```text
//! SP-5 scope: parses one or more compound selectors joined by the
//! four §15 combinators (Descendant / Child / NextSibling /
//! SubsequentSibling). Trailing combinators (e.g. `a >`) produce an
//! `InvalidSelector` error. SP-6 will add additional edge-case
//! handling (mixed combinators, empty-input rejection, etc.).
```

"SP-6 will add ..." 现在已经过时（SP-6 就是现在）。同时"SP-5 scope"应该改为"SP-2..SP-6 scope"。

### - [ ] Step 3.2: 修改注释反映 SP-6 已完成状态

把第 9-13 行替换为：

```text
//! SP-2..SP-6 scope: parses one or more compound selectors joined by
//! the four §15 combinators (Descendant / Child / NextSibling /
//! SubsequentSibling). Trailing combinators (e.g. `a >`) produce an
//! `InvalidSelector` error. Mixed combinators (`a > b + c`), pseudo-
//! class-terminated compounds (`a > b:hover`), and selector lists
//! with trailing-comma / trailing-combinator / empty-input rejection
//! are all handled (see tests/parser_complex.rs).
```

### - [ ] Step 3.3: 跑质量门验证全绿

```powershell
cargo fmt -p muskitty-selectors -- --check
cargo test -p muskitty-selectors
cargo check -p muskitty-selectors
cargo clippy -p muskitty-selectors --all-targets -- -D warnings
```

四步必须全绿。如果有 warning，先修复再重跑。

---

## Task 4: Commit

### - [ ] Step 4.1: 暂存改动文件

```powershell
git add crates/muskitty-selectors/tests/parser_complex.rs
git add crates/muskitty-selectors/src/parser/mod.rs
git add crates/muskitty-selectors/src/parser/complex.rs
git status
```

确认：
- `crates/muskitty-selectors/tests/parser_complex.rs` (new file)
- `crates/muskitty-selectors/src/parser/mod.rs` (modified)
- `crates/muskitty-selectors/src/parser/complex.rs` (modified, only doc comment)

### - [ ] Step 4.2: Commit with structured message

```powershell
git commit -m "[selectors] SP-6: §15 combinators + complete complex/compound selector parsing" -m "Add tests/parser_complex.rs (12 tests) covering 4 combinator types (Descendant/Child/NextSibling/SubsequentSibling), mixed combinators (a > b + c), pseudo-class-terminated compounds (a > b:hover), trailing-combinator error, trailing-comma error, and empty-input rejection." -m "Fix parse_a_selector to return EmptySelector (not InvalidSelector) for empty or whitespace-only input, per §3 L1317-1347. Update complex.rs doc comment to reflect SP-6 completion (remove stale 'SP-6 will add' note)." -m "Spec ref: §15 L4360-4532 Combinators, §3 L4664-4665 complex-selector grammar, §3 L4704-4741 whitespace rules, §3 L1317-1347 Invalid Selectors and Error Handling."
```

注意：PowerShell 不支持 heredoc，所以用多个 `-m` 标签实现多段落 commit message。

### - [ ] Step 4.3: 验证 commit 成功

```powershell
git log --oneline -3
```

应看到 `[selectors] SP-6: §15 combinators + complete complex/compound selector parsing` 在最顶部。

### - [ ] Step 4.4: 推送到远端

```powershell
git push origin main
```

**注意：** 主仓库（d:\Muskitty）才推。`crates/muskitty-selectors` 不是独立 git 仓库，是主仓库的一部分，所以推送主仓库即可。

---

## Self-Review (plan author)

### 1. Spec 覆盖

| SP-6 计划项 | 覆盖任务 |
|----------|--------|
| §15 L4369 descendant (whitespace) | Task 1 Step 1.2 — `descendant_whitespace` |
| §15 L4376 child (`>`) | Task 1 Step 1.2 — `child_explicit` |
| §15 L4383 next-sibling (`+`) | Task 1 Step 1.2 — `next_sibling` |
| §15 L4390 subsequent-sibling (`~`) | Task 1 Step 1.2 — `subsequent_sibling` |
| §3 L4664 complex-selector grammar (3+ units) | Task 1 Step 1.2 — `three_part_descendant` |
| 混合 combinator | Task 1 Step 1.3 — `mixed_combinators` |
| pseudo-class 在 rightmost compound | Task 1 Step 1.3 — `combinator_with_pseudo_class` |
| trailing combinator 错误 | Task 1 Step 1.3 — `trailing_combinator_fails` |
| selector list 3 items | Task 1 Step 1.3 — `selector_list_three_items` |
| trailing comma 错误 | Task 1 Step 1.3 — `trailing_comma_fails` |
| 空输入错误（`EmptySelector`） | Task 1 Step 1.3 + Task 2 |
| single compound (no combinator) | Task 1 Step 1.2 — `single_compound` |

12 个测试用例全部映射到任务。✅

### 2. Placeholder 扫描

- ❌ "TBD" / "TODO" — 无
- ❌ "implement later" — 无
- ❌ "add appropriate error handling" — 无
- ❌ "fill in details" — 无
- ❌ "similar to Task N" — 无
- ❌ 未定义的类型/函数 — 所有引用的 `parse_a_selector` / `Combinator` / `ComplexSelectorUnit` / `CompoundSelector` / `PseudoClass` / `SubclassSelector` / `TypeSelector` / `TypeSelectorName` / `SelectorParseError::EmptySelector` 都在现有代码中已定义（见现状评估表）

✅ 无 placeholder。

### 3. 类型一致性

| 引用项 | 定义位置 |
|--------|--------|
| `parse_a_selector(source: &str) -> Result<SelectorList, SelectorParseError>` | `crates/muskitty-selectors/src/parser/mod.rs:30` |
| `SelectorList(pub Vec<ComplexSelector>)` | `types.rs:29` |
| `ComplexSelector { units: Vec<ComplexSelectorUnit> }` | `types.rs:45` |
| `ComplexSelectorUnit { compound, combinator: Option<Combinator> }` | `types.rs:55` |
| `CompoundSelector { type_selector, subclasses, pseudo_compounds }` | `types.rs:69` |
| `Combinator::{Descendant, Child, NextSibling, SubsequentSibling}` | `types.rs:87` |
| `TypeSelector { ns_prefix, name: TypeSelectorName }` | `types.rs:100` |
| `TypeSelectorName::{Name(String), Universal}` | `types.rs:112` |
| `SubclassSelector::{Id, Class, Attribute, PseudoClass}` | `types.rs:140` |
| `PseudoClass { name, argument }` | `types.rs:238` |
| `SelectorParseError::EmptySelector` | `error.rs:42` |

✅ 所有类型签名与现有定义一致。

---

## 总结

- 3 个 task，4-5 个 commit step，预计 12 个新测试 + 1 处小修 + 1 处注释更新
- 不创建新源文件（除测试外）
- 不升级 Cargo.toml 版本（v0.1.0 已就绪，留给 SP-8 升级）
- **不**自动进入 SP-7 plan mode —— SP-6 commit + push 后停止，等待用户显式触发下一轮
