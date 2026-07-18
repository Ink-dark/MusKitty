# MusKitty 浏览器项目推进路线图

> 本文档为 MusKitty 项目的分层推进计划，目标是避免跑偏、确保每一层做扎实后再进入下一层。
> 制定日期：2026-07-15 | 当前状态：tokenizer 99.5% (7004/7036 html5lib)

---

## 一、摘要

MusKitty 是从零用 Rust 重写的浏览器核心模块集合。当前 HTML tokenizer 已达 99.5% html5lib 通过率，剩余 32 个失败多为属性值字符引用等架构性问题，不阻塞下游。

本计划基于三项用户决策制定：
1. **推进策略**：按浏览器分层逐层做实（DOM → CSS → Layout → Renderer），每层独立通过该层测试后再进下一层
2. **DOM 深度**：完整 DOM API（一步到位，避免后续重构）
3. **遗留修复**：边做 tree construction 边修 tokenizer 遗留问题

本计划聚焦**当前阶段（HTML 解析层 + DOM 层）的详细执行路径**，并对后续层给出高层路线。

---

## 二、当前状态分析

### 已完成
- `crates/muskitty-html5-parser/src/tokenizer/`：完整实现 WHATWG §13.2.5 状态机，80 个状态
  - `types.rs`：Token/TagToken/DoctypeToken/State 类型定义
  - `trait_def.rs`：Tokenizer trait（支持 reentrancy：set_state/reset）
  - `impls.rs`：~3900 行状态机实现，html5lib 通过率 99.5%
  - `entities.rs`：命名实体表
- `tests/html5lib_tokenizer.rs` + `tests/data/tokenizer/*.test`：测试套件与 fixture

### 占位待实现
- `crates/muskitty-html5-parser/src/dom/mod.rs`：仅一行 doc comment
- `crates/muskitty-html5-parser/src/parser/mod.rs`：仅一行 doc comment
- `crates/muskitty-html5-parser/src/error/mod.rs`：仅一行 doc comment

### 遗留问题（32 个 tokenizer 失败）
- 14 个：属性值 `&` 字符引用（需重构 CharacterReference emit 路径，区分 return_state 上下文）
- 6 个：script data 双转义尾部边界
- 3 个：xmlViolation（infoset 强制转换，非 tokenizer 行为，建议从基线排除）
- 9 个：实体边界、CDATA NUL、RAWTEXT EOF 等零散场景

### 架构现状
- Workspace 单成员 `muskitty-html5-parser`，无依赖（dev-dep: serde_json）
- Cargo.toml 已预留 5 个未来 crate：dom/css/network/layout/renderer
- CLAUDE.md 硬约束：Rust stable、零 unsafe、零 C/C++ 依赖、每模块独立 crate、≥80% 测试覆盖、公共 API 必须有 doc comment 引用规范条款

---

## 三、关键决策

### 决策 1：DOM 作为独立 crate
**决定**：新建 `crates/muskitty-dom/` 作为独立 crate，`muskitty-html5-parser` 依赖它。
**理由**：CLAUDE.md 明确"每个模块独立 crate"，Cargo.toml 已预留。DOM 类型将被 layout/renderer 等下游层复用，必须与 parser 解耦。
**影响**：`muskitty-html5-parser/src/dom/mod.rs` 的占位将被删除，改为从 `muskitty-dom` re-export 或直接依赖。

### 决策 2：DOM 完整 API 分子阶段实现
**决定**："完整 DOM API"作为 DOM 层的总目标，但内部分子阶段推进，每个子阶段独立测试。
**理由**：DOM Living Standard 极其庞大（Core + Events + Selectors + Style + HTML-specific），一次性实现无法测试。tree construction 只需要 DOM Core 子集。
**子阶段**：
- DOM Core（Node/Element/Text/Comment/Document/DocumentType/DocumentFragment/Attr + 树操作 API）
- DOM Events（EventTarget trait + Event/CustomEvent 基础）
- DOM Selectors（querySelector，简单选择器先行，复杂选择器依赖 muskitty-css）
- DOM Style（innerHTML/outerHTML、classList、dataset 等）
- HTML-specific DOM（HTMLElement 及子类）推迟到 CSS 层之后

### 决策 3：tree construction 与 DOM Core 交织推进
**决定**：先实现 DOM Core 子集（够 tree construction 用），然后实现 tree construction，最后补齐 DOM 剩余 API。
**理由**：tree construction 是 DOM Core API 的主要消费者和验证场景；没有实际建树，DOM API 测试苍白。

### 决策 4：html5lib tree construction 测试作为 ground truth
**决定**：下载 html5lib tree construction 测试 fixture，作为 tree construction 层的 ground truth，方法论与 tokenizer 阶段一致。
**理由**：延续项目既有方法论（WHATWG > WPT > Chromium，测试为真值）。

### 决策 5：xmlViolation 从基线排除
**决定**：将 `xmlViolation.test` 从 tokenizer 通过率基线排除，并文档化原因。
**理由**：xmlViolation 测试的是 infoset 强制转换（如 U+FFFF 替换、U+000C→空格、注释双连字符折叠），非 WHATWG §13.2.5 tokenizer 行为。CLAUDE.md 明确 WHATWG 是 ground truth。

---

## 四、总体路线图（分层全景）

```
Layer 1: HTML 解析层（当前重点）
  ├─ DOM Core 类型 (muskitty-dom crate)
  ├─ Tree Construction (muskitty-html5-parser/parser)
  ├─ Insertion Modes × 13
  ├─ 关键算法 (adoption agency, foster parenting)
  ├─ html5lib tree construction 测试通过
  └─ DOM 完整 API 扩展 (Events/Selectors/Style)

Layer 2: CSS 解析层 (muskitty-css crate)
  ├─ CSS 语法 tokenizer (CSS Syntax Module §5)
  ├─ CSS 解析器 (CSS Values, Selectors)
  ├─ 样式表数据结构
  └─ Cascade + Computed values

Layer 3: Layout 层 (muskitty-layout crate)
  ├─ Box tree 构建
  ├─ Formatting context (Block/Inline/Flex/Grid)
  ├─ 布局算法
  └─ 文本排版

Layer 4: Renderer 层 (muskitty-renderer crate)
  ├─ 绘制命令生成
  ├─ 文本渲染
  └─ 图像解码

Layer 5: Network 层 (muskitty-network crate，可与上述并行)
  ├─ HTTP/1.1 + HTTP/2
  ├─ TLS
  └─ URL 解析
```

**层间依赖**：Layer N 依赖 Layer N-1。Layer 5 (Network) 无下游依赖，可与 Layer 2-4 并行。

**进入下一层的门槛**：当前层通过其标准测试套件（html5lib / WPT CSS / WPT Layout）且 ≥80% 覆盖率。

---

## 五、本计划聚焦：Layer 1 (HTML 解析层) 详细执行路径

### Phase 1：muskitty-dom crate 骨架 + DOM Core 类型

**目标**：建立独立 DOM crate，实现 DOM Core 核心类型与树操作 API，为 tree construction 提供基础。

**规范依据**：DOM Living Standard §4 (Nodes), §5 (Documents), §6 (Elements), §7 (Text/Comment/ProcessingInstruction)

**具体改动**：

1. **新建 crate 结构** `crates/muskitty-dom/`
   - `Cargo.toml`：`[package] name = "muskitty-dom"`，无依赖
   - `src/lib.rs`：模块导出
   - 加入 workspace `members`

2. **核心类型定义** `crates/muskitty-dom/src/node.rs`
   - `Node` 结构体：`node_type: NodeType`、`node_name: String`、`owner_document: Option<Rc<RefCell<Document>>>`、`parent_node`/`child_nodes`（用 `Rc<Weak<RefCell<Node>>>` 父指针 + `Vec<Rc<RefCell<Node>>>` 子节点）
   - `NodeType` 枚举（u8 常量，匹配 DOM `Node.nodeType`：ELEMENT_NODE=1...DOCUMENT_NODE=9）
   - 树所有权模型：`Rc<RefCell<Node>>` 实现共享所有权 + 可变性（Rust 无 GC 的标准 DOM 建模方式）

3. **具体节点类型** `crates/muskitty-dom/src/element.rs` / `text.rs` / `comment.rs` / `document.rs` / `document_type.rs` / `document_fragment.rs`
   - `Element`：`tag_name`、`attributes: Vec<Attribute>`、`namespace`（HTML/SVG/MathML）、`children`
   - `Text`：`data: String`
   - `Comment`：`data: String`
   - `Document`：`doctype`、`document_element`、`implementation`
   - `DocumentType`：`name`、`public_id`、`system_id`
   - `DocumentFragment`
   - `Attribute`：`namespace`、`local_name`、`value`

4. **树操作 API** `crates/muskitty-dom/src/node.rs`
   - 实现按 DOM Living Standard 规范的方法：
     - `append_child(child)` — §4.2.6
     - `insert_before(new, ref)` — §4.2.6
     - `remove_child(child)` — §4.2.6
     - `replace_child(new, old)` — §4.2.6
   - 内部 `insert(node, parent, before)` 算法严格按规范（处理 pre-insert validation、mutation observers 钩子预留）

5. **只读遍历 API**
   - `first_child`/`last_child`/`previous_sibling`/`next_sibling`/`parent_element`
   - `text_content` getter/setter（按规范聚合后代文本）
   - `descendants` 迭代器（深度优先）

6. **单元测试** `crates/muskitty-dom/tests/node.rs`
   - 建树、插入、删除、替换的规范化测试
   - 覆盖率目标 ≥80%

7. **更新 `muskitty-html5-parser`**
   - `Cargo.toml` 增加 `muskitty-dom = { path = "../muskitty-dom" }`
   - 删除 `src/dom/mod.rs` 占位，从 `muskitty-dom` re-export 或直接引用

**验证**：
- `cargo check -p muskitty-dom` 零 warning
- `cargo test -p muskitty-dom` 全绿，覆盖率 ≥80%
- `cargo check -p muskitty-html5-parser` 仍通过（依赖未使用，不破坏现状）

---

### Phase 2：Tree Construction 骨架

**目标**：建立 tree construction 的调度框架，能消费 token 流并构造最小 DOM 树。

**规范依据**：WHATWG HTML §13.2.6 Tree construction

**具体改动**：

1. **Parser 类型** `crates/muskitty-html5-parser/src/parser/mod.rs`
   - `HtmlTreeConstructor` 结构体：
     - `tokenizer: HtmlTokenizer`（持有，可暂停/恢复）
     - `document: Rc<RefCell<Document>>`（输出根）
     - `open_elements: Vec<Rc<RefCell<Node>>>`（开放元素栈，§13.2.6.3）
     - `active_formatting_elements: Vec<...>`（活动格式化元素列表）
     - `insertion_mode: InsertionMode`
     - `original_insertion_mode: Option<InsertionMode>`（text 状态用）
     - `head_element: Option<Rc<RefCell<Node>>>`
     - `form_element: Option<Rc<RefCell<Node>>>`
     - `foster_parenting: bool`
     - `frameset_ok: bool`
     - `scripting_flag: bool`（默认 false）
   - `step(&mut self)`：取一个 token，按 insertion_mode 分发到 handler
   - `consume_token(token)`：主入口，由 tokenizer next_token 喂入

2. **InsertionMode 枚举** `crates/muskitty-html5-parser/src/parser/insertion_mode.rs`
   - 13 个变体：Initial / BeforeHtml / BeforeHead / InHead / InHeadNoscript / AfterHead / InBody / Text / InTable / InTableText / InCaption / InColumnGroup / InTableBody / InRow / InCell / InSelect / InSelectInTable / InTemplate / AfterBody / InFrameset / AfterFrameset / AfterAfterBody / AfterAfterFrameset
   - 实际变体数按 §13.2.6.1 列表为准

3. **模式分发器** `crates/muskitty-html5-parser/src/parser/dispatch.rs`
   - `match insertion_mode { ... }` 分发到各 handler 函数
   - 每个 handler 接收 `&mut HtmlTreeConstructor` 和 `&Token`

4. **辅助算法骨架** `crates/muskitty-html5-parser/src/parser/helpers.rs`
   - `insert_element(name, attrs)` — §13.2.6.2 创建元素并压栈
   - `insert_character(c)` — 字符插入（区分 foster parenting）
   - `insert_comment(data)` — 注释插入
   - `reconstruct_active_formatting_elements()` — §13.2.6.4
   - `adjust_foreign_attributes()` — §13.2.6.5
   - 这些先建签名 + 最小实现，后续 Phase 3/4 补全

5. **Error 收集** `crates/muskitty-html5-parser/src/error/mod.rs`
   - `ParseError` 枚举（按 §13.2.6 定义的 parse error 类型）
   - `errors: Vec<ParseError>` 字段加入 parser

6. **顶层入口** `crates/muskitty-html5-parser/src/lib.rs`
   - `parse(input: &str) -> Rc<RefCell<Document>>`：构造 tokenizer + parser，跑完整 token 流，返回 Document

**验证**：
- `cargo check` 零 warning
- `cargo test` 既有测试全绿（tokenizer 不受影响）
- 能成功解析空字符串 `""` → 返回空 Document（骨架能跑通）

---

### Phase 3：Insertion Mode 实现（分批）

**目标**：逐个实现 13+ 个 insertion mode 的 token 处理逻辑。这是工作量最大的阶段，内部再分批。

**规范依据**：WHATWG HTML §13.2.6.2 ~ §13.2.6.22（每个模式一节）

**分批策略**（每批一个 commit，附单元测试）：

**批次 3.1 — 前置模式**（§13.2.6.2 ~ §13.2.6.5）
- Initial / BeforeHtml / BeforeHead / InHead / AfterHead
- 处理 DOCTYPE、html/head/body 创建、基础 head 元素（title/meta/link/style）
- 验证：能解析 `<!DOCTYPE html><html><head><title>X</title></head><body></body></html>` 产出正确 DOM 结构

**批次 3.2 — InBody 核心**（§13.2.6.4，最复杂）
- 段落、标题、列表、div/span 等基础元素
- 字符插入与空白处理
- start tag / end tag 的 implied end tag 逻辑
- 验证：能解析简单文档并产出预期 DOM 树

**批次 3.3 — InBody 进阶**
- 格式化元素（b/i/em/strong/code 等）+ active formatting elements 重建
- 列表嵌套、段落隐式结束
- 验证：嵌套格式化标签的 DOM 结构正确

**批次 3.4 — 表格相关**（§13.2.6.7 ~ §13.2.6.13）
- InTable / InTableText / InCaption / InColumnGroup / InTableBody / InRow / InCell
- foster parenting 骨架（完整算法在 Phase 4）
- 验证：基础表格结构

**批次 3.5 — Select / Template / Frameset**（§13.2.6.14 ~ §13.2.6.18）
- InSelect / InSelectInTable / InTemplate / AfterBody / InFrameset / AfterFrameset / AfterAfterBody / AfterAfterFrameset
- 验证：select/template 结构

**批次 3.6 — Text 模式**（§13.2.6.5）
- text/script/textarea/title 内容收集
- 验证：script/textarea 内容正确

**每个批次的验证标准**：
- 对应该批次的 html5lib tree construction 子集测试通过
- `cargo check` 零 warning，`cargo test` 全绿

---

### Phase 4：关键算法

**目标**：实现 tree construction 中两个最复杂的算法，它们影响大量边界场景。

**规范依据**：
- Adoption Agency Algorithm：§13.2.6.4.7（约 100 行规范）
- Foster Parenting：§13.2.6.3（表格上下文中的元素插入）

**具体改动**：

1. **Adoption Agency Algorithm** `crates/muskitty-html5-parser/src/parser/adoption.rs`
   - 实现 §13.2.6.4.7 完整步骤
   - 处理格式化标签的 Noah's Ark case（多个相同格式化元素）
   - 这是 tokenizer→tree construction 阶段最容易出错的算法，需要重点测试

2. **Foster Parenting 完整实现** `crates/muskitty-html5-parser/src/parser/helpers.rs`
   - `insert_element` 在 foster parenting 模式下：找到最近 table/row，将元素插入到 table 的父节点中、table 之前的位置
   - 修复 Phase 3 表格批次的骨架实现

**验证**：
- html5lib tree construction 中涉及 `<b><p></b></p>`、`<table><a>` 等典型用例通过
- adoption agency 的标准 reproducer 全部通过

---

### Phase 5：html5lib Tree Construction 测试集成

**目标**：将 html5lib tree construction 测试套件作为该层 ground truth，跑通基线。

**具体改动**：

1. **下载 fixture** `crates/muskitty-html5-parser/tests/data/tree-construction/`
   - 从 https://github.com/html5lib/html5lib-tests/tree/master/tree-construction 下载
   - 放入测试数据目录

2. **测试 harness** `crates/muskitty-html5-parser/tests/html5lib_tree_construction.rs`
   - 解析 `.test` 文件（JSON 格式：input / output / errors / document-fragment / scripting-enabled）
   - output 格式是 DOM 树的序列化文本，需要实现 DOM 序列化器 `to_string()` 用于比对
   - 支持 `scripting-enabled` flag（影响 template 等处理）

3. **DOM 序列化器** `crates/muskitty-dom/src/serialization.rs`
   - 按 html5lib 输出格式序列化 DOM（`<html>`、`| ` 前缀缩进、`"` 属性引号、属性顺序）
   - 这是测试比对的基石

4. **基线建立**
   - 跑全套，记录通过率
   - 按失败模式分类，与 tokenizer 阶段类似的 gap report

**验证**：
- 测试 harness 能跑通完整套件（不 panic）
- 通过率 ≥80%（剩余失败多为边界场景，Phase 6/后续迭代修）

---

### Phase 6：DOM 完整 API 扩展

**目标**：在 DOM Core 基础上补齐 DOM Living Standard 的其他 API 子集，使 DOM 层"做实"。

**子阶段**（每个独立 commit + 测试）：

1. **DOM Events 基础** `crates/muskitty-dom/src/event.rs`
   - `EventTarget` trait（addEventListener/dispatchEvent，骨架，事件循环后续层做）
   - `Event` / `CustomEvent` 结构体
   - `Node` 实现 `EventTarget`

2. **DOM Selectors 基础** `crates/muskitty-dom/src/selector.rs`
   - `querySelector` / `querySelectorAll`
   - 简单选择器（类型/类/ID/属性）先行
   - 复杂选择器（组合器、伪类）依赖 muskitty-css，标记为 todo

3. **DOM Style 基础** `crates/muskitty-dom/src/style.rs`
   - `Element::class_list()` / `Element::id()` / `Element::dataset()`
   - `innerHTML` / `outerHTML` getter/setter（setter 触发解析，依赖 parser）

4. **NodeList / HTMLCollection** `crates/muskitty-dom/src/collection.rs`
   - 按规范的 lazy/active 语义（active collection 反映树变化）

**验证**：
- 每个子阶段 ≥80% 覆盖率
- `cargo test -p muskitty-dom` 全绿

---

## 六、Tokenizer 遗留修复（穿插进行）

按用户决策，边做 tree construction 边修。优先级：

| 优先级 | 问题 | 触发时机 |
|--------|------|----------|
| 高 | 属性值 `&` 字符引用（14 个） | Phase 3.2 InBody 处理属性时，若测试失败根因指向此处 |
| 中 | script data 边界（6 个） | Phase 3.6 Text 模式时 |
| 中 | CDATA NUL（1 个） | Phase 3 表格/foreign content 时 |
| 低 | RAWTEXT EOF（1 个） | Phase 3 顺带 |
| 低 | 实体边界（2 个） | Phase 3 顺带 |
| 排除 | xmlViolation（3 个） | 本阶段不修，从基线排除 |

**修复方法**：按既有方法论——先复现 failing test，引用 WHATWG 章节号，最小改动，验证不回退。

---

## 七、后续层高层路线（不展开细节）

### Layer 2: CSS 解析层（muskitty-css）
- 子阶段：语法 tokenizer → 选择器解析 → 值解析 → 样式表数据结构 → cascade
- ground truth：WPT CSS 测试套件
- 入场门槛：Layer 1 通过率 ≥80% + DOM API 完整

### Layer 3: Layout 层（muskitty-layout）
- 子阶段：Box tree → Block/Inline formatting → Flex/Grid → 文本排版
- ground truth：WPT reftests
- 入场门槛：Layer 2 通过 cascade + computed values 测试

### Layer 4: Renderer 层（muskitty-renderer）
- 子阶段：绘制命令 → 文本渲染 → 图像解码
- 入场门槛：Layer 3 能产出布局盒

### Layer 5: Network 层（muskitty-network）
- 可与 Layer 2-4 并行
- 子阶段：URL 解析 → HTTP/1.1 → TLS → HTTP/2

---

## 八、防跑偏机制

### 8.1 层间准入门槛（硬性）
每进入下一层前必须满足：
1. 当前层标准测试套件通过率 ≥80%
2. `cargo test` 全绿，`cargo check` 零 warning
3. 测试覆盖率 ≥80%
4. 公共 API 有 doc comment 引用规范条款
5. 提交历史清晰（每子任务一 commit，message 符合 `[module] what + why` 格式）

### 8.2 阶段内防跑偏
- 每个 Phase 完成后，对照本计划检查点核对，不提前跳到下一 Phase
- 遇到规范模糊处，强制读 WHATWG 原文 + Chromium 参考实现，不猜
- 抵抗过早抽象：DOM 类型先硬编码字段，等有真实使用场景再抽象 trait
- 抵抗 Kitchen Sink：DOM API 只实现规范定义的，不加"方便"方法
- 每个 commit 后跑完整 `cargo test`，确保无回退

### 8.3 遗留问题处理纪律
- 任何偏离本计划的"顺手修"都需先评估是否在当前层范围内
- tokenizer 遗留修复只在 Phase 3 触发时进行，不专门停下 tree construction 去修
- xmlViolation 类问题明确标记为"非本项目范围"，不投入时间

### 8.4 规范优先级（来自 CLAUDE.md）
**WHATWG > WPT > Chromium 源码**
- 实现前读规范对应章节
- 不确定时停下来问，不猜
- 测试与规范冲突时，除非能用规范章节号证明测试错误，否则改实现不改测试

---

## 九、验证步骤（本计划整体）

完成 Layer 1 后，整体验证：
1. `cargo check`（整个 workspace）零 warning
2. `cargo test`（整个 workspace）全绿
3. `cargo test --test html5lib_tokenizer` 通过率 ≥99.5%（不回退）
4. `cargo test --test html5lib_tree_construction` 通过率 ≥80%
5. `cargo test -p muskitty-dom` 全绿，覆盖率 ≥80%
6. 顶层 `parse("<!DOCTYPE html><html>...</html>")` 返回结构正确的 Document
7. `git log` 提交历史清晰，每个 commit 独立可回滚

---

## 十、当前立即行动项

完成本计划评审后，立即开始 **Phase 1：muskitty-dom crate 骨架 + DOM Core 类型**。

第一步：
1. 创建 `crates/muskitty-dom/Cargo.toml`
2. 加入 workspace members
3. 创建 `src/lib.rs` + `src/node.rs` 骨架
4. 实现 `Node` 核心类型 + `append_child`/`insert_before`/`remove_child` + 单元测试
5. `cargo check -p muskitty-dom` + `cargo test -p muskitty-dom` 通过后 commit

每一步严格遵循 CLAUDE.md 的 Verification Flow。
