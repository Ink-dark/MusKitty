# MusKitty 浏览器核心模块 — 后续开发路线图

## Context

MusKitty 是从零用 Rust 重写浏览器核心模块的项目。当前处于 Phase 1（HTML Parser），已完成 **2/80 个 tokenizer 状态**（Data + TagOpen）。整个 HTML parser 分为两个阶段：tokenization（§13.2.5）→ tree construction（§13.2.6），之后还需要 DOM 层、CSS 解析、布局、网络等模块。需要一条清晰的渐进路线。

## 总体路线

```
Tokenizer (remain 78 states) → Error types → Tree Construction → DOM types
                                    ↓
                          (短期里程碑：能解析完整 HTML 文档产生 token 流)
                                    ↓
                          CSS Parser → Layout Engine → Networking → ...
```

## Phase 1：完成 Tokenizer（§13.2.5 剩余 78 个状态）

按依赖关系和功能分组，由浅入深逐步实现。每组完成后 `cargo check` 零 warning + `cargo test` 全绿。

### Step 1.1：核心标签解析（3 个状态，~150 行代码）

- **EndTagOpen** (§13.2.5.7)：`</` 之后，判断 ASCII alpha → TagName（创建 end tag），否则走 BogusComment
- **TagName** (§13.2.5.8)：累积标签名（lowercase），遇到 whitespace → BeforeAttributeName，`>` 或 `/` → emit tag token
- **SelfClosingStartTag** (§13.2.5.40)：`/>` 处理，设置 self_closing flag

**目标**：能完整解析 `<div class="x">` → emit 正确的 TagToken（含 attributes）

**前置依赖**：无。TagOpen 已经完成了 `!`、`/`、alpha、`?`、EOF 的处理，EndTagOpen 是直接下游。

### Step 1.2：属性状态（9 个状态，~300 行代码）

- **BeforeAttributeName** (§13.2.5.32)
- **AttributeName** (§13.2.5.33) — 属性名 lowercase
- **AfterAttributeName** (§13.2.5.34)
- **BeforeAttributeValue** (§13.2.5.35)
- **AttributeValueDoubleQuoted** (§13.2.5.36)
- **AttributeValueSingleQuoted** (§13.2.5.37)
- **AttributeValueUnquoted** (§13.2.5.38)
- **AfterAttributeValueQuoted** (§13.2.5.39)
- **SelfClosingStartTag**（Step 1.1 已有，属性结束后会用到）

**目标**：解析 `<div class="foo" id='bar' hidden>` 正确填充 `TagToken.attrs`

### Step 1.3：注释状态（13 个状态，~250 行代码）

- **BogusComment** (§13.2.5.41) — TagOpen 里 `?` 或 EndTagOpen 里非 alpha 触发
- **MarkupDeclarationOpen** (§13.2.5.42) — `<!` 之后判断 `--`（comment）或 `DOCTYPE` 等
- **CommentStart** (§13.2.5.43)
- **CommentStartDash** (§13.2.5.44)
- **Comment** (§13.2.5.45)
- **CommentLessThanSign** (§13.2.5.46)
- **CommentLessThanSignBang** (§13.2.5.47)
- **CommentLessThanSignBangDash** (§13.2.5.48)
- **CommentLessThanSignBangDashDash** (§13.2.5.49)
- **CommentEndDash** (§13.2.5.50)
- **CommentEnd** (§13.2.5.51)
- **CommentEndBang** (§13.2.5.52)

**目标**：`<!-- comment -->` 正确产出 `Token::Comment`

### Step 1.4：DOCTYPE 状态（16 个状态，~350 行代码）

- **Doctype** (§13.2.5.53)
- **BeforeDoctypeName** (§13.2.5.54)
- **DoctypeName** (§13.2.5.55)
- **AfterDoctypeName** (§13.2.5.56)
- **AfterDoctypePublicKeyword** (§13.2.5.57)
- **BeforeDoctypePublicId** (§13.2.5.58)
- **DoctypePublicIdDoubleQuoted** (§13.2.5.59)
- **DoctypePublicIdSingleQuoted** (§13.2.5.60)
- **AfterDoctypePublicId** (§13.2.5.61)
- **BetweenDoctypePublicAndSystemIds** (§13.2.5.62)
- **AfterDoctypeSystemKeyword** (§13.2.5.63)
- **BeforeDoctypeSystemId** (§13.2.5.64)
- **DoctypeSystemIdDoubleQuoted** (§13.2.5.65)
- **DoctypeSystemIdSingleQuoted** (§13.2.5.66)
- **AfterDoctypeSystemId** (§13.2.5.67)
- **BogusDoctype** (§13.2.5.68)

**目标**：`<!DOCTYPE html PUBLIC "-//W3C//DTD HTML 4.01//EN" "http://www.w3.org/TR/html4/strict.dtd">` 正确产出 `Token::Doctype`

### Step 1.5：替代内容模型（RCDATA/RAWTEXT/PLAINTEXT，8 个状态，~200 行代码）

- **RCDATA** (§13.2.5.2)
- **RCDATALessThanSign** (§13.2.5.9)
- **RCDATAEndTagOpen** (§13.2.5.10)
- **RCDATAEndTagName** (§13.2.5.11)
- **RAWTEXT** (§13.2.5.3)
- **RAWTEXTLessThanSign** (§13.2.5.12)
- **RAWTEXTEndTagOpen** (§13.2.5.13)
- **RAWTEXTEndTagName** (§13.2.5.14)
- **PLAINTEXT** (§13.2.5.5)

**目标**：`<title>` / `<textarea>` / `<style>` 内容正确解析

### Step 1.6：Script Data 状态（19 个状态，~400 行代码）

最复杂的一组，含双重转义 (§13.2.5.15–§13.2.5.31)。先仔细读规范理解 `<!-- <script> -->` 转义机制再动手。

**目标**：`<script>` 内容正确解析（含 `<!-- <script>` 和 `</script>` 嵌套处理）

### Step 1.7：Character Reference 状态（9 个状态，~300 行代码）

- **CharacterReference** (§13.2.5.72)
- **NamedCharacterReference** (§13.2.5.73)
- **AmbiguousAmpersand** (§13.2.5.74)
- **NumericCharacterReference** (§13.2.5.75)
- **HexCharacterReferenceStart** (§13.2.5.76)
- **DecimalCharacterReferenceStart** (§13.2.5.77)
- **HexCharacterReference** (§13.2.5.78)
- **DecimalCharacterReference** (§13.2.5.79)
- **NumericCharacterReferenceEnd** (§13.2.5.80)

**挑战**：Named character references 需要一张映射表（WHATWG 定义了 ~2200 个实体）。考虑：先实现一个最小子集，后续补全；或嵌入完整的 named character references JSON 作为编译期静态映射。

### Step 1.8：CDATA Section 状态（3 个状态，~80 行代码）

- **CDATASection** (§13.2.5.69)
- **CDATASectionBracket** (§13.2.5.70)
- **CDATASectionEnd** (§13.2.5.71)

**备注**：CDATA 仅在 SVG/ MathML foreign content 中出现，先实现基础，后续 tree construction 集成时验证。

### Step 1.9：Emit 当前 token 辅助逻辑

目前 `handle_data_state` 中 `<` 到 TagOpen、TagOpen 到 TagName 都返回 `None`（不 emit）。当 TagName 读到 `>` 时需要 emit 完整的 TagToken。需要确保 emit 逻辑一致：增加一个 `flush_current_tag()` 辅助函数，统一 `current_tag.take()` → `Some(Token::Tag(...))` 的模式。

类似地，Comment 和 Doctype 完成后也需要对应的 flush。

### Step 1.10：字符引用 return_state 机制

当前 `handle_data_state` 在遇到 `&` 时直接 `self.state = State::CharacterReference`，但没有保存 return state。按规范，字符引用被 consume 后需要回到调用它的状态（Data 或 attribute value 等）。需要：

- 在 `HtmlTokenizer` 增加 `return_state: Option<State>` 字段
- Data 状态和 attribute value 状态在进入 CharacterReference 前设置 `self.return_state = Some(self.state)`
- CharacterReference consume 完成后 `self.state = self.return_state.take().unwrap_or(State::Data)`

---

## Phase 2：Error 类型定义

当前很多地方有 `// TODO: record parse error (...)`。需要在 `error/mod.rs` 中定义 parse error 枚举。

- 参考 WHATWG §13.2 的 parse error 列表（~50+ 种错误）
- 在 Tokenizer trait 或 HtmlTokenizer 中增加 error 收集能力
- 选项 A：`HtmlTokenizer.errors: Vec<ParseError>` — 简单直接
- 选项 B：回调 `&mut dyn FnMut(ParseError)` — 更灵活，但违背 Simplicity 原则
- **建议选项 A**，等 tree construction 需要回调时再升级

**目标**：所有 `TODO: record parse error` 被替换为实际 error 记录，测试可断言错误列表。

---

## Phase 3：Tree Construction（§13.2.6）

Token 流完备后，开始消费 token 构建 DOM。

### Step 3.1：DOM 节点类型（`dom/mod.rs`）

- `Node` enum（Document / Element / Text / Comment / Doctype）
- `Element` 含 tag name、attributes、children
- 构建 `Document` 顶层节点

### Step 3.2：Tree Construction 核心

- **Insertion modes**（§13.2.6.4）：initial / before html / before head / in head / in body / ...
- **Stack of open elements**
- 每个 insertion mode 对应一个 handler，接收 token 并操作 DOM

**分期策略**：先实现 "in body" 和 "initial" 这两个最常用的 mode，保证 `<html><head></head><body><p>hello</p></body></html>` 能建出正确的 DOM 树。

---

## Phase 4：WPT 集成测试

架构师需要跑 WPT 语义比对。tokenizer 完成后应该能通过 WPT 的 tokenizer 测试套件。

- **WPT tokenizer tests**：`html/syntax/parsing/tokenizer/` — 以 JSON 格式定义输入和期望 token 序列
- 编写一个 test harness 读取 WPT JSON，对比 HtmlTokenizer 输出
- 这是 CI 门禁：零 regression

---

## Phase 5+：后续模块（远期规划）

以下按依赖顺序排列，tokenizer 完成后再细化：

| 模块 | 功能 | 依赖 | 复杂度 |
|------|------|------|--------|
| CSS Parser | CSS tokenizer + parser（CSS Syntax spec） | 无（独立模块） | 高 |
| URL Parser | WHATWG URL spec | 无（独立 crate） | 中 |
| DOM Core | Node 操作、遍历、事件 | Tree Construction 完成 | 高 |
| Layout Engine | 盒模型、流式布局 | DOM + CSS Parser | 极高 |
| Networking | HTTP/1.1, fetch spec | URL Parser | 中 |
| Encoding | 字符编码检测与转换 | 无（Encoding spec） | 中 |

---

## 实施原则（复述 CLAUDE.md）

1. **每组状态 → 独立 commit**，格式 `[tokenizer] implement X states, §13.2.5.N–M`
2. **先写测试 → 看它 fail → 实现 → 看它 pass**
3. **每个 commit 前 `cargo check` 零 warning + `cargo test` 全绿**
4. **不确定时先读 WHATWG 规范对应的状态转换，不猜**
5. **WPT 语义比对通过前不宣布"完成"**

---

## 验证方式

- **每步**：`cargo test -p muskitty-html5-parser --lib` 全绿
- **每步**：`cargo check` 零 warning
- **Phase 1 完结**：WPT tokenizer JSON 测试套件全过
- **Phase 3 完结**：简单 HTML 文档 → 正确 DOM 树
