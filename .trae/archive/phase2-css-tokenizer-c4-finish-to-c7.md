# Phase 2 — muskitty-css Tokenizer C-4 收尾 → C-7

> 规范源：`D:\CSSWG\markdown\css-syntax-3\Overview.md`（CSS Syntax Module Level 3）
> 一切以标准为准。每个 commit message 必须引用 §章节号与 L 行号。
> 上一份计划：`.trae/documents/phase2-css-tokenizer-c2-to-c7.md`（C-2/C-3 已完成，可作 C-5/C-6/C-7 细节参考）。

## 摘要

C-2（commit `7a4a472`）与 C-3（commit `ec11beb`）已完成并提交。C-4 的实现代码已写入 `impls.rs`（`consume_a_numeric_token` / `consume_a_number` / `starts_with_number` + 11 个测试），`cargo check` 通过，但**尚未运行测试、尚未提交**。本计划覆盖 C-4 收尾验证 + 提交，以及 C-5 / C-6 / C-7 三个剩余批次。

## 当前状态分析

### Git 状态
- 分支 `main`，领先 `origin/main` 1 个提交（C-3 `ec11beb`）。
- 工作区有未暂存修改：`crates/muskitty-css/src/tokenizer/impls.rs`（C-4 代码）。
- 未跟踪文件：`.trae/documents/phase2-css-tokenizer-c2-to-c7.md`（上一份计划，C-2 时产生）。

### 已实现并提交（C-0 ~ C-3）
- §4.3.1 `consume_a_token` 主分发（whitespace / 引号 / `#` / 括号 / `+`/`-`/`.` / `<` / `@` / `[`/`]` / `{`/`}` / `\` / digit / `u`/`U` / ident-start / `:`/`;`/`,` / EOF / 其它 → Delim），含注释循环。
- §4.3.2 `consume_comments_body`。
- §4.3.4 `consume_an_ident_like_token`（**无 url( 特例**）、`consume_an_at_keyword_token`、`consume_a_hash_token`（含 Delim 回退）。
- §4.3.5 `consume_a_string_token`（严格按 L1081-1128）。
- §4.3.7 `consume_an_escaped_code_point`（EOF → U+FFFD）。
- §4.3.8 `is_valid_escape_next` / `is_valid_escape_at`。
- §4.3.9 `would_start_ident_sequence_at`。
- §4.3.12 `consume_an_ident_sequence`。
- §5.3 `preprocess_input`。

### C-4 已写代码（待验证 + 提交）
- `impls.rs` L27：`use super::types::{HashType, Numeric, State, Token};`（已加 `Numeric`）。
- `impls.rs` L348-372：`consume_a_numeric_token`（§4.3.3 L1011-1042）— Dimension / Percentage / Number 三分支。
- `impls.rs` L374-458：`consume_a_number`（§4.3.13 L1415-1483）— sign + integer + fraction + exponent，返回 `(f64, bool)`。
- `impls.rs` L690-701：`starts_with_number`（§4.3.10 L1307-1352）— 替换原 stub。
- `impls.rs` L1246-1335：11 个测试（`number_integer` / `number_decimal` / `number_signed` / `number_exponent` / `number_decimal_exponent` / `percentage_token` / `dimension_px` / `dimension_em` / `dimension_signed` / `plus_sign_number` / `dot_starts_number`）。

### 仍为 stub / 未实现
- §4.3.4 url( 特例（`consume_an_ident_like_token` L473-482 统一产 Function）。
- §4.3.6 `consume_a_url_token`：无。
- §4.3.15 `consume_the_remnants_of_a_bad_url`：无。
- §4.3.11 `would_start_unicode_range_at`：无。
- §4.3.14 `consume_a_unicode_range_token`：无。
- §4.3.1 `unicode_ranges_allowed` 标志：无（`consume_u_or_unicode_range` L511-516 当前无条件走 ident-like）。
- `is_whitespace` / `is_non_printable` helper：无（C-5 新增）。

## 提议变更

### 阶段 1：C-4 收尾 — 验证 + 提交

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`（代码已写，仅验证）

1. **运行测试**：
   ```powershell
   cargo test -p muskitty-css
   ```
   预期 55/55 green（C-3 后 44 + C-4 新增 11）。

2. **若出现失败**：逐个排查。重点关注：
   - `consume_a_number` 的 `number_part.parse::<f64>()`：当 `number_part` 为空（如输入 `.5`，sign 无，integer 无，直接进 fraction）时，`"".parse::<f64>()` 返回 `Err`，`unwrap_or(0.0)` 给 0.0，然后 fraction 部分重新解析。**潜在 bug**：当前实现把 sign/integer/fraction 全拼进 `number_part` 再 parse，对 `.5` 会得到 `number_part = ".5"`，`".5".parse::<f64>()` = 0.5，正确。对 `-.5` 得 `"-0.5"`？不，integer 部分无 digit，`number_part = "-"`，进 fraction 后 `number_part = "-.5"`，parse = -0.5，正确。需验证。
   - `1e3`：`number_part = "1"`，exponent 触发（`peek(0)=='e'`，`peek(1)=='3'` 是 digit → valid），`exponent_part = "3"`，`value = 1.0 * 10^3 = 1000.0`，`is_integer = false`。正确。
   - `1.5e2`：`number_part = "1.5"`，exponent `exponent_part = "2"`，`value = 1.5 * 100 = 150.0`。正确。

3. **Commit**（代码已就绪，测试 green 后直接提交）：
   ```
   [css-tokenizer] C-4: 4.3.3/4.3.10/4.3.13 number/percentage/dimension

   - §4.3.3 consume_a_numeric_token (L1011-1042): number → dimension
     (if would-start-ident-sequence) / percentage (if %) / number.
   - §4.3.13 consume_a_number (L1415-1483): sign + integer + fraction
     + exponent; is_integer flag per type "integer"/"number".
   - §4.3.10 starts_with_number (L1307-1352): +/- + digit|.digit;
     . + digit; digit.
   - 11 new tests covering integer/decimal/signed/exponent/percentage/
     dimension variants.
   ```

### 阶段 2：C-5 — §4.3.4 url( 特例 + §4.3.6 Url + §4.3.15 BadUrl

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`

#### 2.1 新增 helper（§4.2 定义）

在 free functions 区（L704+）新增：
```rust
/// §4.2 whitespace: U+0009 TAB, U+000A LF, U+000C FF, U+000D CR, U+0020 SPACE.
fn is_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{000C}' | '\r' | ' ')
}

/// §4.2 non-printable code point: U+0000-U+0008, U+000B, U+000E-U+001F,
/// U+007F-U+009F.
fn is_non_printable(c: char) -> bool {
    matches!(c, '\u{0000}'..='\u{0008}' | '\u{000B}' | '\u{000E}'..='\u{001F}' | '\u{007F}'..='\u{009F}')
}
```

#### 2.2 改 `consume_an_ident_like_token`（L473-482）按 §4.3.4 L1051-1078

```rust
fn consume_an_ident_like_token(&mut self) -> Token {
    let name = self.consume_an_ident_sequence();
    // §4.3.4 L1053-1066: url( 特例
    if name.eq_ignore_ascii_case("url") && self.peek(0) == Some('(') {
        self.consume(); // consume `(`
        // §4.3.4 L1056-1057: while next two are whitespace, consume one
        while self.peek(0).map_or(false, is_whitespace)
            && self.peek(1).map_or(false, is_whitespace)
        {
            self.consume();
        }
        // §4.3.4 L1058-1063: 若 next 1-2 是 " / ' / ws+" / ws+' → Function
        let p0 = self.peek(0);
        let p1 = self.peek(1);
        let is_quote_case = matches!(p0, Some('"') | Some('\''))
            || (p0.map_or(false, is_whitespace) && matches!(p1, Some('"') | Some('\'')));
        if is_quote_case {
            return Token::Function(name);
        }
        // §4.3.4 L1064-1066: 否则 consume a url token
        return self.consume_a_url_token();
    }
    // §4.3.4 L1068-1073: 普通 function
    if self.peek(0) == Some('(') {
        self.consume();
        return Token::Function(name);
    }
    // §4.3.4 L1075-1078: ident
    Token::Ident(name)
}
```

> **L1056-1057 语义**："While the next two input code points are whitespace, consume the next input code point." 即当 peek(0) 与 peek(1) 同时是 whitespace 时，consume 一个。这会把 `url(  \t foo` 的前导空白收敛到剩 1 个，之后 §4.3.6 L1151 再 consume 剩余。实现正确。

#### 2.3 实现 `consume_a_url_token` 按 §4.3.6 L1132-1203

```rust
/// §4.3.6 (L1132-1203) Consume a url token.
///
/// Precondition: `url(` 及后续前导空白已由调用方处理（§4.3.4 L1053-1066
/// 的 whitespace 收敛 + 本函数开头的 L1151 whitespace consume）。
/// 返回 `url-token` 或 `bad-url-token`。
fn consume_a_url_token(&mut self) -> Token {
    let mut value = String::new();
    // §4.3.6 L1151: consume as much whitespace as possible
    while self.peek(0).map_or(false, is_whitespace) {
        self.consume();
    }
    loop {
        match self.consume() {
            // §4.3.6 L1157-1159: `)` → return url-token
            Some(')') => return Token::Url(value),
            // §4.3.6 L1161-1164: EOF → parse error, return url-token
            None => return Token::Url(value),
            // §4.3.6 L1166-1175: whitespace → consume trailing ws, then check
            Some(c) if is_whitespace(c) => {
                while self.peek(0).map_or(false, is_whitespace) {
                    self.consume();
                }
                match self.peek(0) {
                    Some(')') => {
                        self.consume();
                        return Token::Url(value);
                    }
                    None => return Token::Url(value), // parse error
                    _ => {
                        self.consume_the_remnants_of_a_bad_url();
                        return Token::BadUrl;
                    }
                }
            }
            // §4.3.6 L1177-1185: " / ' / ( / non-printable → bad url
            Some(c) if c == '"' || c == '\'' || c == '(' || is_non_printable(c) => {
                self.consume_the_remnants_of_a_bad_url();
                return Token::BadUrl;
            }
            // §4.3.6 L1187-1197: `\`
            Some('\\') => {
                if self.is_valid_escape_next() {
                    let escaped = self.consume_an_escaped_code_point();
                    value.push(escaped);
                } else {
                    self.consume_the_remnants_of_a_bad_url();
                    return Token::BadUrl;
                }
            }
            // §4.3.6 L1199-1202: anything else → append
            Some(c) => value.push(c),
        }
    }
}
```

> **注意**：`is_valid_escape_next()` 检查 `peek(0)`（即 `\` 之后第一个）。当前 `\` 已被 `consume()` 吞掉，`is_valid_escape_next` 内部 `peek(0)` 指向 `\` 之后，正确。

#### 2.4 实现 `consume_the_remnants_of_a_bad_url` 按 §4.3.15 L1551-1577

规范字面是 "Repeatedly consume the next input code point"，但 L1568 "the input stream starts with a valid escape" 指剩余流以 valid escape 开头。采用 **consume 后检查** 模式（更贴近规范字面），关键点：consume 到的 `)` 或 EOF 退出；其余若剩余流以 `\` 开头且 valid escape，则 consume escaped（允许 `\)` 不退出）。

```rust
/// §4.3.15 (L1551-1577) Consume the remnants of a bad url.
///
/// 消费到 `)` 或 EOF 为止。遇到 valid escape（`\` + 非换行非 EOF）时
/// consume escaped code point（使 `\)` 不触发退出）。
fn consume_the_remnants_of_a_bad_url(&mut self) {
    loop {
        match self.consume() {
            // §4.3.15 L1563-1566: `)` or EOF → return
            Some(')') | None => return,
            // §4.3.15 L1568-1572: valid escape → consume escaped code point
            Some('\\') if self.is_valid_escape_next() => {
                let _ = self.consume_an_escaped_code_point();
            }
            // §4.3.15 L1574-1576: anything else → do nothing
            Some(_) => {}
        }
    }
}
```

> **关键修正**：规范 L1568「the input stream starts with a valid escape」指**剩余流**（即 `\` 之后）以 valid escape 开头。consume 到 `\` 后，`is_valid_escape_next()` 检查 `peek(0)`（`\` 之后第一个）是否非换行非 EOF，正好对应。此实现比上一份计划的 peek-then-consume 更简洁且语义等价。

#### 2.5 新增测试（10 个）

- `url_unquoted_simple`：`url(foo)` → `[Url("foo")]`
- `url_unquoted_with_spaces`：`url( foo )` → `[Url("foo")]`
- `url_empty`：`url()` → `[Url("")]`
- `url_eof_unterminated`：`url(foo` → `[Url("foo")]`（EOF parse error，仍 Url）
- `url_quoted_is_function`：`url("foo")` → `[Function("url")]`（L1058-1063 quote 触发）
- `url_single_quoted_is_function`：`url('foo')` → `[Function("url")]`
- `url_ws_then_quote_is_function`：`url( "foo")` → `[Function("url")]`
- `url_with_escape`：`url(foo\29 bar)` → `[Url("foo)bar")]`（`\29 ` = `)`，trailing space 被 §4.3.7 吞）
- `url_bad_paren_in_unquoted`：`url(foo(bar)` → `[BadUrl]`
- `url_bad_quote_in_unquoted`：`url(foo"bar)` → `[BadUrl]`

> **`url_with_escape` 验证**：`\29` 后跟 ` ` → §4.3.7 hex escape 消耗 1 空白，得 `)`。然后 `bar` 是 anything else，append。最终 `value = "foo)bar"`。

#### 2.6 验证 + 提交

```powershell
cargo test -p muskitty-css
cargo check -p muskitty-css
```
预期 65/65 green（55 + 10）。

**Commit**：
```
[css-tokenizer] C-5: 4.3.4/4.3.6/4.3.15 url + bad-url + url( special case

- §4.3.4 consume_an_ident_like_token (L1051-1078): url( special case —
  name == "url" + `(` → consume whitespace pairs, check quote/ws+quote
  → Function, else consume_a_url_token.
- §4.3.6 consume_a_url_token (L1132-1203): `)`/EOF → Url; whitespace →
  trailing ws + `)`/EOF/bad; " ' ( non-printable → BadUrl; `\` → escape
  or BadUrl; else append.
- §4.3.15 consume_the_remnants_of_a_bad_url (L1551-1577): consume to
  `)`/EOF; valid escape consumes escaped (allows `\)`).
- §4.2 helpers: is_whitespace, is_non_printable.
- 10 new tests.
```

### 阶段 3：C-6 — §4.3.1 unicode_ranges_allowed + §4.3.11 + §4.3.14

**文件**：
- `d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`
- `d:\Muskitty\crates\muskitty-css\src\tokenizer\trait_def.rs`

#### 3.1 `trait_def.rs`：加 trait 方法

```rust
/// §4.3.1 L782-783: 设置 unicode_ranges_allowed 标志。
/// 默认 false；仅在 @font-face unicode-range 描述符解析时设 true
/// （§4.3.14 L1500-1506 备注：此 token 正常情况下不由顶层 tokenizer 产生）。
fn set_unicode_ranges_allowed(&mut self, allowed: bool);
```

#### 3.2 `impls.rs`：struct 加字段 + impl

`CssTokenizer` 加字段：
```rust
/// §4.3.1 L782-783: unicode_ranges_allowed 标志，默认 false。
unicode_ranges_allowed: bool,
```
`new` 初始化 `unicode_ranges_allowed: false`。

`impl Tokenizer for CssTokenizer` 加：
```rust
fn set_unicode_ranges_allowed(&mut self, allowed: bool) {
    self.unicode_ranges_allowed = allowed;
}
```

#### 3.3 `would_start_unicode_range_at`（§4.3.11 L1356-1378）

```rust
/// §4.3.11 (L1356-1378) Check if three code points would start a
/// unicode-range, examining code points starting at `offset`.
///
/// Per §4.3.11: first is U/u, second is `+`, third is `?` or hex digit.
fn would_start_unicode_range_at(&self, offset: usize) -> bool {
    let first = match self.peek(offset) {
        Some(c) => c,
        None => return false,
    };
    if !matches!(first, 'U' | 'u') {
        return false;
    }
    if self.peek(offset + 1) != Some('+') {
        return false;
    }
    match self.peek(offset + 2) {
        Some('?') => true,
        Some(c) if is_hex_digit(c) => true,
        _ => false,
    }
}
```

#### 3.4 改 `consume_u_or_unicode_range`（L511-516）按 §4.3.1 L960-972

```rust
/// §4.3.1 (L960-972) U/u 分支：若 unicode_ranges_allowed 且 would-start
/// unicode-range，consume a unicode-range token；否则 ident-like。
///
/// Precondition: `u`/`U` 已被 consume，pos 指向其后。本方法先 reconsume
/// 回 `u`，再判断。
fn consume_u_or_unicode_range(&mut self) -> Token {
    self.reconsume(); // pos 回到 `u`
    // §4.3.1 L963-967: unicode_ranges_allowed && would-start-unicode-range
    if self.unicode_ranges_allowed && self.would_start_unicode_range_at(0) {
        // §4.3.14 会 consume 并丢弃 `u` + `+`
        return self.consume_a_unicode_range_token();
    }
    // §4.3.1 L969-972: 否则 ident-like
    self.consume_an_ident_like_token()
}
```

> reconsume 后 `would_start_unicode_range_at(0)` 检查 `peek(0)`=`u`/`U`、`peek(1)`=`+`、`peek(2)`=`?`/hex，正确。`consume_an_ident_like_token` 会从 `u` 开始 consume ident sequence，正确。

#### 3.5 `consume_a_unicode_range_token`（§4.3.14 L1487-1548）

```rust
/// §4.3.14 (L1487-1548) Consume a unicode-range token.
///
/// Precondition: 流起始为 `u`/`U` + `+` + (?|hex)。
/// 返回 `UnicodeRange(Some(start), Some(end))`。
fn consume_a_unicode_range_token(&mut self) -> Token {
    // §4.3.14 step 1: consume 并丢弃 next two (`u`/`U` + `+`)
    self.consume(); // `u`/`U`
    self.consume(); // `+`

    // §4.3.14 step 2: consume ≤6 hex；不足 6 则 consume `?` 补足总数至 6
    let mut first_segment = String::new();
    while first_segment.len() < 6 {
        match self.peek(0) {
            Some(c) if is_hex_digit(c) => {
                first_segment.push(c);
                self.consume();
            }
            _ => break,
        }
    }
    // consume `?` 补足至总数 6
    while first_segment.len() < 6 {
        match self.peek(0) {
            Some('?') => {
                first_segment.push('?');
                self.consume();
            }
            _ => break,
        }
    }

    // §4.3.14 step 3: 若 first_segment 含 `?`
    if first_segment.contains('?') {
        // `?` → `0` 得 start
        let start_str: String = first_segment.chars().map(|c| if c == '?' { '0' } else { c }).collect();
        let start = u32::from_str_radix(&start_str, 16).unwrap_or(0);
        // `?` → `F` 得 end
        let end_str: String = first_segment.chars().map(|c| if c == '?' { 'F' } else { c }).collect();
        let end = u32::from_str_radix(&end_str, 16).unwrap_or(0);
        return Token::UnicodeRange(Some(start), Some(end));
    }

    // §4.3.14 step 4: first_segment 作 hex → start
    let start = u32::from_str_radix(&first_segment, 16).unwrap_or(0);

    // §4.3.14 step 5: 若 next 2 是 `-` + hex digit
    if self.peek(0) == Some('-') && self.peek(1).map_or(false, is_hex_digit) {
        self.consume(); // consume `-`
        let mut end_segment = String::new();
        while end_segment.len() < 6 {
            match self.peek(0) {
                Some(c) if is_hex_digit(c) => {
                    end_segment.push(c);
                    self.consume();
                }
                _ => break,
            }
        }
        let end = u32::from_str_radix(&end_segment, 16).unwrap_or(0);
        return Token::UnicodeRange(Some(start), Some(end));
    }

    // §4.3.14 step 6: 否则 start == end
    Token::UnicodeRange(Some(start), Some(start))
}
```

#### 3.6 新增测试（6 个）

```rust
#[test]
fn unicode_range_disabled_by_default() {
    // 默认 unicode_ranges_allowed=false：`U+1234` → [Ident("U"), Number(1234)]
    // `U` ident-start → Ident("U")（`+` 非 ident code point 停止）
    // `+1234` → starts_with_number（`+`+digit）→ Number(1234.0, true)
    let tokens = CssTokenizer::collect("U+1234");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], Token::Ident("U".to_string()));
    assert_eq!(tokens[1], Token::Number(Numeric { value: 1234.0, is_integer: true }));
}

#[test]
fn unicode_range_simple() {
    let mut tz = CssTokenizer::new("U+1234");
    tz.set_unicode_ranges_allowed(true);
    let t = tz.next_token().unwrap();
    assert_eq!(t, Token::UnicodeRange(Some(0x1234), Some(0x1234)));
}

#[test]
fn unicode_range_question_marks() {
    let mut tz = CssTokenizer::new("U+12??");
    tz.set_unicode_ranges_allowed(true);
    let t = tz.next_token().unwrap();
    assert_eq!(t, Token::UnicodeRange(Some(0x1200), Some(0x12FF)));
}

#[test]
fn unicode_range_range() {
    let mut tz = CssTokenizer::new("U+12-34FF");
    tz.set_unicode_ranges_allowed(true);
    let t = tz.next_token().unwrap();
    assert_eq!(t, Token::UnicodeRange(Some(0x12), Some(0x34FF)));
}

#[test]
fn unicode_range_max_hex() {
    let mut tz = CssTokenizer::new("U+10FFFF");
    tz.set_unicode_ranges_allowed(true);
    let t = tz.next_token().unwrap();
    assert_eq!(t, Token::UnicodeRange(Some(0x10FFFF), Some(0x10FFFF)));
}

#[test]
fn unicode_range_lowercase_u() {
    let mut tz = CssTokenizer::new("u+abc");
    tz.set_unicode_ranges_allowed(true);
    let t = tz.next_token().unwrap();
    assert_eq!(t, Token::UnicodeRange(Some(0xABC), Some(0xABC)));
}
```

#### 3.7 验证 + 提交

```powershell
cargo test -p muskitty-css
cargo check -p muskitty-css
```
预期 71/71 green（65 + 6）。

**Commit**：
```
[css-tokenizer] C-6: 4.3.1/4.3.11/4.3.14 unicode-range (gated by unicode_ranges_allowed)

- §4.3.1 L782-783: CssTokenizer.unicode_ranges_allowed field (default
  false) + Tokenizer::set_unicode_ranges_allowed trait method.
- §4.3.1 L960-972: consume_u_or_unicode_range reconsume `u`, if flag
  && would-start-unicode-range → consume_a_unicode_range_token, else
  ident-like.
- §4.3.11 would_start_unicode_range_at (L1356-1378): U/u + + + (?|hex).
- §4.3.14 consume_a_unicode_range_token (L1487-1548): discard `u`+`+`,
  consume ≤6 hex + `?` to 6; `?`-form → start(?=0)/end(?=F); else hex
  start, optional `-` + ≤6 hex end.
- Per §4.3.14 L1500-1506 note, unicode-range token only produced when
  flag set (for @font-face unicode-range descriptor parsing).
- 6 new tests.
```

### 阶段 4：C-7 — 清理 + doc 对齐 + clippy

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`

1. **更新顶部 doc coverage 块**（L8-24）：所有 `not yet` → `implemented`，或改为简洁的「§4.3 fully implemented」说明。
2. **删除 dead-code 允许装饰**：
   - `_ident_code_point_used` (L722-726) 及其 `#[allow(dead_code)]`：确认 `is_ident_code_point` 已被 `consume_a_hash_token` / `consume_an_ident_sequence` 实际调用后删除。
   - `_helpers_used` (L764-768) 及其 `#[allow(dead_code)]`：确认 `is_digit` / `is_hex_digit` 已被实际调用后删除。
3. **补 helper 注释**：`is_whitespace` / `is_non_printable` 加 §4.2 引用（C-5 已加，C-7 复查）。
4. **复查所有方法 doc** 中的 §章节号与 L 行号与规范一致。
5. **复查 `consume_a_token` 顶部注释**（L104-107）提到「C-2 onwards」等过期描述，更新为反映完整覆盖。

#### 验证

```powershell
cargo test -p muskitty-css
cargo clippy -p muskitty-css -- -D warnings
cargo check -p muskitty-css
```
预期 71/71 green，0 warning。

**Commit**：
```
[css-tokenizer] C-7: cleanup helpers + align docs with §4.3 coverage

- Update top-level coverage doc: §4.3.1-§4.3.15 fully implemented.
- Remove _ident_code_point_used / _helpers_used dead-code shims
  (helpers now referenced by real call sites).
- Align method doc comments with final §4.3 section/L-line references.
- clippy -D warnings: 0 warnings.
```

## 假设与决策

1. **规范源**：`D:\CSSWG\markdown\css-syntax-3\Overview.md`（用户明确指定，一切以标准为准）。
2. **不修改 `types.rs`**：`Numeric` / `Token::UnicodeRange` / `Token::Url` / `Token::BadUrl` 已存在，无需改动。
3. **不修改 `lib.rs`**：`tokenize()` 入口稳定，doc 已在 C-2 更新。
4. **`consume_a_token` 不加 `unicode_ranges_allowed` 参数**：spec 说该参数可选默认 false；实现通过 struct 字段 + trait setter 暴露，不污染 `consume_a_token` 签名。符合 §4.3.1 L782-783 语义。
5. **§4.3.15 实现采用 consume-then-check 模式**：比上一份计划的 peek-then-consume 更贴近规范字面「Repeatedly consume」，且语义等价（`\)` 不触发退出）。
6. **测试不修改规范**：所有测试期望按规范推导，不为了让测试通过而违反规范。
7. **逐提交**：C-4 收尾 → C-5 → C-6 → C-7，每个阶段一个 commit，用户自行 push（用户原话「你一个一个提交就行 弄好了我自己推送」）。
8. **C-4 代码已就绪**：本计划阶段 1 仅运行测试 + 提交，不重写代码（除非测试失败需修复）。

## 验证步骤（每阶段通用）

```powershell
cargo test -p muskitty-css
cargo check -p muskitty-css
```

C-7 额外：
```powershell
cargo clippy -p muskitty-css -- -D warnings
```

全部 green 且 4 个 commit 完成后，由用户执行 `git push`。
