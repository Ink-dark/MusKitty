# MusKitty 项目推进计划（恢复执行）

> 本计划承接已批准的 `muskitty-browser-roadmap.md`，聚焦从"当前实际状态"到"Phase 2 完成"的执行路径。
> 制定日期：2026-07-15 | 阶段：Layer 1 (HTML 解析层) Phase 1 收尾 → Phase 2

---

## 一、摘要

上一轮会话完成了 roadmap 制定与 Phase 1 (muskitty-dom crate) 的代码实现，但因上下文丢失，未完成验证与提交。本计划的目标是：

1. **收尾 Phase 1**：验证测试 → 修复依赖 → 按提交纪律提交
2. **清理未提交工作**：将混在一起的 tokenizer 修复与 DOM crate 拆分为独立 commit
3. **启动 Phase 2**：Tree Construction 骨架
4. **固化防跑偏机制**：每个阶段的准入/准出检查点

**不重新规划**——已有的 `muskitty-browser-roadmap.md` 仍是项目的 north star，本计划只是执行层面的细化。

---

## 二、当前状态分析

### 2.1 已提交（HEAD = `dbd20e6`）
- tokenizer 基础实现（80 个状态，命名实体表，CDATA 等）
- 最后提交：`[chore] add .zcode/ to gitignore`

### 2.2 未提交的工作（3 类混合）

| 类别 | 文件 | 说明 |
|------|------|------|
| **tokenizer 修复** | `crates/muskitty-html-parser/src/tokenizer/impls.rs` (+400/-238) | 上一轮 99.5% 通过率的 bug 修复，含 EndTagOpen/AttributeName/Comment 等多处规范合规修正 |
| **tokenizer 测试** | `crates/muskitty-html-parser/tests/html5lib_tokenizer.rs` (+6/-6) | test harness 的 self_closing 读取修正等 |
| **DOM crate** | `crates/muskitty-dom/**` (全部新增) | Phase 1 完整实现：node/element/text/comment/document/document_type/document_fragment/attribute/error/tree + 24 个测试 |
| **workspace 配置** | `Cargo.toml` (+1/-1) | 加入 muskitty-dom 到 members |
| **项目文档** | `docs/**`, `.trae/**` | 审查报告、skill 定义、roadmap |
| **调试残留** | `crates/muskitty-html-parser/test_failures.txt` | 应删除 |

### 2.3 Phase 1 实现状态
- `cargo check -p muskitty-dom`：✅ 已通过（零 warning）
- `cargo test -p muskitty-dom`：⏳ **未运行**（24 个测试待验证）
- `muskitty-html-parser` 依赖 `muskitty-dom`：❌ **未添加**（Cargo.toml `[dependencies]` 仍为空）
- Phase 1 commit：❌ **未提交**

### 2.4 关键问题
1. **多类工作混合未提交**：tokenizer 修复、DOM crate、文档混在一起，违反 CLAUDE.md 的"surgical changes + 每子任务一 commit"纪律
2. **Phase 1 未验证**：测试未跑，无法确认 DOM 实现正确性
3. **依赖未接通**：muskitty-html-parser 还不能使用 muskitty-dom 的类型

---

## 三、立即行动项（Phase 1 收尾）

### 步骤 1：验证 muskitty-dom 测试
```bash
cargo test -p muskitty-dom
```
- **预期**：24 个测试全绿
- **若失败**：根据失败原因修复，遵循"先 failing test → 修 → pass"流程
- **成功标准**：所有测试通过，零 warning

### 步骤 2：删除调试残留
- 删除 `crates/muskitty-html-parser/test_failures.txt`（调试产物，不应入库）

### 步骤 3：接通 muskitty-html-parser → muskitty-dom 依赖
- 编辑 `crates/muskitty-html-parser/Cargo.toml`：
  ```toml
  [dependencies]
  muskitty-dom = { path = "../muskitty-dom" }
  ```
- 验证：`cargo check -p muskitty-html-parser` 仍通过（依赖未使用，不破坏现状）

### 步骤 4：全 workspace 验证
```bash
cargo check        # 零 warning
cargo test         # 全绿（tokenizer 99.5% + dom 24 测试）
```

---

## 四、提交策略（拆分未提交工作）

按 CLAUDE.md 纪律，将混合的未提交工作拆为 4 个独立 commit，按依赖顺序提交：

### Commit 1：tokenizer 规范合规修复
```
[tokenizer] fix spec compliance bugs reaching 99.5% html5lib pass rate

Fixes EndTagOpen anything-else (bogus comment), AttributeName quote
handling, Comment state '<'/'!'/'-' appending, BeforeAttributeName
'=' branch, and EOF branch emissions per WHATWG §13.2.5.
```
**文件**：
- `crates/muskitty-html-parser/src/tokenizer/impls.rs`
- `crates/muskitty-html-parser/tests/html5lib_tokenizer.rs`

### Commit 2：DOM Core crate
```
[dom] add muskitty-dom crate with DOM Core types and tree operations

Implements Node/Element/Text/Comment/Document/DocumentType/
DocumentFragment per DOM Living Standard §4-7, with append/insert/
remove/replace tree operations (§4.2.6) and read-only traversal API.
24 unit tests covering construction, mutation, traversal, errors.
```
**文件**：
- `Cargo.toml`（workspace members）
- `crates/muskitty-dom/Cargo.toml`
- `crates/muskitty-dom/src/**`（全部源文件）
- `crates/muskitty-dom/tests/node.rs`
- `crates/muskitty-html-parser/Cargo.toml`（添加依赖）

### Commit 3：项目文档与 skill
```
[docs] add tokenizer spec review report and adversarial review skill

Adds the whatwg-spec-adversarial-review skill definition and the
tokenizer spec review report documenting 12 identified spec violations.
```
**文件**：
- `docs/skill/whatwg-spec-adversarial-review.md`
- `docs/tokenizer-spec-review.md`
- `.trae/skills/whatwg-spec-adversarial-review/SKILL.md`（如存在）

### Commit 4：项目 roadmap
```
[docs] add project browser roadmap document

Adds the layered project advancement plan (DOM→CSS→Layout→Renderer)
with anti-drift mechanisms and phase admission thresholds.
```
**文件**：
- `.trae/documents/muskitty-browser-roadmap.md`
- `.trae/documents/muskitty-project-resume-plan.md`（本文件）

---

## 五、Phase 2：Tree Construction 骨架

完成 Phase 1 提交后，按 roadmap Phase 2 执行。以下是执行细化：

### 5.1 目标
建立 tree construction 调度框架，能消费 token 流并构造最小 DOM 树（空字符串 → 空 Document）。

### 5.2 规范依据
WHATWG HTML §13.2.6 Tree construction

### 5.3 具体改动（按实现顺序）

#### 5.3.1 InsertionMode 枚举
- 文件：`crates/muskitty-html-parser/src/parser/insertion_mode.rs`
- 内容：按 §13.2.6.1 定义全部 insertion mode 变体
- 验证：编译通过

#### 5.3.2 Parser 主体
- 文件：`crates/muskitty-html-parser/src/parser/mod.rs`（替换占位）
- `HtmlTreeConstructor` 结构体字段：
  - `document: Rc<RefCell<Node>>`（输出根，来自 muskitty-dom）
  - `open_elements: Vec<Rc<RefCell<Node>>>`
  - `active_formatting_elements: Vec<...>`
  - `insertion_mode: InsertionMode`
  - `original_insertion_mode: Option<InsertionMode>`
  - `head_element: Option<Rc<RefCell<Node>>>`
  - `form_element: Option<Rc<RefCell<Node>>>`
  - `foster_parenting: bool`
  - `frameset_ok: bool`
  - `scripting_flag: bool`（默认 false）
- `step(&mut self, token: Token)`：按 insertion_mode 分发

#### 5.3.3 模式分发器
- 文件：`crates/muskitty-html-parser/src/parser/dispatch.rs`
- `match insertion_mode { ... }` 分发到各 handler
- 每个模式 handler 先建空骨架（`match token { _ => todo!() }`）

#### 5.3.4 辅助算法骨架
- 文件：`crates/muskitty-html-parser/src/parser/helpers.rs`
- 签名 + 最小实现：
  - `insert_element(name, attrs)` — 创建元素并压栈
  - `insert_character(c)` — 字符插入
  - `insert_comment(data)`
  - `reconstruct_active_formatting_elements()` — 先空实现

#### 5.3.5 Error 模块
- 文件：`crates/muskitty-html-parser/src/error/mod.rs`（替换占位）
- `ParseError` 枚举（按 §13.2.6 定义的 parse error 类型）
- parser 增加 `errors: Vec<ParseError>` 字段

#### 5.3.6 顶层入口
- 文件：`crates/muskitty-html-parser/src/lib.rs`
- 新增 `pub fn parse(input: &str) -> Rc<RefCell<Node>>`
  - 构造 tokenizer + parser
  - 跑完整 token 流
  - 返回 Document
- 删除 `src/dom/mod.rs` 占位（DOM 类型已由 muskitty-dom 提供）

### 5.4 Phase 2 验证标准
- `cargo check` 零 warning
- `cargo test` 既有测试全绿（tokenizer 不受影响）
- `parse("")` 返回空 Document（骨架能跑通）
- 单元测试：`parse("<!DOCTYPE html>")` 产出含 DocumentType 的 Document

### 5.5 Phase 2 提交
```
[parser] add tree construction skeleton with insertion mode dispatcher

Implements HtmlTreeConstructor with open elements stack, active
formatting elements list, and insertion mode state machine per
WHATWG §13.2.6. Handler bodies are scaffolded; actual token handling
comes in Phase 3 batches.
```

---

## 六、Phase 3+ 路线（引用 roadmap）

Phase 2 完成后，按 `muskitty-browser-roadmap.md` 第五节执行：

| Phase | 内容 | 参考章节 |
|-------|------|----------|
| 3.1 | 前置模式 (Initial/BeforeHtml/BeforeHead/InHead/AfterHead) | §13.2.6.2-5 |
| 3.2 | InBody 核心 | §13.2.6.4 |
| 3.3 | InBody 进阶（格式化元素） | §13.2.6.4 |
| 3.4 | 表格相关 | §13.2.6.7-13 |
| 3.5 | Select/Template/Frameset | §13.2.6.14-18 |
| 3.6 | Text 模式 | §13.2.6.5 |
| 4 | Adoption Agency + Foster Parenting | §13.2.6.4.7, §13.2.6.3 |
| 5 | html5lib tree construction 测试集成 | — |
| 6 | DOM 完整 API 扩展 | DOM LS |

每个 Phase 内部：一个批次 = 一个 commit + 对应测试。

---

## 七、防跑偏机制

### 7.1 层间准入门槛（硬性，来自 roadmap §8.1）
进入下一 Phase 前**必须**满足：
1. 当前 Phase 标准测试通过率 ≥80%（Phase 1/2 用单元测试，Phase 3+ 用 html5lib）
2. `cargo test` 全绿，`cargo check` 零 warning
3. 公共 API 有 doc comment 引用规范条款
4. 提交历史清晰（每子任务一 commit，`[module] what + why` 格式）

### 7.2 阶段内防跑偏
- **每 Phase 完成后核对**：对照 roadmap 检查点，不提前跳到下一 Phase
- **规范模糊时**：强制读 WHATWG 原文，不猜
- **抵抗过早抽象**：DOM 类型先硬编码字段，有真实使用场景再抽象 trait
- **抵抗 Kitchen Sink**：只实现规范定义的 API，不加"方便"方法
- **每 commit 后跑完整 `cargo test`**：确保无回退

### 7.3 遗留问题处理纪律
- tokenizer 遗留修复只在 Phase 3 触发时进行（边做 tree construction 边修）
- xmlViolation 类问题明确标记为"非本项目范围"
- 任何偏离计划的"顺手修"都需先评估是否在当前 Phase 范围内

### 7.4 规范优先级（来自 CLAUDE.md）
**WHATWG > WPT > Chromium 源码**
- 实现前读规范对应章节
- 不确定时停下来问，不猜
- 测试与规范冲突时，除非能用规范章节号证明测试错误，否则改实现不改测试

### 7.5 本计划专属检查点

| 检查点 | 时机 | 通过条件 |
|--------|------|----------|
| CP-1 | Phase 1 验证 | `cargo test -p muskitty-dom` 全绿 |
| CP-2 | Phase 1 提交 | 4 个 commit 干净分离，无混合关注点 |
| CP-3 | Phase 2 骨架 | `parse("")` 返回空 Document |
| CP-4 | Phase 2 提交 | `[parser]` commit 完成 |
| CP-5 | Phase 3.1 | 能解析完整 `<!DOCTYPE html><html>...` |

---

## 八、假设与决策

### 假设
1. `cargo test -p muskitty-dom` 的 24 个测试能通过（cargo check 已过，逻辑应正确）
2. `test_failures.txt` 是调试残留，可安全删除
3. `.trae/` 目录应入库（包含 skill 定义和 roadmap，是项目配置的一部分）
4. tokenizer 的未提交修复（impls.rs）是上一轮 99.5% 通过率的工作成果，可整批提交

### 决策
1. **提交顺序**：tokenizer 修复 → DOM crate → 文档 → roadmap（按依赖关系）
2. **DOM 占位处理**：删除 `muskitty-html-parser/src/dom/mod.rs`，从 `muskitty-dom` re-export
3. **Phase 2 粒度**：骨架一个 commit，不拆分（各 handler 空骨架不算独立子任务）

---

## 九、验证步骤（Phase 1 收尾）

1. `cargo test -p muskitty-dom` → 全绿
2. 删除 `test_failures.txt`
3. 编辑 `muskitty-html-parser/Cargo.toml` 添加依赖
4. `cargo check`（整个 workspace）→ 零 warning
5. `cargo test`（整个 workspace）→ 全绿
6. 按"提交策略"第四节执行 4 个 commit
7. `git log --oneline -6` 确认提交历史清晰
8. 进入 Phase 2

---

## 十、执行顺序总结

```
[当前] 验证 DOM 测试
   ↓
[当前] 清理残留 + 接通依赖
   ↓
[当前] 全 workspace cargo check + cargo test
   ↓
[当前] 提交 Commit 1 (tokenizer 修复)
   ↓
[当前] 提交 Commit 2 (DOM crate)
   ↓
[当前] 提交 Commit 3 (文档)
   ↓
[当前] 提交 Commit 4 (roadmap)
   ↓
[Phase 2] Tree Construction 骨架
   ↓
[Phase 3.1-3.6] Insertion Mode 分批实现
   ↓
[Phase 4] 关键算法
   ↓
[Phase 5] html5lib tree construction 测试
   ↓
[Phase 6] DOM 完整 API
```
