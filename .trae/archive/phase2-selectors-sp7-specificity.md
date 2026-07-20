# Selectors Level 4 — SP-7: §17 Specificity Calculation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement §17 "Calculating a selector's specificity" for `muskitty-selectors`, exposing `ComplexSelector::specificity()` and `SelectorList::specificity_max()` APIs that return a strongly-typed `Specificity { a, b, c }` value with full `Ord` support.

**Architecture:** Add a new `specificity` module that walks the existing selector data model (`SelectorList` / `ComplexSelector` / `CompoundSelector` / `SubclassSelector` / `PseudoClass` / `PseudoElement`) and computes the A/B/C triplet per §17 L4534-4633. The data model needs one small extension: `PseudoClassArgument::AnPlusB` must carry an optional `SelectorList` so that `:nth-child(An+B of S)` can be represented (§17 L4560-4564 adds max-of-S to the pseudo-class's own specificity when `of S` is present).

**Tech Stack:** Rust 2021, `muskitty-css` v0.4.0 (for `Token`/`TokenStream`), existing `muskitty-selectors` parser/types.

**Spec source:** `D:\CSSWG\selectors-4\Overview.md`, §17 L4534-4633.

---

## §17 Spec Summary (authoritative reference for this plan)

Per L4536-4548, a selector's specificity is `(A, B, C)` where:
- `A` = number of ID selectors
- `B` = number of class selectors + attribute selectors + pseudo-classes
- `C` = number of type selectors + pseudo-elements
- Universal selector (`*`) is ignored

Per L4550-4566, special pseudo-classes have non-default specificity rules:
- `:is()`, `:not()`, `:has()` → replaced by max specificity of complex selectors in argument (L4555-4558)
- `:nth-child()`, `:nth-last-child()` → (pseudo-class itself = 1×B) **plus** max specificity of complex selectors in `of S` argument if present (L4560-4564)
- `:where()` → replaced by zero (L4566)

Per L4598-4605, comparison is lexicographic on (A, B, C).

Worked examples (L4613-4628):
- `*` → (0,0,0)
- `LI` → (0,0,1)
- `UL LI` → (0,0,2)
- `UL OL+LI` → (0,0,3)
- `H1 + *[REL=up]` → (0,1,1)
- `UL OL LI.red` → (0,1,3)
- `LI.red.level` → (0,2,1)
- `#x34y` → (1,0,0)
- `#s12:not(FOO)` → (1,0,1)
- `.foo :is(.bar, #baz)` → (1,1,0)

Additional examples (L4570-4593):
- `:is(em, #foo)` → (1,0,0)
- `.qux:where(em, #foo#bar#baz)` → (0,1,0)
- `:nth-child(even of li, .item)` → (0,2,0)
- `:not(em, strong#foo)` → (1,0,1)

---

## File Structure

- **Create:** `crates/muskitty-selectors/src/specificity.rs` — new module defining `Specificity` type and the calculation functions
- **Modify:** `crates/muskitty-selectors/src/lib.rs` — add `pub mod specificity;` and re-export `Specificity`
- **Modify:** `crates/muskitty-selectors/src/types.rs` — extend `PseudoClassArgument::AnPlusB` variant to `(AnPlusB, Option<SelectorList>)`
- **Modify:** `crates/muskitty-selectors/src/parser/simple.rs` — parse `An+B of S` for `:nth-child` / `:nth-last-child`
- **Modify:** `crates/muskitty-selectors/src/parser/an_plus_b.rs` — update doc to mention the `of S` extension handled in `simple.rs`
- **Create:** `crates/muskitty-selectors/tests/specificity.rs` — comprehensive tests covering every worked example in §17 L4613-4628 and L4570-4593

---

## Auto-transition rule

**SP-7 commit 完成后，不自动进入 SP-8 plan mode**。下一步是按用户指示剥离 `muskitty-selectors` 到独立 git 仓库（参考 `muskitty-css-parser` 剥离模式）。SP-8 (§18 matching engine) 留待独立仓库启动后再开 plan mode。

---

## Task 1: Extend `PseudoClassArgument::AnPlusB` to carry optional `of S` selector list

**Files:**
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\types.rs:248-258`

### - [ ] Step 1.1: Read current `PseudoClassArgument` enum

Open `d:\Muskitty\crates\muskitty-selectors\src\types.rs` lines 248-258. Current state:

```rust
pub enum PseudoClassArgument {
    /// For `:nth-child(An+B)`, `:nth-last-child(An+B)`,
    /// `:nth-of-type(An+B)`, `:nth-last-of-type(An+B)`.
    AnPlusB(AnPlusB),
    /// For `:is()`, `:not()`, `:where()`, `:has()` — a selector list.
    SelectorList(SelectorList),
    /// For `:lang(*)`, `:dir(*)`, `:current(*)`, etc. — preserved
    /// component values for caller-side interpretation.
    Raw(Vec<Token>),
}
```

### - [ ] Step 1.2: Modify the `AnPlusB` variant to carry `Option<SelectorList>`

Edit `d:\Muskitty\crates\muskitty-selectors\src\types.rs`. Replace the `AnPlusB` arm with:

```rust
pub enum PseudoClassArgument {
    /// For `:nth-child(An+B [of S]?)`, `:nth-last-child(An+B [of S]?)`,
    /// `:nth-of-type(An+B)`, `:nth-last-of-type(An+B)`. The optional
    /// `SelectorList` carries the `of S` argument when present
    /// (§13.3 L3968, §13.4 L4077). Always `None` for `:nth-of-type`
    /// and `:nth-last-of-type` (those do not accept `of S` syntax).
    AnPlusB(AnPlusB, Option<SelectorList>),
    /// For `:is()`, `:not()`, `:where()`, `:has()` — a selector list.
    SelectorList(SelectorList),
    /// For `:lang(*)`, `:dir(*)`, `:current(*)`, etc. — preserved
    /// component values for caller-side interpretation.
    Raw(Vec<Token>),
}
```

### - [ ] Step 1.3: Update the parser site that constructs `AnPlusB` to pass `None`

Open `d:\Muskitty\crates\muskitty-selectors\src\parser\simple.rs` line 702-705. Current state:

```rust
"nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
    let an_plus_b = parse_an_plus_b(stream)?;
    Ok(PseudoClassArgument::AnPlusB(an_plus_b))
}
```

Replace with (we'll extend this in Task 2 to actually parse `of S`):

```rust
"nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" => {
    let an_plus_b = parse_an_plus_b(stream)?;
    // Task 2 will parse the optional `of S` selector list for
    // nth-child / nth-last-child. For now, always `None`.
    Ok(PseudoClassArgument::AnPlusB(an_plus_b, None))
}
```

### - [ ] Step 1.4: Run check to verify compile

Run: `cargo check -p muskitty-selectors`
Expected: compile succeeds with no errors (no other call sites construct `AnPlusB`).

### - [ ] Step 1.5: Run full test suite to verify no regression

Run: `cargo test -p muskitty-selectors`
Expected: 61 tests pass (same as pre-SP-7 baseline).

### - [ ] Step 1.6: Commit

```bash
git add crates/muskitty-selectors/src/types.rs crates/muskitty-selectors/src/parser/simple.rs
git commit -m "[selectors] SP-7 task 1: extend PseudoClassArgument::AnPlusB to carry Option<SelectorList>"
```

---

## Task 2: Parse `An+B of S` syntax for `:nth-child` / `:nth-last-child`

**Files:**
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\parser\simple.rs:697-726`
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\parser\an_plus_b.rs` (doc comment only)

### - [ ] Step 2.1: Write the failing test

Create file `d:\Muskitty\crates\muskitty-selectors\tests\parser_nth_of.rs` with content:

```rust
//! SP-7 Task 2: verify `:nth-child(An+B of S)` parsing.
//!
//! Per §13.3 L3968, `:nth-child()` and `:nth-last-child()` accept an
//! optional `of S` clause where S is a selector list. §17 L4560-4564
//! uses this list for the special specificity rule. `:nth-of-type()`
//! and `:nth-last-of-type()` do NOT accept `of S` per §13.6/§13.7.

use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{
    ComplexSelectorUnit, CompoundSelector, PseudoClass, PseudoClassArgument, SelectorList,
    SubclassSelector,
};

fn single_compound(list: &SelectorList) -> &CompoundSelector {
    assert_eq!(list.0.len(), 1);
    let unit: &ComplexSelectorUnit = &list.0[0].units[0];
    assert!(unit.combinator.is_none());
    &unit.compound
}

fn single_pseudo_class(compound: &CompoundSelector) -> &PseudoClass {
    assert_eq!(compound.subclasses.len(), 1);
    match &compound.subclasses[0] {
        SubclassSelector::PseudoClass(pc) => pc,
        other => panic!("expected PseudoClass, got {:?}", other),
    }
}

/// §13.3 L3968: `:nth-child(2n of .a, .b)` — the `of S` clause captures
/// a 2-element SelectorList argument alongside the An+B value.
#[test]
fn nth_child_with_of_selector_list() {
    let list =
        parse_a_selector(":nth-child(2n of .a, .b)").expect(":nth-child(2n of .a, .b) parses");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "nth-child");
    match &pc.argument {
        Some(PseudoClassArgument::AnPlusB(anb, Some(of_s))) => {
            assert_eq!(anb.a, 2);
            assert_eq!(anb.b, 0);
            assert_eq!(of_s.0.len(), 2, "expected 2 selectors in of S");
        }
        other => panic!(
            "expected AnPlusB(2n, Some(list)), got {:?}",
            other
        ),
    }
}

/// §13.3 L3968: `:nth-child(even)` without `of S` — the optional list
/// is `None`.
#[test]
fn nth_child_without_of() {
    let list = parse_a_selector(":nth-child(even)").expect(":nth-child(even) parses");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "nth-child");
    match &pc.argument {
        Some(PseudoClassArgument::AnPlusB(anb, None)) => {
            assert_eq!(anb.a, 2);
            assert_eq!(anb.b, 0);
        }
        other => panic!("expected AnPlusB(2n, None), got {:?}", other),
    }
}

/// §13.4 L4077: `:nth-last-child(odd of .x)` — same parsing rule as
/// `:nth-child()` for the `of S` clause.
#[test]
fn nth_last_child_with_of() {
    let list =
        parse_a_selector(":nth-last-child(odd of .x)").expect(":nth-last-child(odd of .x) parses");
    let compound = single_compound(&list);
    let pc = single_pseudo_class(compound);
    assert_eq!(pc.name, "nth-last-child");
    match &pc.argument {
        Some(PseudoClassArgument::AnPlusB(anb, Some(of_s))) => {
            assert_eq!(anb.a, 2);
            assert_eq!(anb.b, 1);
            assert_eq!(of_s.0.len(), 1);
        }
        other => panic!("expected AnPlusB(2n+1, Some(list)), got {:?}", other),
    }
}

/// §13.6/§13.7: `:nth-of-type(2n)` does NOT accept `of S` syntax. If
/// the parser encounters `of` after An+B, it must fail.
#[test]
fn nth_of_type_rejects_of_clause() {
    let result = parse_a_selector(":nth-of-type(2n of .a)");
    assert!(
        result.is_err(),
        ":nth-of-type should not accept 'of S' clause"
    );
}
```

### - [ ] Step 2.2: Run test to verify it fails

Run: `cargo test -p muskitty-selectors --test parser_nth_of`
Expected: `nth_child_with_of_selector_list`, `nth_last_child_with_of` FAIL (parser doesn't accept `of S` yet; `PseudoClassArgument::AnPlusB(_, None)` is always returned).
`nth_child_without_of`, `nth_of_type_rejects_of_clause` may PASS or FAIL depending on existing parser strictness.

### - [ ] Step 2.3: Extend parser to handle `of S` for `:nth-child` / `:nth-last-child`

Open `d:\Muskitty\crates\muskitty-selectors\src\parser\simple.rs`. Find the `parse_pseudo_class_argument` function (around line 700). Replace the `nth-child | nth-last-child | nth-of-type | nth-last-of-type` arm with:

```rust
"nth-child" | "nth-last-child" => {
    // §13.3 L3968 / §13.4 L4077: `An+B [of S]?`. Only nth-child and
    // nth-last-child accept the `of S` clause.
    let an_plus_b = parse_an_plus_b(stream)?;
    let of_s = parse_optional_of_selector_list(stream)?;
    Ok(PseudoClassArgument::AnPlusB(an_plus_b, of_s))
}
"nth-of-type" | "nth-last-of-type" => {
    // §13.6 / §13.7: no `of S` syntax. If the user wrote `of`, it's
    // an error — the trailing tokens will be caught by the closing
    // `)` verification step in `parse_pseudo_class_or_legacy`.
    let an_plus_b = parse_an_plus_b(stream)?;
    Ok(PseudoClassArgument::AnPlusB(an_plus_b, None))
}
```

### - [ ] Step 2.4: Add the `parse_optional_of_selector_list` helper

Add this helper function below `parse_pseudo_class_argument` in `d:\Muskitty\crates\muskitty-selectors\src\parser\simple.rs` (before the `KNOWN_PSEUDO_CLASSES` const, or wherever fits the file style):

```rust
/// §13.3 L3968 / §13.4 L4077: parse the optional `of S` clause of
/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)`.
///
/// Pre-condition: An+B has already been consumed; the stream cursor
/// sits just past the B-part of An+B.
///
/// Returns:
/// - `Ok(Some(SelectorList))` — `of S` clause present and parsed.
/// - `Ok(None)` — no `of` keyword; the clause was omitted.
///
/// The closing `)` is left unconsumed for the caller to verify (same
/// convention as other pseudo-class argument parsers).
fn parse_optional_of_selector_list(
    stream: &mut TokenStream,
) -> Result<Option<SelectorList>, SelectorParseError> {
    stream.discard_whitespace();
    match stream.next_token() {
        // No `of` keyword — clause is absent. Leave the stream
        // positioned at the closing `)` (or whatever terminator
        // follows) for the caller.
        Token::CloseParen | Token::Eof => Ok(None),
        // `of` keyword (case-insensitive per CSS ident folding).
        Token::Ident(ref s) if s.eq_ignore_ascii_case("of") => {
            stream.discard_token();
            // §13.3 L3968: S is a <selector-list> (non-forgiving).
            // Reuse parse_selector_list from list.rs.
            let list = crate::parser::list::parse_selector_list(stream)?;
            Ok(Some(list))
        }
        // Anything else after An+B is a structural error.
        _ => Err(SelectorParseError::InvalidSelector(format!(
            "expected `of` or `)` after An+B in :nth-child/:nth-last-child argument"
        ))),
    }
}
```

### - [ ] Step 2.5: Update `an_plus_b.rs` doc comment to mention `of S` is handled elsewhere

Open `d:\Muskitty\crates\muskitty-selectors\src\parser\an_plus_b.rs`. Add a Note section to the existing module doc comment block (insert after the existing `# Note on the `<signless-integer>` distinction` block, near line 39):

```rust
//! # Note on the `of S` clause
//!
//! This module parses only the An+B production. The optional
//! `of <selector-list>` clause accepted by `:nth-child()` and
//! `:nth-last-child()` (§13.3 L3968 / §13.4 L4077) is parsed by
//! [`crate::parser::simple::parse_optional_of_selector_list`]
//! after this function returns.
```

### - [ ] Step 2.6: Run the new tests to verify they pass

Run: `cargo test -p muskitty-selectors --test parser_nth_of`
Expected: all 4 tests PASS.

### - [ ] Step 2.7: Run the full test suite to verify no regression

Run: `cargo test -p muskitty-selectors`
Expected: 61 (pre-SP-7 baseline) + 4 (new) = 65 tests PASS.

### - [ ] Step 2.8: Commit

```bash
git add crates/muskitty-selectors/src/parser/simple.rs crates/muskitty-selectors/src/parser/an_plus_b.rs crates/muskitty-selectors/tests/parser_nth_of.rs
git commit -m "[selectors] SP-7 task 2: parse :nth-child(An+B of S) syntax per §13.3"
```

---

## Task 3: Create `specificity.rs` skeleton with `Specificity` type

**Files:**
- Create: `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\lib.rs:25-27`

### - [ ] Step 3.1: Write a failing test for `Specificity` ordering

Create file `d:\Muskitty\crates\muskitty-selectors\tests\specificity.rs` with this initial content (we'll grow it through Tasks 4-7):

```rust
//! SP-7 §17 specificity calculation tests.
//!
//! Covers §17 L4534-4633: the A/B/C triplet computation, comparison
//! rules, and the special cases for `:is`/`:not`/`:has`/`:where`/
//! `:nth-child`/`:nth-last-child`.

use muskitty_selectors::specificity::Specificity;

/// §17 L4598-4605: lexicographic comparison on (A, B, C).
#[test]
fn specificity_ordering() {
    assert!(Specificity::new(1, 0, 0) > Specificity::new(0, 99, 99));
    assert!(Specificity::new(0, 2, 0) > Specificity::new(0, 1, 99));
    assert!(Specificity::new(0, 0, 2) > Specificity::new(0, 0, 1));
    assert_eq!(Specificity::new(1, 2, 3), Specificity::new(1, 2, 3));
    // Default is (0,0,0) — the universal-selector / `*` specificity.
    assert_eq!(Specificity::default(), Specificity::new(0, 0, 0));
}
```

### - [ ] Step 3.2: Run test to verify it fails

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: compile error — module `muskitty_selectors::specificity` does not exist.

### - [ ] Step 3.3: Create the `specificity.rs` module with `Specificity` type

Create file `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs` with content:

```rust
//! §17 Calculating a selector's specificity.
//!
//! Implements the A/B/C triplet computation per Selectors Level 4
//! §17 L4534-4633. Specificity is computed for a parsed selector
//! (independent of any element); element-matching specificity is
//! therefore identical to selector specificity (SP-8 will define the
//! matching-side contract).
//!
//! # Components
//!
//! Per §17 L4539-4542:
//! - `A` = number of ID selectors in the selector
//! - `B` = number of class selectors + attribute selectors + pseudo-classes
//! - `C` = number of type selectors + pseudo-elements
//! - The universal selector (`*`) is ignored.
//!
//! # Special pseudo-classes
//!
//! Per §17 L4550-4566:
//! - `:is()`, `:not()`, `:has()` → specificity is replaced by the
//!   max specificity of the complex selectors in the argument list.
//! - `:nth-child()`, `:nth-last-child()` → specificity is the
//!   pseudo-class itself (1×B) **plus** the max specificity of the
//!   complex selectors in the `of S` argument (if present).
//! - `:where()` → specificity is replaced by zero.
//!
//! # Comparison
//!
//! Per §17 L4598-4605: lexicographic on (A, B, C).
//!
//! # Spec source
//!
//! `D:\CSSWG\selectors-4\Overview.md`, §17 L4534-4633.

use crate::types::{ComplexSelector, CompoundSelector, PseudoClass, SelectorList, SubclassSelector};

/// §17 L4534-4548: A selector's specificity, expressed as the (A, B, C)
/// triplet. Comparison is lexicographic per L4598-4605.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Specificity {
    /// §17 L4539: count of ID selectors.
    pub a: u32,
    /// §17 L4540: count of class selectors + attribute selectors +
    /// pseudo-classes.
    pub b: u32,
    /// §17 L4541: count of type selectors + pseudo-elements.
    pub c: u32,
}

impl Specificity {
    /// Construct a new specificity triplet.
    pub const fn new(a: u32, b: u32, c: u32) -> Self {
        Self { a, b, c }
    }

    /// §17 L4598-4605: lexicographic comparison. Used by `Ord`.
    /// Returns `true` if `self` is strictly more specific than `other`.
    fn gt(self, other: Self) -> bool {
        if self.a != other.a {
            return self.a > other.a;
        }
        if self.b != other.b {
            return self.b > other.b;
        }
        self.c > other.c
    }
}

impl Ord for Specificity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        if self.gt(*other) {
            Ordering::Greater
        } else if self == other {
            Ordering::Equal
        } else {
            Ordering::Less
        }
    }
}

impl PartialOrd for Specificity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::ops::Add for Specificity {
    type Output = Self;
    /// Component-wise addition. Used to sum the specificities of
    /// multiple compound selectors in a complex selector.
    fn add(self, other: Self) -> Self {
        Self {
            a: self.a + other.a,
            b: self.b + other.b,
            c: self.c + other.c,
        }
    }
}

impl std::ops::AddAssign for Specificity {
    fn add_assign(&mut self, other: Self) {
        self.a += other.a;
        self.b += other.b;
        self.c += other.c;
    }
}

impl Specificity {
    /// §17 L4547-4548: for a selector list, the specificity in effect
    /// is that of the most specific selector in the list that matches.
    /// Since we have no element here (matching is SP-8), this returns
    /// the max specificity over all complex selectors in the list.
    pub fn max_of_list(list: &SelectorList) -> Self {
        list.0
            .iter()
            .map(specificity_of_complex)
            .max()
            .unwrap_or_default()
    }
}

// Placeholder — full implementations added in Tasks 4 and 5.
pub fn specificity_of_complex(_cs: &ComplexSelector) -> Specificity {
    Specificity::default()
}

fn _specificity_of_compound(_compound: &CompoundSelector) -> Specificity {
    Specificity::default()
}

fn _specificity_of_pseudo_class(_pc: &PseudoClass) -> Specificity {
    Specificity::default()
}

#[allow(dead_code)]
fn _classify_subclass(_s: &SubclassSelector) -> Specificity {
    Specificity::default()
}
```

### - [ ] Step 3.4: Wire the module into `lib.rs`

Open `d:\Muskitty\crates\muskitty-selectors\src\lib.rs`. Add `pub mod specificity;` after `pub mod parser;` (around line 26). Also update the top-level doc comment to mention the new module. The relevant block becomes:

```rust
//! - **Parsing** ([`parser`], [`types`]) — consumes a token stream
//!   produced by `muskitty-css::tokenize` and builds selector data
//!   structures (SelectorList / ComplexSelector / CompoundSelector /
//!   SubclassSelector / ...). No DOM dependency.
//! - **Specificity** ([`specificity`]) — computes the A/B/C triplet per
//!   §17.
//! - **Matching** ([`matching`]) — matches parsed selectors against an
//!   element tree via the [`matching::Element`] trait. A reference
//!   implementation for `muskitty-dom` is provided as a dev-only
//!   integration.

pub mod error;
pub mod parser;
pub mod specificity;
pub mod types;
```

### - [ ] Step 3.5: Run the failing test to verify it now passes

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: `specificity_ordering` PASSES.

### - [ ] Step 3.6: Run full test suite to verify no regression

Run: `cargo test -p muskitty-selectors`
Expected: 65 + 1 = 66 tests PASS.

### - [ ] Step 3.7: Run clippy

Run: `cargo clippy -p muskitty-selectors --all-targets -- -D warnings`
Expected: no warnings.

### - [ ] Step 3.8: Commit

```bash
git add crates/muskitty-selectors/src/specificity.rs crates/muskitty-selectors/src/lib.rs crates/muskitty-selectors/tests/specificity.rs
git commit -m "[selectors] SP-7 task 3: add specificity module skeleton with Specificity type"
```

---

## Task 4: Implement `specificity_of_compound` — base cases

**Files:**
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`

### - [ ] Step 4.1: Write the failing tests for compound-selector specificity

Append to `d:\Muskitty\crates\muskitty-selectors\tests\specificity.rs` (after `specificity_ordering`):

```rust
use muskitty_selectors::parser::parse_a_selector;
use muskitty_selectors::types::{ComplexSelectorUnit, CompoundSelector};

fn single_compound_of(list: &muskitty_selectors::types::SelectorList) -> &CompoundSelector {
    assert_eq!(list.0.len(), 1);
    let unit: &ComplexSelectorUnit = &list.0[0].units[0];
    assert!(unit.combinator.is_none());
    &unit.compound
}

fn specificity_of(input: &str) -> Specificity {
    let list = parse_a_selector(input).expect("selector should parse");
    let compound = single_compound_of(&list);
    muskitty_selectors::specificity::specificity_of_compound(compound)
}

/// §17 L4616: `*` → (0,0,0).
#[test]
fn star_zero() {
    assert_eq!(specificity_of("*"), Specificity::new(0, 0, 0));
}

/// §17 L4617: `LI` → (0,0,1).
#[test]
fn type_li() {
    assert_eq!(specificity_of("LI"), Specificity::new(0, 0, 1));
}

/// §17 L4623: `#x34y` → (1,0,0).
#[test]
fn id_selector() {
    assert_eq!(specificity_of("#x34y"), Specificity::new(1, 0, 0));
}

/// §17 L4622: `LI.red.level` → (0,2,1).
#[test]
fn type_with_two_classes() {
    assert_eq!(
        specificity_of("LI.red.level"),
        Specificity::new(0, 2, 1)
    );
}

/// §17 L4620: `H1 + *[REL=up]` — but this is a complex selector, not
/// a single compound. The single-compound variant `[REL=up]` alone
/// has specificity (0,1,0).
#[test]
fn attribute_selector_alone() {
    assert_eq!(specificity_of("[REL=up]"), Specificity::new(0, 1, 0));
}

/// §17 L4542: universal selector contributes nothing.
#[test]
fn universal_with_pseudo_class() {
    assert_eq!(
        specificity_of("*:hover"),
        Specificity::new(0, 1, 0)
    );
}

/// §14 pseudo-element: `::before` → (0,0,1).
#[test]
fn pseudo_element_alone() {
    assert_eq!(
        specificity_of("::before"),
        Specificity::new(0, 0, 1)
    );
}

/// Pseudo-class without `:is`/`:not`/`:where`/`:has`/`:nth-child`:
/// simple case `:hover` → (0,1,0).
#[test]
fn simple_pseudo_class() {
    assert_eq!(specificity_of(":hover"), Specificity::new(0, 1, 0));
}

/// Compound with type + class + attribute + pseudo-class + pseudo-element:
/// `div.foo[bar]:hover::before` → A=0, B=3 (.foo, [bar], :hover),
/// C=2 (div + ::before).
#[test]
fn compound_full_mix() {
    assert_eq!(
        specificity_of("div.foo[bar]:hover::before"),
        Specificity::new(0, 3, 2)
    );
}

/// §3 L762-787: trailing pseudo-classes on a pseudo-compound.
/// `::before:hover` → pseudo-element + pseudo-class → (0,1,1).
#[test]
fn pseudo_compound_with_trailing_pc() {
    assert_eq!(
        specificity_of("::before:hover"),
        Specificity::new(0, 1, 1)
    );
}
```

### - [ ] Step 4.2: Run tests to verify they fail

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: all newly added tests FAIL because `specificity_of_compound` returns `Specificity::default()`.

### - [ ] Step 4.3: Implement `specificity_of_compound` and `classify_subclass`

Open `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`. Replace the placeholder `_specificity_of_compound` and `_classify_subclass` (rename without leading underscore, mark `pub`) with the real implementation:

```rust
/// §17 L4539-4542: compute the specificity of a single compound
/// selector. Walks the type selector (if any), each subclass, and
/// each pseudo-compound.
pub fn specificity_of_compound(compound: &CompoundSelector) -> Specificity {
    let mut s = Specificity::default();

    // Type selector (or universal). Universal contributes 0.
    if let Some(ref ts) = compound.type_selector {
        match &ts.name {
            crate::types::TypeSelectorName::Universal => {}
            crate::types::TypeSelectorName::Name(_) => s.c += 1,
        }
    }

    // Subclass selectors: id / class / attribute / pseudo-class.
    for sub in &compound.subclasses {
        s += classify_subclass(sub);
    }

    // Pseudo-compounds: pseudo-element (+ any trailing pseudo-classes
    // that apply to it). §3 L762-787.
    for pc in &compound.pseudo_compounds {
        // Pseudo-element itself: +1 C.
        s.c += 1;
        // Trailing pseudo-classes on this pseudo-compound.
        for trailing in &pc.trailing_pseudo_classes {
            s += specificity_of_pseudo_class(trailing);
        }
    }

    s
}

/// §17 L4539-4542: classify a subclass selector into its specificity
/// contribution. ID → (1,0,0); class/attribute/pseudo-class → (0,1,0).
fn classify_subclass(s: &SubclassSelector) -> Specificity {
    match s {
        SubclassSelector::Id(_) => Specificity::new(1, 0, 0),
        SubclassSelector::Class(_) | SubclassSelector::Attribute(_) => Specificity::new(0, 1, 0),
        // Pseudo-classes need full recursive handling for
        // :is/:not/:has/:where/:nth-child. Defer to
        // `specificity_of_pseudo_class`.
        SubclassSelector::PseudoClass(pc) => specificity_of_pseudo_class(pc),
    }
}
```

Also rename `_specificity_of_pseudo_class` to `specificity_of_pseudo_class` and replace its body with the base case (default for now; special cases added in Task 5):

```rust
/// §17 L4550-4566: compute the specificity contribution of a
/// pseudo-class. The default case (a plain pseudo-class like `:hover`)
/// contributes (0,1,0). The special cases for `:is`/`:not`/`:has`/
/// `:where`/`:nth-child`/`:nth-last-child` are handled in
/// [`special_pseudo_class_specificity`] (Task 5).
fn specificity_of_pseudo_class(pc: &PseudoClass) -> Specificity {
    // §17 L4540: a pseudo-class counts as one B.
    let base = Specificity::new(0, 1, 0);
    match special_pseudo_class_specificity(pc) {
        Some(special) => special,
        None => base,
    }
}

/// §17 L4550-4566: returns `Some(s)` for the special pseudo-classes
/// (`:is`, `:not`, `:has`, `:where`, `:nth-child`, `:nth-last-child`)
/// whose specificity is replaced/extended per the spec. Returns `None`
/// for ordinary pseudo-classes (which use the default (0,1,0)).
///
/// Currently returns `None` for all names — Task 5 fills this in.
fn special_pseudo_class_specificity(_pc: &PseudoClass) -> Option<Specificity> {
    None
}
```

### - [ ] Step 4.4: Add the `pub` visibility for `specificity_of_compound`

Ensure `specificity_of_compound` is exported as `pub` (it was made `pub` in Step 4.3 above — verify by reading the file back).

### - [ ] Step 4.5: Run the failing tests to verify they pass

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: 10 compound-level tests PASS (the 3 special pseudo-class tests are not yet present).

### - [ ] Step 4.6: Run full test suite

Run: `cargo test -p muskitty-selectors`
Expected: 66 + 10 = 76 tests PASS.

### - [ ] Step 4.7: Run clippy

Run: `cargo clippy -p muskitty-selectors --all-targets -- -D warnings`
Expected: no warnings.

### - [ ] Step 4.8: Commit

```bash
git add crates/muskitty-selectors/src/specificity.rs crates/muskitty-selectors/tests/specificity.rs
git commit -m "[selectors] SP-7 task 4: implement specificity_of_compound (base cases)"
```

---

## Task 5: Implement special pseudo-class specificity rules

**Files:**
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`

### - [ ] Step 5.1: Write the failing tests for special pseudo-classes

Append to `d:\Muskitty\crates\muskitty-selectors\tests\specificity.rs`:

```rust
/// §17 L4573-4577: `:is(em, #foo)` → (1,0,0). The `:is()` argument
/// list has `em` = (0,0,1) and `#foo` = (1,0,0); max is (1,0,0).
#[test]
fn is_takes_max_of_args() {
    assert_eq!(
        specificity_of(":is(em, #foo)"),
        Specificity::new(1, 0, 0)
    );
}

/// §17 L4590-4593: `:not(em, strong#foo)` → (1,0,1). Same max rule.
#[test]
fn not_takes_max_of_args() {
    assert_eq!(
        specificity_of(":not(em, strong#foo)"),
        Specificity::new(1, 0, 1)
    );
}

/// §17 L4579-4582: `.qux:where(em, #foo#bar#baz)` → (0,1,0).
/// `:where()` always contributes zero specificity regardless of args.
#[test]
fn where_zero_specificity() {
    assert_eq!(
        specificity_of(".qux:where(em, #foo#bar#baz)"),
        Specificity::new(0, 1, 0)
    );
}

/// §17 L4584-4588: `:nth-child(even of li, .item)` → (0,2,0).
/// The pseudo-class contributes (0,1,0), plus max of `li` (0,0,1) and
/// `.item` (0,1,0) which is (0,1,0). Total = (0,2,0).
#[test]
fn nth_child_of_s_adds_max() {
    assert_eq!(
        specificity_of(":nth-child(even of li, .item)"),
        Specificity::new(0, 2, 0)
    );
}

/// §17 L4560-4564: `:nth-child(even)` without `of S` → (0,1,0) (just
/// the pseudo-class).
#[test]
fn nth_child_without_of_s() {
    assert_eq!(
        specificity_of(":nth-child(even)"),
        Specificity::new(0, 1, 0)
    );
}

/// §17 L4555-4558: `:has(.a)` argument is a relative selector. The
/// implicit `:scope` pseudo-class is part of the complex selector,
/// contributing (0,1,0); `.a` contributes (0,1,0). Max over the list
/// is (0,2,0). `:has()` itself is replaced by this max.
#[test]
fn has_takes_max_of_relative_args() {
    assert_eq!(
        specificity_of(":has(.a)"),
        Specificity::new(0, 2, 0)
    );
}

/// §17 L4624-4625: `#s12:not(FOO)` → (1,0,1). The `#s12` is (1,0,0);
/// `:not(FOO)` is replaced by max of `FOO` = (0,0,1). Total = (1,0,1).
#[test]
fn compound_id_with_not_foo() {
    assert_eq!(
        specificity_of("#s12:not(FOO)"),
        Specificity::new(1, 0, 1)
    );
}
```

### - [ ] Step 5.2: Run tests to verify they fail

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: 7 new tests FAIL (`is_takes_max_of_args`, `not_takes_max_of_args`, `where_zero_specificity`, `nth_child_of_s_adds_max`, `has_takes_max_of_relative_args`, `compound_id_with_not_foo`).

### - [ ] Step 5.3: Implement `special_pseudo_class_specificity`

Open `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`. Replace the placeholder `special_pseudo_class_specificity` function (returns `None` for all) with the real implementation:

```rust
/// §17 L4550-4566: returns `Some(s)` for the special pseudo-classes
/// (`:is`, `:not`, `:has`, `:where`, `:nth-child`, `:nth-last-child`)
/// whose specificity is replaced/extended per the spec. Returns `None`
/// for ordinary pseudo-classes (which use the default (0,1,0)).
fn special_pseudo_class_specificity(pc: &PseudoClass) -> Option<Specificity> {
    use crate::types::PseudoClassArgument;
    // §17 L4555-4558: `:is`/`:not`/`:has` — replaced by max of args.
    if matches!(pc.name.as_str(), "is" | "not" | "has") {
        return pc.argument.as_ref().and_then(|arg| match arg {
            PseudoClassArgument::SelectorList(list) => {
                Some(Specificity::max_of_list(list))
            }
            _ => None,
        });
    }
    // §17 L4566: `:where` — replaced by zero.
    if pc.name == "where" {
        return Some(Specificity::default());
    }
    // §17 L4560-4564: `:nth-child` / `:nth-last-child` — pseudo-class
    // base (1×B) plus max of `of S` (if present).
    if matches!(pc.name.as_str(), "nth-child" | "nth-last-child") {
        return pc.argument.as_ref().and_then(|arg| match arg {
            PseudoClassArgument::AnPlusB(_, Some(of_s)) => {
                // Base pseudo-class + max of S.
                let base = Specificity::new(0, 1, 0);
                let max_of_s = Specificity::max_of_list(of_s);
                Some(base + max_of_s)
            }
            // Without `of S`: just the base (0,1,0). But this case is
            // already handled by the default path in
            // `specificity_of_pseudo_class`. Returning `None` here
            // lets the default path apply.
            PseudoClassArgument::AnPlusB(_, None) => None,
            _ => None,
        });
    }
    None
}
```

### - [ ] Step 5.4: Run the failing tests to verify they pass

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: all 7 new tests PASS (in addition to all previously-passing tests).

### - [ ] Step 5.5: Run full test suite

Run: `cargo test -p muskitty-selectors`
Expected: 76 + 7 = 83 tests PASS.

### - [ ] Step 5.6: Run clippy

Run: `cargo clippy -p muskitty-selectors --all-targets -- -D warnings`
Expected: no warnings.

### - [ ] Step 5.7: Commit

```bash
git add crates/muskitty-selectors/src/specificity.rs crates/muskitty-selectors/tests/specificity.rs
git commit -m "[selectors] SP-7 task 5: implement :is/:not/:has/:where/:nth-child special specificity"
```

---

## Task 6: Implement `specificity_of_complex` (sum across compound units)

**Files:**
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`

### - [ ] Step 6.1: Write the failing tests for complex-selector specificity

Append to `d:\Muskitty\crates\muskitty-selectors\tests\specificity.rs`:

```rust
use muskitty_selectors::types::SelectorList;

fn complex_specificity_of(input: &str) -> Specificity {
    let list: SelectorList = parse_a_selector(input).expect("selector should parse");
    assert_eq!(list.0.len(), 1, "expected 1 complex selector in list");
    muskitty_selectors::specificity::specificity_of_complex(&list.0[0])
}

/// §17 L4618: `UL LI` → (0,0,2).
#[test]
fn two_type_descendant() {
    assert_eq!(
        complex_specificity_of("UL LI"),
        Specificity::new(0, 0, 2)
    );
}

/// §17 L4619: `UL OL+LI` → (0,0,3). Three type selectors; `+` is
/// just a combinator and contributes nothing.
#[test]
fn three_type_with_combinator() {
    assert_eq!(
        complex_specificity_of("UL OL+LI"),
        Specificity::new(0, 0, 3)
    );
}

/// §17 L4620: `H1 + *[REL=up]` → (0,1,1). `H1` is (0,0,1); `*` is 0;
/// `[REL=up]` is (0,1,0). Sum = (0,1,1).
#[test]
fn h1_plus_attr() {
    assert_eq!(
        complex_specificity_of("H1 + *[REL=up]"),
        Specificity::new(0, 1, 1)
    );
}

/// §17 L4621: `UL OL LI.red` → (0,1,3). Three type selectors and one
/// class.
#[test]
fn three_type_one_class_descendant() {
    assert_eq!(
        complex_specificity_of("UL OL LI.red"),
        Specificity::new(0, 1, 3)
    );
}

/// §17 L4626: `.foo :is(.bar, #baz)` → (1,1,0). `.foo` = (0,1,0);
/// `:is(.bar, #baz)` = max of (0,1,0) and (1,0,0) = (1,0,0).
/// Sum = (1,1,0).
#[test]
fn foo_desc_is_bar_baz() {
    assert_eq!(
        complex_specificity_of(".foo :is(.bar, #baz)"),
        Specificity::new(1, 1, 0)
    );
}

/// Three combinators chained: `a > b + c ~ d` → all type selectors, no
/// other components → (0,0,4).
#[test]
fn mixed_combinators_sum_types() {
    assert_eq!(
        complex_specificity_of("a > b + c ~ d"),
        Specificity::new(0, 0, 4)
    );
}

/// Complex selector with combinator + pseudo-class:
/// `div > .foo:hover` → div (0,0,1) + .foo:hover (0,2,0) = (0,2,1).
#[test]
fn child_combinator_with_pseudo_class() {
    assert_eq!(
        complex_specificity_of("div > .foo:hover"),
        Specificity::new(0, 2, 1)
    );
}
```

### - [ ] Step 6.2: Run tests to verify they fail

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: all newly-added tests FAIL (since `specificity_of_complex` returns `Specificity::default()`).

### - [ ] Step 6.3: Implement `specificity_of_complex`

Open `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`. Replace the placeholder `specificity_of_complex` with:

```rust
/// §17 L4536-4548: compute the specificity of a single complex
/// selector. Sums the specificities of all compound units; the
/// combinator on each unit contributes nothing.
pub fn specificity_of_complex(cs: &ComplexSelector) -> Specificity {
    let mut s = Specificity::default();
    for unit in &cs.units {
        s += specificity_of_compound(&unit.compound);
    }
    s
}
```

### - [ ] Step 6.4: Run tests to verify they pass

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: all 7 new tests PASS.

### - [ ] Step 6.5: Run full test suite

Run: `cargo test -p muskitty-selectors`
Expected: 83 + 7 = 90 tests PASS.

### - [ ] Step 6.6: Run clippy

Run: `cargo clippy -p muskitty-selectors --all-targets -- -D warnings`
Expected: no warnings.

### - [ ] Step 6.7: Commit

```bash
git add crates/muskitty-selectors/src/specificity.rs crates/muskitty-selectors/tests/specificity.rs
git commit -m "[selectors] SP-7 task 6: implement specificity_of_complex (sum across compounds)"
```

---

## Task 7: Add public API methods and re-export

**Files:**
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\specificity.rs`
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\types.rs` (add convenience methods)
- Modify: `d:\Muskitty\crates\muskitty-selectors\src\lib.rs`

### - [ ] Step 7.1: Write the failing tests for public API methods

Append to `d:\Muskitty\crates\muskitty-selectors\tests\specificity.rs`:

```rust
use muskitty_selectors::types::{ComplexSelector, SelectorList};

/// §17 L4536: top-level `ComplexSelector::specificity()` method.
#[test]
fn complex_selector_method() {
    let list: SelectorList = parse_a_selector("UL OL LI.red").expect("parses");
    let cs: &ComplexSelector = &list.0[0];
    assert_eq!(cs.specificity(), Specificity::new(0, 1, 3));
}

/// §17 L4547-4548: `SelectorList::specificity_max()` returns the max
/// specificity over all complex selectors in the list.
#[test]
fn selector_list_max_method() {
    // List with 3 selectors of increasing specificity.
    let list: SelectorList = parse_a_selector("div, .a, #id").expect("parses");
    assert_eq!(list.0.len(), 3);
    // div = (0,0,1); .a = (0,1,0); #id = (1,0,0). Max = (1,0,0).
    assert_eq!(list.specificity_max(), Specificity::new(1, 0, 0));
}

/// §17 L4547-4548: empty list → (0,0,0).
#[test]
fn empty_list_max() {
    let empty = SelectorList::default();
    assert_eq!(empty.specificity_max(), Specificity::default());
}

/// `Specificity` is re-exported at the crate root for ergonomics.
#[test]
fn specificity_re_exported_at_root() {
    let _: muskitty_selectors::Specificity = muskitty_selectors::Specificity::new(1, 2, 3);
}
```

### - [ ] Step 7.2: Run tests to verify they fail

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: 4 new tests FAIL (methods `ComplexSelector::specificity`, `SelectorList::specificity_max`, and root-level `Specificity` re-export do not exist).

### - [ ] Step 7.3: Add convenience methods to `ComplexSelector` and `SelectorList`

Open `d:\Muskitty\crates\muskitty-selectors\src\types.rs`. Add a new `impl` block for `ComplexSelector` (below its struct definition, around line 47) and `SelectorList` (below line 29). Also add `use` import for `Specificity`:

At top of `types.rs`, after `use muskitty_css::tokenizer::Token;`, add:

```rust
// Forward declaration trick: the actual impl lives in specificity.rs
// and is gated on `crate::specificity`. We avoid a circular import
// by using the pub free function instead of calling into the
// `specificity` module from `types`.
```

Add `impl` blocks (no fields touched, only methods):

```rust
impl ComplexSelector {
    /// §17 L4536-4548: compute the specificity of this complex
    /// selector. Delegates to [`crate::specificity::specificity_of_complex`].
    pub fn specificity(&self) -> crate::specificity::Specificity {
        crate::specificity::specificity_of_complex(self)
    }
}

impl SelectorList {
    /// §17 L4547-4548: max specificity over all complex selectors in
    /// the list. Returns `(0,0,0)` for an empty list.
    pub fn specificity_max(&self) -> crate::specificity::Specificity {
        crate::specificity::Specificity::max_of_list(self)
    }
}
```

### - [ ] Step 7.4: Re-export `Specificity` at crate root

Open `d:\Muskitty\crates\muskitty-selectors\src\lib.rs`. Add the re-export after `pub mod specificity;`:

```rust
pub mod error;
pub mod parser;
pub mod specificity;
pub mod types;

/// Convenience re-export of the [`Specificity`](specificity::Specificity)
/// type for ergonomic access from downstream crates.
pub use specificity::Specificity;
```

### - [ ] Step 7.5: Run tests to verify they pass

Run: `cargo test -p muskitty-selectors --test specificity`
Expected: all 4 new tests PASS.

### - [ ] Step 7.6: Run full test suite

Run: `cargo test -p muskitty-selectors`
Expected: 90 + 4 = 94 tests PASS.

### - [ ] Step 7.7: Run clippy

Run: `cargo clippy -p muskitty-selectors --all-targets -- -D warnings`
Expected: no warnings.

### - [ ] Step 7.8: Commit

```bash
git add crates/muskitty-selectors/src/specificity.rs crates/muskitty-selectors/src/types.rs crates/muskitty-selectors/src/lib.rs crates/muskitty-selectors/tests/specificity.rs
git commit -m "[selectors] SP-7 task 7: add public API methods + re-export Specificity at crate root"
```

---

## Task 8: Final quality gate + push

**Files:** none (verification only).

### - [ ] Step 8.1: Run fmt check

Run: `cargo fmt -p muskitty-selectors -- --check`
Expected: no diff output, exit 0. If a diff appears, run `cargo fmt -p muskitty-selectors` to format, then re-run `-- --check`.

### - [ ] Step 8.2: Run check

Run: `cargo check -p muskitty-selectors`
Expected: `Finished` with no errors.

### - [ ] Step 8.3: Run full test suite

Run: `cargo test -p muskitty-selectors`
Expected: 94 tests PASS (61 pre-SP-7 + 4 parser_nth_of + 1 specificity_ordering + 10 compound + 7 special + 7 complex + 4 API).

### - [ ] Step 8.4: Run clippy with `-D warnings`

Run: `cargo clippy -p muskitty-selectors --all-targets -- -D warnings`
Expected: no warnings.

### - [ ] Step 8.5: Verify the commit list for SP-7

Run: `git log --oneline -n 8`
Expected: 7 commits for SP-7 tasks 1-7 (plus the prior SP-6 commit at the bottom).

### - [ ] Step 8.6: Push to origin/main

Run: `git push origin main`
Expected: push succeeds, no rejection.

### - [ ] Step 8.7: STOP — do not auto-enter SP-8 plan mode

After push succeeds, **stop**. Per the auto-transition rule at the top of this plan, the next step is the user-directed split of `muskitty-selectors` into an independent git repo (mirroring the `muskitty-css-parser` extraction pattern). SP-8 (§18 matching engine) is deferred to the independent repo.

---

## Self-Review

### Spec coverage

- ✅ §17 L4536-4548: base calculation (A/B/C counts) → Task 4
- ✅ §17 L4555-4558: `:is`/`:not`/`:has` max-of-args → Task 5
- ✅ §17 L4560-4564: `:nth-child`/`:nth-last-child` plus-max-of-S → Task 5 (depends on Task 2's `of S` parsing)
- ✅ §17 L4566: `:where` zero → Task 5
- ✅ §17 L4547-4548: SelectorList max → Tasks 3 + 7
- ✅ §17 L4598-4605: lexicographic comparison → Task 3
- ✅ §17 L4613-4628 examples: every worked example has a dedicated test (`*`, `LI`, `UL LI`, `UL OL+LI`, `H1 + *[REL=up]`, `UL OL LI.red`, `LI.red.level`, `#x34y`, `#s12:not(FOO)`, `.foo :is(.bar, #baz)`)
- ✅ §17 L4570-4593 additional examples: `:is(em, #foo)`, `.qux:where(em, #foo#bar#baz)`, `:nth-child(even of li, .item)`, `:not(em, strong#foo)`

### Placeholder scan

- No "TBD", "TODO", "implement later" anywhere.
- Every step has actual code or exact commands.
- Task 3 Step 3.3 includes complete `Specificity` struct + `Ord`/`Add` impls.
- Task 4 Step 4.3 includes complete `specificity_of_compound` + `classify_subclass`.
- Task 5 Step 5.3 includes complete `special_pseudo_class_specificity`.
- Task 6 Step 6.3 includes complete `specificity_of_complex`.
- Task 7 Steps 7.3-7.4 include complete `impl` blocks and re-export.

### Type consistency

- `Specificity::new(a, b, c)` constructor used consistently across all tests and impls.
- `Specificity::default()` returns `(0,0,0)` consistently.
- `specificity_of_compound(&CompoundSelector) -> Specificity` signature consistent in Task 4 (impl) and tests (uses).
- `specificity_of_complex(&ComplexSelector) -> Specificity` signature consistent in Task 6 (impl) and tests (uses).
- `PseudoClassArgument::AnPlusB(AnPlusB, Option<SelectorList>)` shape consistent from Task 1 (definition), Task 2 (construction in parser), Task 5 (matching in `special_pseudo_class_specificity`).
- `SelectorList::specificity_max()` method added in Task 7 matches the test calls.
- `ComplexSelector::specificity()` method added in Task 7 matches the test calls.
- `muskitty_selectors::Specificity` re-export added in Task 7 matches the test `specificity_re_exported_at_root`.

### Notes on interpretation choices

1. **`:has(.a)` → (0,2,0)**: Per the literal spec reading, the `:has()` argument is a *relative selector list*. Each relative selector is parsed with an implicit `:scope` prepended (per parser/relative.rs). The complex selector `:scope .a` has specificity `(0,1,0) + (0,1,0) = (0,2,0)`. The spec says "the most specific complex selector in its selector list argument" — we interpret the argument's complex selectors (with implicit `:scope` included) as the input to the max. This matches the existing parser behavior. If WPT tests or future spec clarifications contradict this, revisit.

2. **`:nth-of-type` / `:nth-last-of-type` reject `of S`**: Per §13.6 L4212-4238 and §13.7 L4241-4262, these pseudo-classes do NOT accept the `of S` syntax. The parser is extended to reject this combination explicitly in Task 2.

3. **`Specificity` uses `u32` components**: The spec mentions "Due to storage limitations, implementations may have limitations on the size of A, B, or C. If so, values higher than the limit must be clamped to that limit, and not overflow." (L4607-4610). `u32` (max ~4.3 billion) is effectively unlimited for any realistic selector; no clamping logic is added in this iteration.

4. **Universal selector contributes zero**: §17 L4542 explicitly. Verified in `specificity_of_compound` Step 4.3 via the `TypeSelectorName::Universal` no-op branch.

---

## Execution Handoff

Plan complete. Two execution options:

1. **Inline Execution (recommended for SP-7)** — Execute tasks in this session, batching quality gates. SP-7 is small enough (8 tasks, mostly TDD with one source file per task) that inline execution is faster than subagent dispatch overhead.

2. **Subagent-Driven** — Dispatch a fresh subagent per task with two-stage review. Heavier process but cleaner separation.

Recommended: Inline, matching SP-6's flow.

**Which approach?**
