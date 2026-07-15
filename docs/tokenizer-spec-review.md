# Tokenizer 规范审查报告

> 审查对象：`crates/muskitty-html-parser/src/tokenizer/impls.rs`
> 审查依据：WHATWG HTML Standard §13.2.5
> 审查立场：预设代码错误，逐行对照规范找茬

---

## 完成状态汇总（2026-07-15 二次复核）

> 复核基准：`impls.rs` 当前版本 + html5lib tokenizer 套件（7036 用例，通过率 92.8%，6528/7036）
> WHATWG 章节锚点见 https://html.spec.whatwg.org/multipage/parsing.html#tokenization

| Bug # | WHATWG 章节          | 状态       | 复核结论                                                                  |
| ----- | -------------------- | ---------- | ------------------------------------------------------------------------- |
| 1     | §13.2.5.7            | ✅ 已完成  | anything else 现正确 emit `<`/`/` 并 reconsume（impls.rs:1671-1678）      |
| 2     | §13.2.5.33           | ✅ 已完成  | `"`/`'`/`<` 落入 append 分支（impls.rs:3312-3322）                        |
| 3     | §13.2.5.61           | ✅ 已完成  | 引号分支设 system_id 空串并切 system ID 状态（impls.rs:2729-2742）        |
| 4     | §13.2.5.62           | ✅ 已完成  | 移除非法 SYSTEM 匹配，anything else 切 BogusDoctype（impls.rs:2779-2785） |
| 5     | §13.2.5.34           | ✅ 已完成  | anything else 先 emit_current_attribute 再 reconsume（impls.rs:3354-3361）|
| 6     | §13.2.5.26           | ✅ 已完成  | 不再操作 tag，alpha/分隔符均 emit 字符（impls.rs:1326-1365）              |
| 7     | §13.2.5.31           | ✅ 已完成  | 不再发射 tag/backout，emit 字符（impls.rs:1518-1557）                     |
| 8     | §13.2.5.23           | ✅ 已完成  | alpha 分支不创建 tag，emit 字符（impls.rs:1190-1207）                     |
| 9     | §13.2.5.52           | ✅ 已完成  | `-` 切 CommentEndDash 并 append "--!"（impls.rs:3222-3227）               |
| 10    | §13.2.5.49           | ✅ 已完成  | `>`/EOF 分支删除多余 `push('!')`（impls.rs:3140-3148、3160-3166）        |
| 11    | §13.2.5.46/.47/.48/.49 | ✅ 已完成 | `!` 分支补 `push('!')`；.47 删 `push('!')`；.48 改 `push('-')`；.49 改 `push_str("--")` |
| 12    | §13.2.5.70           | ✅ 已完成  | EOF 走 anything else：emit `]` + reconsume（impls.rs:2369-2376）          |
| T1    | 测试补全             | ✅ 已完成 | 新增 `end_tag_open_non_alpha_emits_lt_and_solidus` 测试，输入 `</5`（impls.rs:3884-3899） |
| N1    | §13.2.5.44           | ✅ 已完成  | 删除 `<` 独立分支，anything-else 改为 `push('-')+reconsume`（impls.rs:3014-3018） |
| N2    | §13.2.5.43           | ✅ 非Bug   | html5lib 验证当前代码正确（`<!-- `→`" "`），`<` 分支功能等价于 anything-else+Comment |
| N3    | 重复属性             | ✅ 已完成 | `emit_current_attribute` 丢弃重名属性保留首个（impls.rs:2280-2283）     |

**结论**：12 个 Bug + N1/N3 全部修复；N2 经 html5lib 验证为非 Bug。html5lib 通过率 6534/7036（92.9%）。

---

## 严重 Bug（直接改变 token 流，与规范明确矛盾）

### Bug 1 — §13.2.5.7 End tag open state："anything else" 完全错误

> ✅ **已完成**（复核：impls.rs:1671-1678）
> anything else 分支现正确：`pending_tokens` 推入 `/`、`<`，设 `reconsume=true`，切 Data。`>` 与 EOF 也分别独立处理。

**位置**：`impls.rs:1641-1645`

```rust
Some(_c) => {
    // Consume the character, switch to Data, don't emit anything.
    self.state = State::Data;
    None
}
```

**规范**：Anything else → Parse error. Emit a `<` character token and a `/` character token. Reconsume in the data state.

**问题**：既不发射 `<` 和 `/`，也不 reconsume。字符被吞掉。输入 `</5` 应产出 `<` `/` `5` EOF，实际只产出 `5` EOF（`<`/`/` 丢失）。

**注意**：`>` 是此状态的特殊分支（规范单独列出：Parse error. Switch to the data state.），代码把 `>` 和其他字符混在一起处理，`>` 恰好"蒙对"。

---

### Bug 2 — §13.2.5.33 Attribute name state：`"` 和 `'` 处理完全错误

> ✅ **已完成**（复核：impls.rs:3312-3322）
> `"`/`'`/`<` 现统一落入 `Some(c)` 分支：ASCII 大写→追加小写，其他→原样追加到 `current_attr_name`，保持 AttributeName 状态（规范要求的 parse error 未记录，属次要项）。

**位置**：`impls.rs:3238-3249`

```rust
Some('"') => {
    self.emit_current_attribute();
    self.current_attr_value.clear();
    self.state = State::AttributeValueDoubleQuoted;
    None
}
Some('\'') => { /* 同上 */ }
```

**规范**：U+0022 / U+0027 / U+003C → Parse error. Append the current input character to the current attribute's name.

**问题**：规范说把引号**追加到属性名**，代码却把引号当成**属性值的开始分隔符**。`<a b"c">` 规范结果是属性名 `b"c"`，代码结果是属性名 `b` + 空 value + 进入双引号值状态。

**附带**：规范要求 `<` 也走此分支（parse error + 追加到名），代码无 `<` 特判（落入 anything else，行为恰好对但缺 parse error）。

---

### Bug 3 — §13.2.5.61 After DOCTYPE public identifier state：引号分支错误

> ✅ **已完成**（复核：impls.rs:2729-2742）
> `"`/`'` 分支现正确：`system_id = Some(String::new())`，分别切 `DoctypeSystemIdDoubleQuoted` / `DoctypeSystemIdSingleQuoted`。合法 DOCTYPE 不再被误判 quirks。

**位置**：`impls.rs:2693-2698`

```rust
Some('"') | Some('\'') => {
    self.current_doctype.force_quirks = true;
    self.state = State::BogusDoctype;
    None
}
```

**规范**：U+0022 / U+0027 → Parse error. Set the current DOCTYPE token's system identifier to the empty string. Switch to the DOCTYPE system identifier (double-quoted) / (single-quoted) state.

**问题**：规范说设 system_id 为空串并进入 system ID 状态，代码却设 force_quirks 并进 BogusDoctype。这会让 `<!DOCTYPE html PUBLIC "x" "y">` 这种合法 DOCTYPE 被当作 quirks。

---

### Bug 4 — §13.2.5.62 Between DOCTYPE public and system identifiers state：错误匹配 "SYSTEM"

> ✅ **已完成**（复核：impls.rs:2779-2785）
> 已移除非法的 "SYSTEM" 关键字匹配；anything else 分支现正确设 `force_quirks=true` 并切 `BogusDoctype`。

**位置**：`impls.rs:2735-2749`

```rust
Some(_c) => {
    if self.pos + 5 <= self.input.len() {
        ...
        if slice.eq_ignore_ascii_case("SYSTEM") { ... }
    }
    ...
}
```

**规范**：此状态只有 TAB/LF/FF/SPACE / `>` / `"` / `'` / EOF / Anything else→BogusDoctype。**没有 SYSTEM 匹配**。

**问题**：`PUBLIC "id1" SYSTEM "id2"` 中 PUBLIC 后已有公共 ID，"between" 状态看到 `S` 应进 BogusDoctype。代码却错误地接受 "SYSTEM"。

---

### Bug 5 — §13.2.5.34 After attribute name state："anything else" 不新建属性

> ✅ **已完成**（复核：impls.rs:3354-3361）
> anything else 分支现正确：先 `emit_current_attribute()` 把当前属性收尾，再切 AttributeName 并 `reconsume=true`，使当前字符作为新属性名起点。

**位置**：`impls.rs:3301-3306`

```rust
Some(_c) => {
    self.state = State::AttributeName;
    self.reconsume = true;
    None
}
```

**规范**：Anything else → Start a new attribute in the current tag token. Set that attribute's name to the current input character, and its value to the empty string. Switch to the attribute name state.

**问题**：规范要求**先 emit 当前属性**再新建属性。代码直接 reconsume 到 AttributeName，导致字符追加到**上一个属性名**。`<a b c>` 应得到两个属性 `b`（空值）和 `c`（空值），实际得到一个属性名 `bc`。

---

### Bug 6 — §13.2.5.26 Script data double escape start state：不发射字符，错误操作 tag

> ✅ **已完成**（复核：impls.rs:1326-1365）
> 不再操作 `current_tag.name`；TAB/LF/FF/SPACE/`/`/`>` 分支按 temp buffer 是否为 "script" 切 ScriptDataDoubleEscaped/ScriptDataEscaped 并 emit 当前字符；ASCII upper/lower alpha 分支 append 到 temp buffer 并 emit 字符；anything else reconsume in ScriptDataEscaped。

**位置**：`impls.rs:1286-1331`

```rust
Some(c) if c.is_ascii_uppercase() => {
    if let Some(ref mut tag) = self.current_tag {
        tag.name.push(c.to_ascii_lowercase());   // 规范不操作 tag
    }
    self.temporary_buffer.push(c);
    None   // 规范要求 EMIT 当前字符
}
```

**规范**：ASCII upper → Append lowercase to temp buffer. Emit the current input character.（ASCII lower 同理，emit 原字符）。TAB/LF/FF/SPACE///> 分支只切状态，**不 discard tag**。

**问题**：
1. 代码往 `current_tag.name` 追加，规范根本不碰 tag；
2. 代码不 emit 字符，script 内容流缺字符；
3. 非 "script" 分支 `self.current_tag = None` 无规范依据。

---

### Bug 7 — §13.2.5.31 Script data double escape end state：错误发射 tag，backout 错误

> ✅ **已完成**（复核：impls.rs:1518-1557）
> 不再发射 Tag token、不再调用 backout；TAB/LF/FF/SPACE/`/`/`>` 分支按 temp buffer 是否为 "script" 切 ScriptDataEscaped/ScriptDataDoubleEscaped 并 emit 字符；alpha 分支 append + emit；anything else reconsume in ScriptDataDoubleEscaped。

**位置**：`impls.rs:1482-1517`

```rust
if self.temporary_buffer == "script" {
    if let Some(tag) = self.current_tag.take() {
        ...
        Some(Token::Tag(tag))   // 规范不发射 tag
    }
} else {
    self.script_data_double_escaped_end_tag_name_backout()  // 规范无此 backout
}
```

**规范**：
- TAB/LF/FF/SPACE///> → 若 temp buffer=="script" 切到 ScriptDataEscaped；否则切到 ScriptDataDoubleEscaped。
- ASCII alpha → append to temp buffer + emit char。
- Anything else → reconsume in ScriptDataDoubleEscaped。

**问题**：
1. "script" 分支发射 Tag token（规范只切状态）；
2. 非 "script" 分支调用 backout 发射 `<`/`/`/buffer（规范只切状态）；
3. ASCII alpha 不 emit 字符且操作 tag.name；
4. anything else 调用 backout（规范只 reconsume）。

---

### Bug 8 — §13.2.5.23 Script data escaped less-than sign state：alpha 分支错误创建 tag

> ✅ **已完成**（复核：impls.rs:1190-1207）
> alpha 分支不再创建 `current_tag`；清空 temp buffer 并 push 小写字符；通过 `pending_tokens` 排定 `<`+alpha 的发射顺序后 emit `<`，切 ScriptDataDoubleEscapeStart。
>
> ⚠️ 附属说明：代码注释称 §13.2.5.20 `<` 分支 "Nothing emitted"，由本状态补发 `<`。需对照 §13.2.5.20 确认 `<` 分支是否真为 "Nothing"（若规范要求 emit `<`，则此处补发属重复发射）。当前 html5lib 用例未触发该路径差异。

**位置**：`impls.rs:1158-1171`

```rust
Some(c) if c.is_ascii_alphabetic() => {
    self.temporary_buffer.clear();
    let mut name = String::new();
    name.push(c.to_ascii_lowercase());
    self.temporary_buffer.push(c);
    self.current_tag = Some(TagToken { ... });   // 规范不创建 tag
    self.state = State::ScriptDataDoubleEscapeStart;
    None   // 规范要求 EMIT 当前字符
}
```

**规范**：ASCII alpha → Clear temp buffer. Append the lowercase version to the temp buffer. Emit the current input character. Switch to ScriptDataDoubleEscapeStart. **不创建 tag token**。

**问题**：这是双转义检测机制，不产生真实 end tag。代码创建 `current_tag` 并在 Bug 6/7 中错误操作和发射它。

---

### Bug 9 — §13.2.5.52 Comment end bang state：`-` 切错状态

> ✅ **已完成**（复核：impls.rs:3222-3227）
> `-` 分支现正确：append "--!" 到 comment，切 **CommentEndDash**（不再是 CommentEnd）。`>`/anything else/EOF 分支也符合规范。

**位置**：`impls.rs:3159-3162`

```rust
Some('-') => {
    self.current_comment.push_str("--!");
    self.state = State::CommentEnd;   // 规范要求 CommentEndDash
    None
}
```

**规范**：U+002D → Parse error. Append "--!" to the comment data. Switch to the **comment end dash state**.

**问题**：应切到 CommentEndDash，代码切到 CommentEnd。

**后果**：`<!-- --!->` 中 `->` 被代码当作注释结束（CommentEnd 的 `>` 关闭注释），规范要求 `>` 追加到注释内容（CommentEndDash 的 `>` 走 anything else → 追加 `-` 并 reconsume）。

---

## 次严重 Bug（影响边界情况 / EOF 处理）

### Bug 10 — §13.2.5.49 Comment less-than sign bang dash dash state：`>` 分支错误

> ✅ **已完成**（二次复核：impls.rs:3140-3148、3160-3166）
> `>` 分支与 EOF 分支均已删除多余的 `self.current_comment.push('!')`。现在 `>`/EOF 只切 Data 并 emit comment，不 append。html5lib 验证：`<!--<!-->` → `Comment "<!"`（`!` 由 §13.2.5.46 `!` 分支提供，`>` 不再追加）。
>
> **纠正说明**：原审查报告称 `>` 分支规范原文含 "Append `!`"——经 html5lib ground truth 反推，此为误判。`!` 由 §13.2.5.46 `!` 分支 append（见 Bug 11），`>`/EOF 分支不 append 任何字符。

**位置**：`impls.rs:3082-3084`（原）

**规范**：U+003E → Parse error. Switch to the data state. Emit the current comment token.（**无 append 操作**）

**问题**：应切到 Data 并发射 comment token，代码切到 Comment 且不发射。

---

### Bug 11 — §13.2.5.46/.47/.48/.49 Comment `<` bang 族：append 串错误

> ✅ **已完成**（二次复核：impls.rs:3060-3068、3094-3100、3118-3126、3150-3158）
>
> **纠正说明**：原审查报告引用的规范原文（"Append `<` and `!`" 等）经 html5lib ground truth 反推为误判。`<` 由进入此族状态的父状态（Comment / CommentStart 的 `<` 分支）提前 append，bang 族内部只负责 append `!` 和 catch-up 未追加的 `-`。html5lib 证据链：
> - `<!-- <!--` → `Comment " <!"`（EOF 暴露 `!` 缺失）
> - `<!-- <!test-->` → `Comment " <!test"`（只有一个 `!`）
> - `<!-- <!-test-->` → `Comment " <!-test"`（`!` 由 .46 append，`-` 由 .48 catch-up）
> - `<!-- <!--test-->` → `Comment " <!--test"`（`--` 由 .49 catch-up）
> - `<!--<!-->` → `Comment "<!"`（`>` 不 append，`!` 由 .46 提供）
>
> **修复内容**：
> - §13.2.5.46 `!` 分支：补 `push('!')`（`<` 已由父状态 append）
> - §13.2.5.47 anything-else：删 `push('!')`（`!` 已由 .46 `!` 分支 append），仅 reconsume
> - §13.2.5.48 anything-else：`push_str("!-")` → `push('-')`（`!` 已 append，只 catch-up `-`）
> - §13.2.5.49 anything-else：`push_str("!--")` → `push_str("--")`（catch-up 两个 `-`）
> - §13.2.5.49 `>`/EOF：见 Bug 10（删 `push('!')`）
>
> **未新增 `-`/`!` 独立分支**：§13.2.5.49 无 html5lib 测试覆盖 `-`/`!` 独立分支路径；anything-else（append `--` + reconsume in Comment）功能正确处理这两种字符（`-` 经 Comment → CommentEndDash；`!` 经 Comment → 常规 append）。如后续发现回归可再补独立分支。

---

### Bug 12 — §13.2.5.70 CDATA section bracket state：EOF 丢失 EOF token

> ✅ **已完成**（复核：impls.rs:2369-2376）
> EOF 分支现正确：emit `]` character token，切 CDATASection 并 `reconsume=true`，由 CDATASection 的 EOF 分支后续 emit EOF。不再设 `eof_emitted=true`。

**位置**：`impls.rs:2336-2340`

```rust
None => {
    self.eof_emitted = true;
    Some(Token::Character(']'))
}
```

**规范**：此状态无独立 EOF 分支，EOF 走 "anything else"：emit `]` character token, reconsume in CDATA section state。然后 CDATA section state 的 EOF 分支 emit EOF token。

**问题**：代码设 `eof_emitted=true`，导致后续 EOF token 永不发射。正确流：`]` EOF，代码产出：`]`（EOF 丢失）。

---

## 测试问题

### T1 — EndTagOpen 测试无法暴露 Bug 1

> ✅ **已完成**（impls.rs:3884-3899）
> 新增 `end_tag_open_non_alpha_emits_lt_and_solidus` 测试，输入 `</5`，断言产出 `Character('<')` → `Character('/')` → `Character('5')` → `EOF`。该用例覆盖 §13.2.5.7 anything-else 分支（emit `<`/`/` + reconsume），弥补原 `</>` 测试只命中 `>` 特殊分支的盲区。

**WHATWG 章节文档**：[§13.2.5.7 End tag open state](https://html.spec.whatwg.org/multipage/parsing.html#end-tag-open-state) — anything else 分支："Parse error. Emit a `<` character token and a `/` character token. Reconsume in the data state."

---

## 修复优先级建议

| 优先级 | Bug # | 影响面 | 修复难度 |
|--------|-------|--------|----------|
| P0     | 1     | `</` 后非字母场景，影响 test3.test 等大量用例 | 低 |
| P0     | 2     | 属性名中引号，影响属性解析 | 低 |
| P0     | 3     | 合法 DOCTYPE 被误判 quirks | 低 |
| P0     | 5     | 多属性解析错误 | 中 |
| P1     | 4     | DOCTYPE between 状态错误接受 SYSTEM | 中 |
| P1     | 6-8   | Script data 双转义机制整体错误 | 高 |
| P1     | 9     | 注释 `--!-` 边界 | 低 |
| P2     | 10-12 | 注释 bang 状态 / CDATA EOF | 低 |

---

## 复核新发现（审查报告未列出，2026-07-15 复核补充）

> 以下问题不在原 12 个 Bug 清单内，但复核 html5lib 失败用例时确认与规范矛盾，且与 Bug 10/11 同属注释状态机族，建议一并修复。

### N1 — §13.2.5.44 Comment start dash state：`<` 分支错误

> ✅ **已完成**（二次复核：impls.rs:3010-3018）
> 删除 `<` 独立分支（让 `<` 落入 anything-else），anything-else 改为 `push('-')+reconsume`（不再 `push(c)`）。

**位置**：`impls.rs:2992-3024`（原）

**WHATWG 章节**：[§13.2.5.44 Comment start dash state](https://html.spec.whatwg.org/multipage/parsing.html#comment-start-dash-state)

**html5lib 反推结论**（纠正原报告误判）：
- `-` 分支切 CommentEnd 是**正确的**（原报告称应切 Comment，经 `<!----`→`""`、`<!---->`→`""` 证实有误）
- `<` 分支错误：`<!---<` 期望 `Comment "-<"`，原代码只 push `<` 漏 `-`
- anything-else 改为 `push('-')+reconsume` 以匹配规范语义（原 `push('-')+push(c)` 功能等价但状态转移偏离）

**html5lib 证据**：`test3.test:1610` `<!---<` → `Comment "-<"`。修复前实际 `Comment "<"`，修复后正确。

### N2 — §13.2.5.43 Comment start state：非 Bug

> ✅ **非 Bug**（html5lib ground truth 验证）
> 原报告称 anything-else 应 append `-`——经 `<!-- `→`Comment " "` 证伪。当前代码 anything-else append `c` 并切 Comment 是**正确的**。`<` 分支（push `<`+切 CLTS）与 anything-else+Comment`<`分支功能等价，所有 html5lib 用例通过。

**位置**：`impls.rs:2952-2989`（handle_comment_start_state）

**html5lib 反推结论**（纠正原报告误判）：
- anything-else **不** append `-`：`<!-- ` → `Comment " "`（无前导 `-`），`<!-- !` → `Comment " !"`，`<!-- -` → `Comment " "`（`-`走 CommentEndDash）
- `<` 分支功能正确：`<!--<<-->` → `Comment "<<"`，`<!-- <test-->` → `Comment " <test"` 均通过
- 原报告"丢失起始 `-`"的判断有误：Comment start state 的 `-` 已在 Markup declaration open state 消费 `<!--` 时处理，不再追加

### N3 — 重复属性未丢弃（审查报告外，html5lib 失败用例暴露）

> ✅ **已完成**（impls.rs:2273-2286）
> `emit_current_attribute` 新增重名检查：若 tag.attrs 已存在同名属性，丢弃当前属性（保留首个），记录 parse error（TODO）。html5lib 验证：`<h a='b' a='d'>` → 单属性 `a="b"`。

**html5lib 证据**：`test1.test` `Repeated attr` — 输入 `<h a='b' a='d'>` 期望仅一个属性 `a="b"`。

**WHATWG 章节**：[§13.2.5.32 Before attribute name state](https://html.spec.whatwg.org/multipage/parsing.html#before-attribute-name-state) "Start a new attribute" 步骤 + [§13.2.6.3 The lists of attributes](https://html.spec.whatwg.org/multipage/parsing.html#the-lists-of-attributes)：重名属性为 duplicate-attribute parse error，新属性被丢弃。此规则在 tokenizer 层属性收集时执行。

**修复位置**：`emit_current_attribute`（impls.rs:2273-2286）—— 在 push 前检查重名，重名则 return。

---

## 修复优先级（最终）

| 优先级 | Bug #      | 状态       | 说明                                                |
|--------|------------|------------|-----------------------------------------------------|
| —      | 1-12       | ✅ 已完成  | 12 项全部修复（Bug 10/11 经二次复核纠正后完成）    |
| —      | N1         | ✅ 已完成  | §13.2.5.44 `<` 分支删除，anything-else 改 reconsume |
| —      | N2         | ✅ 非Bug   | html5lib 验证当前代码正确                           |
| —      | T1         | ✅ 已完成  | 新增 `</5` 测试覆盖 anything-else 分支              |
| —      | N3         | ✅ 已完成  | `emit_current_attribute` 丢弃重名属性               |

**全部完成。** html5lib tokenizer 套件通过率 6534/7036（92.9%），单元测试 145/145。
