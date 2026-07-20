# Phase 2 — muskitty-css Tokenizer C-2 收尾 → C-7

> 规范源：`D:\CSSWG\markdown\css-syntax-3\Overview.md`（CSS Syntax Module Level 3）
> 一切以标准为准。每个 commit message 必须引用 §章节号与 L 行号。

## 摘要

完成 muskitty-css tokenizer 的 §4.3 全部子算法实现，共 6 个提交批次（C-2 收尾 → C-7）。
当前 `impls.rs` 已覆盖 §4.3.1 简单 token、§4.3.2 注释、§4.3.4 ident/function/at-keyword/hash、§4.3.7/§4.3.8/§4.3.9/§4.3.12 原语；剩余 §4.3.3/§4.3.5/§4.3.6/§4.3.10/§4.3.11/§4.3.13/§4.3.14/§4.3.15 未实现或为 stub。

## 当前状态分析

### 已实现（C-0 + C-1 + C-2 部分）
- §4.3.1 `consume_a_token`：whitespace / `"`/`'` / `#` / `(`/`)` / `+`/`-`/`.` / `<` / `@` / `[`/`]` / `{`/`}` / `\` / digit / `u`/`U` / ident-start / `:`/`;`/`,` / EOF / 任何其它 → Delim。注释循环在 dispatch 前。
- §4.3.2 `consume_comments_body`：`/* ... */` 跳过，未终止吞到 EOF。
- §4.3.4 `consume_an_ident_like_token`：ident + function（**无 url( 特例**，C-5 补）。
- §4.3.4 at-keyword 分支 `consume_an_at_keyword_token`。
- §4.3.4 hash 分支 `consume_a_hash_token`（**有 Bug 1**）。
- §4.3.7 `consume_an_escaped_code_point`（**有 Bug 2**）。
- §4.3.8 `is_valid_escape_next` / `is_valid_escape_at`。
- §4.3.9 `would_start_ident_sequence_at`。
- §4.3.12 `consume_an_ident_sequence`。
- §5.3 `preprocess_input`：CR/LF/FF 归一化。

### Stub / 未实现
- §4.3.3 `consume_a_numeric_token` (L320-322)：`todo!("C-4: numeric token")`
- §4.3.5 `consume_a_string_token` (L268-301)：临时 escape 处理，未走 §4.3.7
- §4.3.4 url( 特例 (L337-346)：无，统一产 Function
- §4.3.6 `consume_a_url_token`：无
- §4.3.10 `starts_with_number` (L546-548)：恒返回 `false`
- §4.3.11 would-start-unicode-range：无
- §4.3.13 `consume_a_number`：无
- §4.3.14 `consume_a_unicode_range_token`：无
- §4.3.15 `consume_the_remnants_of_a_bad_url`：无
- §4.3.1 `unicode_ranges_allowed` 参数 (L782-783)：无

### 规范违规 Bug（C-2 收尾修复）

**Bug 1** — `consume_a_hash_token` (impls.rs L309-317) 缺 Delim 回退。
规范 §4.3.1 L801-826：`#` 后若**不是** ident code point 且**不是** valid escape → 返回 `Delim('#')`。
当前实现无条件产 `Hash`，对 `#<EOF>` / `#@` / `# ` 错误。

**Bug 2** — `consume_an_escaped_code_point` (impls.rs L432-435) EOF 返回 `\\`。
规范 §4.3.7 L1233-1236：EOF → parse error，返回 U+FFFD REPLACEMENT CHARACTER。
当前返回 `'\\'`。虽然当前调用路径 unreachable（caller 先过 §4.3.8 检查），但仍需按规范修正以保证 C-3 string token 走 §4.3.7 时正确。

### 测试期望错误（4 个，C-2 收尾修复）

| 测试 | 行 | 当前期望 | 规范正确值 | 原因 |
|---|---|---|---|---|
| `hash_unrestricted_type` | L903-917 | `Hash("", Unrestricted)` | `Hash("123", Unrestricted)` | 数字是 ident code point（§4.2），`consume_an_ident_sequence` 会吞掉 `123`；测试注释错误 |
| `declaration_with_ident_and_value` | L961-968 | `tokens.len() == 3` | `== 4` | `color: red` → [Ident, Colon, Whitespace, Ident]，4 个 token |
| `backslash_escape_in_ident` | L935-940 | `Ident("ab")`（输入 `a\b`） | 改输入 `a\z` → `Ident("az")` | `b` 是 hex digit，`\b`=U+000B；用 `z`（非 hex）才符合"非 hex 字面量"语义 |
| `unicode_escape_in_ident` | L971-978 | 2 tokens（`Ident("&B")` + `Whitespace`） | 1 token（`Ident("&B")`） | §4.3.7 L1223-1225：6 位 hex 后若 next 是 whitespace 则吞掉；`\000026 B` 的空格被吞，`B` 并入 ident |

## 提议变更

### 阶段 1：C-2 收尾

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`

1. **修 Bug 1** — `consume_a_hash_token` (L309-317)：
   按 §4.3.1 L801-826，先检查 next 是否 ident code point 或 valid escape；否则返回 `Delim('#')`。
   ```rust
   fn consume_a_hash_token(&mut self) -> Token {
       // §4.3.1 L801-826: # 后若不是 ident code point 且不是 valid escape → Delim
       let next_is_ident = self.peek(0).map_or(false, is_ident_code_point);
       let next_is_valid_escape = self.is_valid_escape_at(self.pos); // pos 指向 # 后第一个
       // 注意：is_valid_escape_at 期望 offset 指向 '\'，这里 # 后不是 '\'，故需直接判断
       // 实际：检查 peek(0)=='\' 且 peek(1) 非 newline/EOF
       if !next_is_ident && !self.is_valid_escape_next_for_hash() {
           return Token::Delim('#');
       }
       let hash_type = if self.would_start_ident_sequence_at(0) {
           HashType::Id
       } else {
           HashType::Unrestricted
       };
       let name = self.consume_an_ident_sequence();
       Token::Hash(name, hash_type)
   }
   ```
   > 实现细节：`is_valid_escape_next_for_hash` 等价于"peek(0)=='\' 且 peek(1) 非 newline 且非 EOF"。可复用 `is_valid_escape_at(self.pos)`（pos 已指向 # 后第一个 code point，因为 # 已被 consume）。验证：`is_valid_escape_at` 检查 `peek(offset)=='\'`，offset=pos 时即 peek(0)=='\'，正确。

2. **修 Bug 2** — `consume_an_escaped_code_point` (L432-435)：
   ```rust
   let Some(c) = self.consume() else {
       // §4.3.7 L1233-1236: EOF → parse error, return U+FFFD.
       return '\u{FFFD}';
   };
   ```
   同步更新方法 doc comment（L424「EOF → return `\`」→「EOF → return U+FFFD」）。

3. **修 4 个测试期望**：
   - `hash_unrestricted_type` (L903-917)：`assert_eq!(s, "123")`，更新注释说明数字是 ident code point。
   - `declaration_with_ident_and_value` (L961-968)：`assert_eq!(tokens.len(), 4)`，补 `assert!(matches!(&tokens[3], Token::Ident(s) if s == "red"))`。
   - `backslash_escape_in_ident` (L935-940)：输入改为 `"a\\z"`，期望 `Ident("az")`，注释说明 `z` 非 hex。
   - `unicode_escape_in_ident` (L971-978)：`assert_eq!(tokens.len(), 1)`，删除 Whitespace 断言，注释引用 §4.3.7 L1223-1225。

4. **新增 3 个测试**（验证 Bug 1）：
   ```rust
   #[test]
   fn hash_alone_is_delim() {
       // §4.3.1 L824-826: # 后 EOF → Delim('#')
       let tokens = CssTokenizer::collect("#");
       assert_eq!(tokens.len(), 1);
       assert!(matches!(tokens[0], Token::Delim('#')));
   }
   #[test]
   fn hash_followed_by_space_is_delim() {
       // §4.3.1: # 后空格（非 ident code point，非 valid escape）→ Delim
       let tokens = CssTokenizer::collect("# ");
       assert_eq!(tokens.len(), 2);
       assert!(matches!(tokens[0], Token::Delim('#')));
       assert!(matches!(tokens[1], Token::Whitespace));
   }
   #[test]
   fn hash_followed_by_at_is_delim() {
       // §4.3.1: # 后 @（非 ident code point，非 valid escape）→ Delim
       let tokens = CssTokenizer::collect("#@");
       assert_eq!(tokens.len(), 2);
       assert!(matches!(tokens[0], Token::Delim('#')));
       assert!(matches!(tokens[1], Token::Delim('@')));
   }
   ```

5. **验证**：
   - `cargo test -p muskitty-css` → 37/37 green（原 34 + 新增 3）
   - `cargo check -p muskitty-css` → 0 warning

6. **Commit**：
   ```
   [css-tokenizer] C-2: §4.3.4/§4.3.7/§4.3.8/§4.3.9/§4.3.12 ident/function/at-keyword/hash

   - §4.3.4 consume_an_ident_like_token: ident + function (url( 特例 C-5 补)
   - §4.3.4 at-keyword/hash 分支 + Delim 回退
   - §4.3.7 consume_an_escaped_code_point: hex escape + U+FFFD 兜底
   - §4.3.8/§4.3.9 valid escape / would-start-ident-sequence 谓词
   - §4.3.12 consume_an_ident_sequence

   Bug 1 修复 (§4.3.1 L801-826): consume_a_hash_token 缺 Delim 回退，
   #<EOF>/#@/#  错误产 Hash。
   Bug 2 修复 (§4.3.7 L1233-1236): EOF 返回 '\\' 改为 U+FFFD。

   修正 4 个测试期望：
   - hash_unrestricted_type: #123 → Hash("123", Unrestricted)（数字是 ident code point）
   - declaration_with_ident_and_value: color: red → 4 tokens
   - backslash_escape_in_ident: 输入 a\b 改 a\z（b 是 hex）
   - unicode_escape_in_ident: \000026 B → 1 token（§4.3.7 吞 trailing whitespace）

   新增 3 个测试覆盖 Bug 1: hash_alone_is_delim / hash_followed_by_space_is_delim /
   hash_followed_by_at_is_delim.
   ```

### 阶段 2：C-3 — §4.3.5 String + BadString

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`

替换 `consume_a_string_token` (L268-301) 临时实现，严格按 §4.3.5 L1081-1128：
```rust
fn consume_a_string_token(&mut self, quote: char) -> Token {
    // §4.3.5 L1092: value 初始为空
    let mut value = String::new();
    loop {
        match self.consume() {
            None => {
                // §4.3.5 L1101-1104: EOF → parse error, 返回 string-token
                return Token::String(value);
            }
            Some(c) if c == quote => {
                // §4.3.5 L1097-1099: ending code point → 返回
                return Token::String(value);
            }
            Some('\n') => {
                // §4.3.5 L1106-1110: newline → parse error, reconsume, 返回 bad-string
                self.reconsume();
                return Token::BadString;
            }
            Some('\\') => {
                // §4.3.5 L1112-1124
                match self.peek(0) {
                    None => {
                        // §4.3.5 L1114-1115: next is EOF, do nothing（继续循环，下次 EOF 命中）
                        // 注意：不 consume EOF，下一轮 consume() 返回 None → String 返回
                    }
                    Some('\n') => {
                        // §4.3.5 L1117-1119: next is newline, consume it（行续）
                        self.consume();
                    }
                    Some(_) => {
                        // §4.3.5 L1121-1124: valid escape, consume escaped code point
                        let escaped = self.consume_an_escaped_code_point();
                        value.push(escaped);
                    }
                }
            }
            Some(c) => {
                // §4.3.5 L1126-1128: anything else → append
                value.push(c);
            }
        }
    }
}
```

**新增测试**（约 6 个）：
- `string_double_quoted`：`"hello"` → `String("hello")`
- `string_single_quoted`：`'hello'` → `String("hello")`
- `string_with_escape`：`"a\nb"`（源码 `"a\\nb"`）→ `String("a\nb")`（`\n` = U+000B？不，`n` 非 hex → 字面 `n`；测试用 `"a\\z"` → `String("az")`）
- `string_unterminated_eof`：`"hello` → `String("hello")`（EOF parse error）
- `string_unescaped_newline`：`"a\nb"`（真实换行）→ `BadString`（value 丢弃）
- `string_line_continuation`：`"a\<newline>b"` → `String("ab")`
- `string_hex_escape`：`"\26"` → `String("&")`

**验证**：`cargo test -p muskitty-css` 全绿。

**Commit**：`[css-tokenizer] C-3: §4.3.5 string + bad-string tokens`
- 替换临时 escape 处理，走 §4.3.7 consume_an_escaped_code_point
- EOF / newline / 行续 / escape 四分支按 L1097-1128
- 新增 7 个测试

### 阶段 3：C-4 — §4.3.3/§4.3.13/§4.3.10 Numeric/Percentage/Dimension

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`

1. 实现 `starts_with_number` (L546-548) 按 §4.3.10 L1307-1352：
   ```rust
   fn starts_with_number(&self, first: char) -> bool {
       // §4.3.10: first 已 consume，检查 peek(0)/peek(1)
       match first {
           '+' | '-' => {
               match self.peek(0) {
                   Some(d) if is_digit(d) => true,
                   Some('.') => matches!(self.peek(1), Some(d) if is_digit(d)),
                   _ => false,
               }
           }
           '.' => matches!(self.peek(0), Some(d) if is_digit(d)),
           d if is_digit(d) => true,
           _ => false,
       }
   }
   ```
   > 注意：当前 `consume_a_token` 的 `+`/`-`/`.` 分支调用 `self.starts_with_number(c)` 时 `c` 已被 consume，pos 指向 c 之后。故 `first=c`，peek(0) 是 c 之后的第一个。符合 §4.3.10「three code points = current + next two」语义。

2. 实现 `consume_a_number` 按 §4.3.13 L1415-1483，返回 `(f64, bool /*is_integer*/)`：
   - sign / integer part / fraction part / exponent part
   - `type` flag：含 `.` 或 `e`/`E` → "number"（is_integer=false），否则 "integer"
   - 用 `f64::from_str` 解析 number part + exponent（避免手写 10^exp）

3. 实现 `consume_a_numeric_token` (L320-322) 按 §4.3.3 L1011-1042：
   ```rust
   fn consume_a_numeric_token(&mut self) -> Token {
       let (value, is_integer) = self.consume_a_number();
       // §4.3.3 L1019: 若 next 3 code points would start ident sequence → Dimension
       if self.would_start_ident_sequence_at(0) {
           let unit = self.consume_an_ident_sequence();
           return Token::Dimension(Numeric { value, is_integer }, unit);
       }
       // §4.3.3 L1032-1036: 若 next 是 % → Percentage
       if self.peek(0) == Some('%') {
           self.consume();
           return Token::Percentage(Numeric { value, is_integer });
       }
       // §4.3.3 L1040-1042: 否则 Number
       Token::Number(Numeric { value, is_integer })
   }
   ```

4. 修复 `Numeric` 构造：当前 `types.rs` 的 `Numeric` 是 `{ value: f64, is_integer: bool }`，无 sign character 字段。§4.3.13 返回 sign character，但 §4.3.3 传入 numeric token 时 sign 已包含在 value 内（`f64` 带符号）。故不需要单独 sign 字段。确认 `types.rs` 无需改。

**新增测试**（约 10 个）：
- `number_integer`：`42` → `Number(42.0, true)`
- `number_decimal`：`3.14` → `Number(3.14, false)`
- `number_signed`：`-5` → `Number(-5.0, true)`
- `number_exponent`：`1e3` → `Number(1000.0, false)`
- `number_signed_exponent`：`1.5e-2` → `Number(0.015, false)`
- `percentage_token`：`50%` → `Percentage(50.0, true)`
- `dimension_px`：`10px` → `Dimension(10.0, true, "px")`
- `dimension_em`：`1.5em` → `Dimension(1.5, false, "em")`
- `dimension_signed`：`-30deg` → `Dimension(-30.0, true, "deg")`
- `plus_sign_number`：`+5` → `Number(5.0, true)`
- `dot_starts_number`：`.5` → `Number(0.5, false)`

**验证**：`cargo test -p muskitty-css` 全绿。

**Commit**：`[css-tokenizer] C-4: §4.3.3/§4.3.10/§4.3.13 number/percentage/dimension`

### 阶段 4：C-5 — §4.3.4 url( 特例 + §4.3.6 Url + §4.3.15 BadUrl

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`

1. 改 `consume_an_ident_like_token` (L337-346) 按 §4.3.4 L1053-1078 实现 url( 特例：
   ```rust
   fn consume_an_ident_like_token(&mut self) -> Token {
       let name = self.consume_an_ident_sequence();
       // §4.3.4 L1053-1066: url( 特例
       if name.eq_ignore_ascii_case("url") && self.peek(0) == Some('(') {
           self.consume(); // consume (
           // §4.3.4 L1056-1057: while next two are whitespace, consume one
           while self.peek(0).map_or(false, is_whitespace) && self.peek(1).map_or(false, is_whitespace) {
               self.consume();
           }
           // §4.3.4 L1058-1063: 若 next 1-2 是 " / ' / whitespace+" / ' → Function
           let p0 = self.peek(0);
           let p1 = self.peek(1);
           let is_quote_case = matches!(p0, Some('"') | Some('\''))
               || (is_whitespace_opt(p0) && matches!(p1, Some('"') | Some('\'')));
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
   > 关键：L1056-1057「while next two are whitespace, consume one」实现为：当 peek(0) 与 peek(1) 都是 whitespace 时 consume 一个。这会把连续 whitespace 收敛到只剩 1 个。
   > 注意 `is_whitespace` 需按 §4.2 定义：U+0009 TAB / U+000A LF / U+000C FF（已预处理为 LF）/ U+000D CR / U+0020 SPACE。当前 `consume_a_token` whitespace 分支用 `' '|'\t'|'\n'|'\r'|'\u{000C}'`，一致。

2. 实现 `consume_a_url_token` 按 §4.3.6 L1132-1203：
   ```rust
   fn consume_a_url_token(&mut self) -> Token {
       let mut value = String::new();
       // §4.3.6 L1151: consume as much whitespace as possible
       while self.peek(0).map_or(false, is_whitespace) { self.consume(); }
       loop {
           match self.consume() {
               Some(')') => return Token::Url(value),       // L1157-1159
               None => return Token::Url(value),             // L1161-1164 (parse error)
               Some(c) if is_whitespace(c) => {              // L1166-1175
                   while self.peek(0).map_or(false, is_whitespace) { self.consume(); }
                   match self.peek(0) {
                       Some(')') => { self.consume(); return Token::Url(value); }
                       None => return Token::Url(value), // parse error
                       _ => {
                           self.consume_the_remnants_of_a_bad_url();
                           return Token::BadUrl;
                       }
                   }
               }
               Some('"') | Some('\'') | Some('(') | Some(c) if is_non_printable(c) => {
                   // L1177-1185: parse error, bad url
                   self.consume_the_remnants_of_a_bad_url();
                   return Token::BadUrl;
               }
               Some('\\') => {                              // L1187-1197
                   if self.is_valid_escape_next() {
                       let escaped = self.consume_an_escaped_code_point();
                       value.push(escaped);
                   } else {
                       self.consume_the_remnants_of_a_bad_url();
                       return Token::BadUrl;
                   }
               }
               Some(c) => value.push(c),                     // L1199-1202
           }
       }
   }
   ```
   > `is_non_printable`：§4.2 定义，U+0000-U+0008 / U+000B / U+000E-U+001F / U+007F-U+009F。需新增 helper。
   > 注意 `Some('"') | Some('\'') | Some('(') | Some(c) if is_non_printable(c)` 在 Rust match 中不能直接混合 literal 与 guard；需拆成两 arm 或用 `|` 模式 + 单独 guard。实现时用：
   ```rust
   Some(c) if c == '"' || c == '\'' || c == '(' || is_non_printable(c) => { ... }
   ```

3. 实现 `consume_the_remnants_of_a_bad_url` 按 §4.3.15 L1551-1576：
   ```rust
   fn consume_the_remnants_of_a_bad_url(&mut self) {
       loop {
           match self.consume() {
               Some(')') | None => return,   // L1563-1566
               Some(_) => {
                   if self.is_valid_escape_next() {
                       // L1568-1572: peek-then-consume（valid escape 时 consume escaped）
                       // 注意：is_valid_escape_next 检查 peek(0)=='\'... 但此处已 consume 当前 char
                       // 规范语义：「stream starts with a valid escape」指当前剩余流
                       // 实现：若刚 consume 的不是 '\'，则 valid escape 检查应基于 peek(0)
                       // 修正：规范 L1568 是「the input stream starts with a valid escape」，
                       //   即 peek(0)=='\' 且 peek(1) 非 newline/EOF
                       self.reconsume(); // 回退刚 consume 的，让 consume_an_escaped_code_point 处理
                       // 但 consume_an_escaped_code_point 期望 '\' 已 consume...
                       // 正确做法：检查 peek(0)=='\'，若是且 valid escape，则 consume '\' + consume_escaped
                   }
               }
           }
       }
   }
   ```
   > **实现细节修正**：§4.3.15「the input stream starts with a valid escape」指**剩余流**以 valid escape 开头，即 peek(0)=='\'。故实现应为：
   ```rust
   fn consume_the_remnants_of_a_bad_url(&mut self) {
       loop {
           if self.peek(0) == Some(')') || self.peek(0).is_none() {
               // peek-then-consume：先 peek，匹配则 consume 退出
               if self.peek(0).is_some() { self.consume(); }
               return;
           }
           if self.is_valid_escape_at(self.pos) {
               // valid escape: consume '\' then consume_an_escaped_code_point
               self.consume(); // consume '\'
               self.consume_an_escaped_code_point();
           } else {
               self.consume(); // consume anything else, do nothing
           }
       }
   }
   ```
   > 采用 peek-then-consume 模式（而非规范字面的「Repeatedly consume」），以支持 escaped `)` 被遇到而非提前退出。

**新增测试**（约 8 个）：
- `url_unquoted_simple`：`url(foo)` → `Url("foo")`
- `url_unquoted_with_spaces`：`url( foo )` → `Url("foo")`
- `url_quoted_is_function`：`url("foo")` → `Function("url")`（非 Url，因 quote 触发 L1058-1063）
- `url_single_quoted_is_function`：`url('foo')` → `Function("url")`
- `url_space_then_quote_is_function`：`url( "foo")` → `Function("url")`（whitespace + quote）
- `url_empty`：`url()` → `Url("")`
- `url_with_escape`：`url(foo\29 bar)` → `Url("foo)bar"`)（`\29 ` = `)`，trailing space 吞）
- `url_bad_unterminated`：`url(foo` → `Url("foo")`（EOF parse error，但仍 Url）
- `url_bad_paren_in_unquoted`：`url(foo(bar)` → `BadUrl`
- `url_bad_quote_in_unquoted`：`url(foo"bar)` → `BadUrl`

**验证**：`cargo test -p muskitty-css` 全绿。

**Commit**：`[css-tokenizer] C-5: §4.3.4/§4.3.6/§4.3.15 url + bad-url + url( special case`

### 阶段 5：C-6 — §4.3.1 unicode_ranges_allowed + §4.3.11 + §4.3.14 UnicodeRange

**文件**：
- `d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`
- `d:\Muskitty\crates\muskitty-css\src\tokenizer\trait_def.rs`
- `d:\Muskitty\crates\muskitty-css\src\tokenizer\mod.rs`

1. `trait_def.rs`：在 `Tokenizer` trait 加方法
   ```rust
   /// §4.3.1 L782-783: 设置 unicode_ranges_allowed 标志。
   /// 默认 false；仅在 @font-face unicode-range 描述符解析时设 true。
   /// 见 §4.3.14 L1500-1506 备注。
   fn set_unicode_ranges_allowed(&mut self, allowed: bool);
   ```
   并在 `impls.rs` 的 `impl Tokenizer for CssTokenizer` 加实现，`CssTokenizer` 加字段 `unicode_ranges_allowed: bool`，`new` 初始化为 `false`。

2. `impls.rs` 加 `would_start_unicode_range_at` 按 §4.3.11 L1356-1378：
   ```rust
   fn would_start_unicode_range_at(&self, offset: usize) -> bool {
       // §4.3.11: U/u, +, ? 或 hex digit
       let first = match self.peek(offset) { Some(c) => c, None => return false };
       if !matches!(first, 'U' | 'u') { return false; }
       if self.peek(offset + 1) != Some('+') { return false; }
       match self.peek(offset + 2) {
           Some('?') => true,
           Some(c) if is_hex_digit(c) => true,
           _ => false,
       }
   }
   ```

3. 改 `consume_u_or_unicode_range` (L375-380) 按 §4.3.1 L960-972：
   ```rust
   fn consume_u_or_unicode_range(&mut self) -> Token {
       // §4.3.1 L960-972
       if self.unicode_ranges_allowed && self.would_start_unicode_range_at(self.pos - 1) {
           // 注意：u/U 已 consume，pos 指向其后；would_start_unicode_range 需从 u 位置检查
           // 故传 self.pos - 1
           self.reconsume(); // 回退到 u
           return self.consume_a_unicode_range_token();
       }
       self.reconsume();
       self.consume_an_ident_like_token()
   }
   ```
   > 关键：`consume_a_token` 中 `consume()` 已吞掉 `u`/`U`，pos 指向 `u` 之后。`would_start_unicode_range_at` 需检查 3 个连续 code point（u, +, ?/hex），故应从 `pos-1`（即 `u` 的位置）开始。或改为在 `consume_a_token` 的 `u|U` 分支先 peek 再决定是否 consume，更清晰。**采纳后者**：改 `consume_a_token` 的 `u|U` 分支为：
   ```rust
   'u' | 'U' => {
       if self.unicode_ranges_allowed
           && self.would_start_unicode_range_at_offset_minus_1()
       {
           // reconsume u, consume unicode-range
           self.reconsume();
           self.consume_a_unicode_range_token()
       } else {
           self.reconsume();
           self.consume_an_ident_like_token()
       }
   }
   ```
   > 简化：因 `u` 已 consume，直接检查 `peek(-1)` 不可行。最简方案：在 `consume_u_or_unicode_range` 内 reconsume 后用 `would_start_ident_sequence_at(0)` 风格检查。即 reconsume 到 `u`，然后 `would_start_unicode_range_at(0)`（pos 指向 `u`）。实现：
   ```rust
   fn consume_u_or_unicode_range(&mut self) -> Token {
       self.reconsume(); // pos 回到 u
       if self.unicode_ranges_allowed && self.would_start_unicode_range_at(0) {
           self.consume_a_unicode_range_token()
       } else {
           self.consume_an_ident_like_token()
       }
   }
   ```
   > 这样 `consume_an_ident_like_token` 会从 `u` 开始 consume ident sequence，正确。

4. 实现 `consume_a_unicode_range_token` 按 §4.3.14 L1487-1548：
   - step 1: consume 2 个 code point（`u`/`U` + `+`），丢弃
   - step 2: consume ≤6 hex digit；若不足 6，consume `?` 补足至 6 总数 → first_segment
   - step 3: 若 first_segment 含 `?`：`?`→`0` 得 start，`?`→`F` 得 end，返回 `UnicodeRange(Some(start), Some(end))`
   - step 4: 否则 first_segment 作 hex → start
   - step 5: 若 next 2 是 `-` + hex digit：consume `-`，consume ≤6 hex digit → end，返回 `UnicodeRange(Some(start), Some(end))`
   - step 6: 否则 `UnicodeRange(Some(start), Some(start))`

5. `mod.rs`：`pub use types::{HashType, Numeric, State, Token};` 无需改（`UnicodeRange` 是 `Token` 变体）。`trait_def.rs` 的 `set_unicode_ranges_allowed` 通过 `Tokenizer` trait 暴露，`mod.rs` 已 `pub use trait_def::Tokenizer`。

**新增测试**（约 6 个）：
- `unicode_range_disabled_by_default`：`U+1234` 默认 → `Ident("U+1234"...)`（实际 `U` 是 ident-start，`+1234` 不是 ident code point，故 `Ident("U")` 然后 `Delim('+')`... 需仔细：`U` ident-start → consume_an_ident_like_token → consume_an_ident_sequence 吞 `U`（`+` 非 ident code point 非 escape）→ `Ident("U")`。然后 `+` → Delim（非 number，因 `+` 后 `1` 是 digit → starts_with_number true → consume_a_numeric_token → Number(1234)）。故 `U+1234` 默认 → [Ident("U"), Number(1234, true)]。测试断言此。
- `unicode_range_simple`：`set_unicode_ranges_allowed(true)` 后 `U+1234` → `UnicodeRange(Some(0x1234), Some(0x1234))`
- `unicode_range_question_marks`：`U+12??` → `UnicodeRange(Some(0x1200), Some(0x12FF))`
- `unicode_range_range`：`U+12-34FF` → `UnicodeRange(Some(0x12), Some(0x34FF))`
- `unicode_range_max_hex`：`U+10FFFF` → `UnicodeRange(Some(0x10FFFF), Some(0x10FFFF))`
- `unicode_range_lowercase_u`：`u+abc` → `UnicodeRange(Some(0xABC), Some(0xABC))`

**验证**：`cargo test -p muskitty-css` 全绿。

**Commit**：`[css-tokenizer] C-6: §4.3.1/§4.3.11/§4.3.14 unicode-range (gated by unicode_ranges_allowed)`

### 阶段 6：C-7 — 算法原语整理与 doc 对齐

**文件**：`d:\Muskitty\crates\muskitty-css\src\tokenizer\impls.rs`

1. 删除 `impls.rs` 顶部「Current coverage」注释块 (L8-24) 中所有「not yet」→ 改为「implemented」或删除该块，因 C-2~C-6 已全实现。
2. 删除 `_ident_code_point_used` / `_helpers_used` dead-code 允许装饰 (L570-573, L612-615)，确认 `is_digit` / `is_hex_digit` / `is_ident_code_point` 已被实际调用。
3. 补 `is_non_printable` / `is_whitespace` helper 的 §4.2 引用注释。
4. 检查所有方法 doc comment 中的 §引用与 L 行号与当前规范一致。
5. `cargo clippy -p muskitty-css -- -D warnings` 确保 0 warning。

**验证**：
- `cargo test -p muskitty-css` 全绿
- `cargo clippy -p muskitty-css -- -D warnings` 0 warning
- `cargo check -p muskitty-css` 0 warning

**Commit**：`[css-tokenizer] C-7: cleanup helpers + align docs with §4.3 coverage`

## 假设与决策

1. **规范源**：`D:\CSSWG\markdown\css-syntax-3\Overview.md`（用户明确指定，一切以标准为准）。
2. **C-6 UnicodeRange**：用户选「一切以标准为准」= 实现原语 + 用 `unicode_ranges_allowed` 标志接线。默认 false，仅 `@font-face unicode-range` 描述符解析时设 true（未来 CSSOM 阶段）。
3. **不修改 `types.rs`**：`Numeric` 已有 `value: f64` + `is_integer: bool`，sign 含于 value，无需 sign character 字段。`Token::UnicodeRange(Option<u32>, Option<u32>)` 已存在。
4. **不修改 `lib.rs`**：`tokenize()` 入口已稳定。
5. **`consume_a_token` 不加 `unicode_ranges_allowed` 参数**：spec 说该参数可选默认 false；实现通过 struct 字段 + trait setter 暴露，不污染 `consume_a_token` 签名。
6. **测试不修改规范**：所有测试期望按规范推导，不为了让测试通过而违反规范。
7. **逐提交**：C-2 收尾 → C-3 → C-4 → C-5 → C-6 → C-7，每个阶段一个 commit，用户自行 push。
8. **`is_whitespace` / `is_non_printable` helper**：C-5 新增，§4.2 定义。whitespace = U+0009/000A/000C/000D/0020（预处理后 000C/000D 已归 000A，但 helper 仍按规范完整定义）。

## 验证步骤（每阶段通用）

```powershell
cargo test -p muskitty-css
cargo check -p muskitty-css
```

C-7 额外：
```powershell
cargo clippy -p muskitty-css -- -D warnings
```

全部 green 后，由用户执行 `git push`（用户原话「弄好了我自己推送」）。
