# MusKitty — Progress Dashboard

> 最后更新: 2026-07-17 | 基于 git commit `5f767da`
>
> Phase 1 (HTML 解析层) 收尾：html5lib tree construction **84.8% → 100%**。

## 总览

| 模块 | 状态 | 规范覆盖 | 测试通过率 | 备注 |
|------|------|---------|-----------|------|
| **Tokenizer** | ✅ 完成 | §13.2.5.1–§13.2.5.80 (80/80) | 99.8% (7022/7036) | 14 失败：11 个 `<?...>` 处理指令边界 + 3 个 xmlViolation（infoset 强制转换，规范范围外，已从基线排除） |
| **Tree Construction** | ✅ 完成 | §13.2.6 (全 insertion mode + 关键算法) | 100% (1716/1716, 204 skipped) | skipped = fragment 解析 + script-on 模式（未实现） |
| **DOM Core** | ✅ 完成 | DOM Living Standard §4–§7 | 单元测试全绿 | `muskitty-dom` 独立 crate |
| **HTML Parser (整体)** | ✅ Phase 1 收尾 | — | — | 作为独立 crate 已可用 |
| DOM 完整 API (Events/Selectors/Style) | ⬜ 推迟 | — | — | 推迟到 Phase 2 (CSS) 之后 |
| muskitty-css | ⬜ Phase 2 | — | — | 下一个重点 |
| muskitty-network / muskitty-layout / muskitty-renderer | ⬜ 远期 | — | — | roadmap Layer 3–5 |

## Phase 1 (HTML 解析层) — 已收尾

按 [muskitty-browser-roadmap.md](.trae/documents/muskitty-browser-roadmap.md) 的 6 个 Phase 全部完成（Phase 6 推迟）：

### Phase 1 — muskitty-dom crate + DOM Core 类型 ✅

- 新建 `crates/muskitty-dom/`，无外部依赖
- `Node` / `NodeType` / `NodeKind`：`Rc<RefCell<Node>>` 共享所有权模型
- 节点类型：`Element` / `Text` / `Comment` / `Document` / `DocumentType` / `DocumentFragment` / `ProcessingInstruction`
- `Attribute` / `Namespace` (HTML/SVG/MathML)
- 树操作 API：`append_child` / `insert_before` / `remove_child` / `replace_child` / `set_text_content`
- 只读遍历：`first_child` / `last_child` / `previous_sibling` / `next_sibling` / `parent_element` / `Descendants` 迭代器
- 单元测试：`crates/muskitty-dom/tests/node.rs`

### Phase 2 — Tree Construction 骨架 ✅

- `HtmlTreeConstructor` 结构体：`open_elements` / `active_formatting_elements` / `insertion_mode` / `head_element` / `form_element` / `foster_parenting` / `frameset_ok` / `scripting_flag` / `template_insertion_modes`
- `InsertionMode` 枚举（全 23 个模式）
- `dispatch()` 主分发器：foreign content 优先，然后按 insertion mode
- 顶层入口 `parse(input: &str) -> Rc<RefCell<Node>>`
- `ParseError` 枚举与错误收集

### Phase 3 — Insertion Mode 实现 ✅

分批实现，每批独立提交（P3-a / P3-b / P3-c / P3-d 系列）：

| 批次 | 内容 | 规范 |
|------|------|------|
| P3.1 | Initial / BeforeHtml / BeforeHead / InHead / AfterHead | §13.2.6.2–§13.2.6.5 |
| P3.2 | InBody 核心（段落、标题、列表、div/span、字符插入、implied end tags） | §13.2.6.4 |
| P3.3 | InBody 进阶（active formatting elements 重建、格式化标签、列表嵌套） | §13.2.6.4 |
| P3.4 | InTable / InTableText / InCaption / InColumnGroup / InTableBody / InRow / InCell | §13.2.6.7–§13.2.6.13 |
| P3.5 | InSelect / InSelectInTable / InTemplate / AfterBody / InFrameset / AfterFrameset / AfterAfterBody / AfterAfterFrameset | §13.2.6.14–§13.2.6.22 |
| P3.6 | Text 模式（text/script/textarea/title 内容收集） | §13.2.6.5 |
| P3-d | 边界修复（adoption agency / foster parenting / foreign content 交互） | §13.2.6.4.7 / §13.2.6.4.9 / §13.2.6.5 |

### Phase 4 — 关键算法 ✅

- **Adoption Agency Algorithm**：§13.2.6.4.7 完整步骤，含 Noah's Ark clause
- **Foster Parenting**：§13.2.6.3 + §13.2.6.2 foster parent location 完整算法（template/table 优先级、before table 插入）
- **Foreign Content**：§13.2.6.5 完整 — MathML/SVG namespace、attribute adjustment、integration points、breakout list
- **generate_implied_end_tags** / **reconstruct_active_formatting_elements** / **reset_insertion_mode** 等辅助算法

### Phase 5 — html5lib Tree Construction 测试集成 ✅

- 测试 fixture：`crates/muskitty-html5-parser/tests/data/tree-construction/*.dat`（56 个 .dat 文件）
- 测试 harness：`crates/muskitty-html5-parser/tests/html5lib_tree_construction.rs`
- DOM 序列化器（html5lib `#document` 格式）
- **通过率：100% (1716/1716)**，204 skipped（document-fragment 192 + script-on 12）
- gap report：`crates/muskitty-html5-parser/tests/tree_construction_gap_report.md`

### Phase 6 — DOM 完整 API 扩展 ⬜ 推迟

Events / Selectors / Style / innerHTML 推迟到 Phase 2 (CSS) 之后。理由：tree construction 只需要 DOM Core 子集；Selectors/Style 依赖 muskitty-css，提前做会返工。

## Tokenizer 状态详情

### 状态实现 (80/80) ✅

完整覆盖 WHATWG §13.2.5.1–§13.2.5.80，详见 [先前版本](https://github.com/Ink-dark/MusKitty/blob/main/PROGRESS.md) 的状态表。

### 辅助基础设施

| 功能 | 文件 | 状态 |
|------|------|------|
| Tokenizer trait | `trait_def.rs` | ✅ |
| HtmlTokenizer struct | `impls.rs` | ✅ |
| reconsume 机制 | `impls.rs` | ✅ |
| pending_tokens 多 token 发射 | `impls.rs` | ✅ |
| 全量 WHATWG 命名实体表 | `entities.rs` | ✅ 2,231 条 |
| 实体查找（二分搜索） | `entities.rs` | ✅ |
| Windows-1252 替换表 | `impls.rs` | ✅ |

### 14 个失败分类

| 类别 | 数量 | 处理 |
|------|-----:|------|
| `<?...>` 处理指令边界（test2/test3） | 11 | Phase 2 期间顺带修 |
| xmlViolation（infoset 强制转换） | 3 | 已从基线排除（CLAUDE.md: WHATWG 是 ground truth） |

## Tree Construction 状态详情

### Insertion Mode 覆盖

全 23 个 insertion mode 完整实现，无 stub：

Initial / BeforeHtml / BeforeHead / InHead / InHeadNoscript / AfterHead / InBody / Text / InTable / InTableText / InCaption / InColumnGroup / InTableBody / InRow / InCell / InSelect / InSelectInTable / InTemplate / AfterBody / InFrameset / AfterFrameset / AfterAfterBody / AfterAfterFrameset

### 关键算法实现位置

| 算法 | 位置 | 规范 |
|------|------|------|
| Adoption Agency | `parser/helpers.rs::adoption_agency` | §13.2.6.4.7 |
| Foster Parenting | `parser/helpers.rs::foster_parent_location` / `insert_node` | §13.2.6.2 / §13.2.6.3 |
| Foreign Content dispatch | `parser/foreign.rs::dispatcher_routes_to_foreign` | §13.2.6 / §13.2.6.5 |
| Active Formatting Elements 重建 | `parser/helpers.rs::reconstruct_active_formatting_elements` | §13.2.6.4 |
| generate_implied_end_tags | `parser/helpers.rs` | §13.2.6.4.1 |
| reset_insertion_mode | `parser/dispatch.rs` | §13.2.6.1 |
| has_element_in_scope (全 5 种 scope) | `parser/helpers.rs` | §13.2.6.4.2 |

### 84.8% → 100% 关键修复（P3-d 系列）

| commit | 修复 |
|--------|------|
| `b55e31a` | Foreign Content (SVG/MathML) 与 Template 交互修复（84.8% → 96.4%） |
| `f901a0d` | Phase 5 测试集成 + bug fixes |
| `4764216` | foster parenting / adoption agency / 插入模式规范修复 |
| `523d816` | ProcessingInstruction 节点支持 |
| `122677d` | reconstruct active formatting elements 经由 foster parenting 插入 |
| `8635e92` | applet/marquee/object 起始与结束标签处理 |
| `10e4cda` | AfterAfterFrameset DOCTYPE/whitespace/<html> 委派 InBody |
| `b55e31a` | Foreign Content + Template 交互修复 |
| `dae15ed` | `<a>` 起始标签 AFE 搜索范围与重构顺序 |
| `1285460` | Noah's Ark clause 移除最早而非最晚的匹配条目 |
| `d7c4eb2` | harness parse_dat 截断 #document 内嵌空行（tricky01.dat #9） |
| `1b889fc` | adoption agency furthest block 匹配 SVG 元素为 special（adoption01.dat #13） |
| `5f767da` | in-cell/applet/marquee/object end-tag namespace 检查（namespace-sensitivity.dat #1） |

## 仓库策略

- **暂不拆分独立 crate**：`muskitty-dom` + `muskitty-html5-parser` 保留在主仓库 `d:\Muskitty` 的 workspace 内，成熟后再拆。
- **主仓库保留为 workspace 协调中心**：后续 CSS/Network/Layout/Renderer crate 在主仓库开发。
- **后续计划**：开发一个工具统一拉取各 crate 源代码（具体形式待定，可能为 git submodule / cargo workspace / vendoring 工具）。
- **远期目标**：`https://github.com/muskitty-dev/muskitty-html5-parser` 仍为预留仓库名，待 HTML Parser + DOM 成熟后再推送。

## Phase 2 规划：muskitty-css (CSS 解析层)

按 [roadmap](.trae/documents/muskitty-browser-roadmap.md) Layer 2 推进。

**目标**：实现 CSS 解析层，为 Layer 3 (Layout) 提供 cascade + computed values。

**入场门槛**（已满足）：
- Layer 1 通过率 ≥80% ✅（100%）
- DOM Core API 完整 ✅

**子阶段**（每个独立 commit + 测试）：

1. **CSS 语法 tokenizer** — CSS Syntax Module §5
   - token 类型（ident/function/at-keyword/hash/string/url/number/...）
   - 状态机：ident / string / url / number / unicode-range
   - 输入预处理（§5.3）+ filter CR（§5.4）

2. **选择器解析** — Selectors Level 4
   - 简单选择器（类型/通用/类/ID/属性）
   - 组合器（后代/子代/相邻兄弟/一般兄弟）
   - 伪类（结构性、UI、动态暂留 stub）
   - 伪元素
   - selector 匹配引擎（基于 DOM Core）

3. **值解析** — CSS Values Module
   - 长度/百分比/角度/时间/分辨率
   - calc() / min() / max()
   - var() 与自定义属性

4. **样式表数据结构** — CSSOM
   - Stylesheet / Rule / Declaration / AtRule
   - CSSStyleRule / CSSMediaRule / CSSImportRule 等

5. **Cascade + Computed values**
   - 重要度/层叠顺序/来源排序
   - 继承 / initial / inherit / unset
   - 计算值 / 使用值 / 实际值

**ground truth**：WPT CSS 测试套件（`css/` 目录）

**规范依据**：
- CSS Syntax Module: <https://drafts.csswg.org/css-syntax-3/>
- Selectors Level 4: <https://drafts.csswg.org/selectors-4/>
- CSS Cascading: <https://drafts.csswg.org/css-cascade-5/>
- CSSOM: <https://drafts.csswg.org/cssom-1/>

**远期 Layer 路线**（不展开细节）：

| Layer | Crate | 入场门槛 |
|-------|-------|---------|
| 3 | muskitty-layout | Layer 2 cascade + computed values 测试通过 |
| 4 | muskitty-renderer | Layer 3 能产出布局盒 |
| 5 | muskitty-network | 可与 Layer 2–4 并行 |

## 源代码结构

```
d:\Muskitty\
├── Cargo.toml                          (workspace)
├── PROGRESS.md                         (本文件)
├── CLAUDE.md                           (硬约束)
├── crates/
│   ├── muskitty-dom/                   (Layer 1 子模块)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── node.rs                 (Node / NodeType / NodeKind / Descendants)
│   │   │   ├── element.rs              (ElementData)
│   │   │   ├── text.rs                 (TextData)
│   │   │   ├── comment.rs              (CommentData)
│   │   │   ├── document.rs             (DocumentData)
│   │   │   ├── document_type.rs        (DocumentTypeData)
│   │   │   ├── document_fragment.rs    (DocumentFragmentData)
│   │   │   ├── processing_instruction.rs
│   │   │   ├── attribute.rs            (Attribute / Namespace)
│   │   │   ├── tree.rs                 (append_child / insert_before / ...)
│   │   │   └── error.rs                (DomError)
│   │   └── tests/
│   │       └── node.rs
│   ├── muskitty-html5-parser/           (Layer 1 主模块)
│   │   ├── Cargo.toml                  (依赖 muskitty-dom)
│   │   ├── src/
│   │   │   ├── lib.rs                  (parse() 入口)
│   │   │   ├── error/mod.rs            (ParseError)
│   │   │   ├── tokenizer/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── types.rs            (Token / TagToken / DoctypeToken / State)
│   │   │   │   ├── trait_def.rs        (Tokenizer trait)
│   │   │   │   ├── impls.rs            (HtmlTokenizer + 80 状态, ~3900 行)
│   │   │   │   └── entities.rs         (2231 条命名实体表)
│   │   │   └── parser/
│   │   │       ├── mod.rs              (HtmlTreeConstructor)
│   │   │       ├── insertion_mode.rs   (InsertionMode 枚举)
│   │   │       ├── dispatch.rs         (insertion mode 分发 + 23 handler)
│   │   │       ├── helpers.rs          (adoption agency / foster parenting / ...)
│   │   │       └── foreign.rs          (SVG/MathML foreign content)
│   │   └── tests/
│   │       ├── data/
│   │       │   ├── tokenizer/*.test
│   │       │   └── tree-construction/*.dat
│   │       ├── html5lib_tokenizer.rs
│   │       ├── html5lib_tree_construction.rs
│   │       ├── html5lib_gap_report.md
│   │       └── tree_construction_gap_report.md
│   ├── muskitty-css/                   (Layer 2 — Phase 2 待开始)
│   ├── muskitty-network/               (Layer 5 — 远期)
│   ├── muskitty-layout/                (Layer 3 — 远期)
│   └── muskitty-renderer/              (Layer 4 — 远期)
└── docs/
    ├── skill/
    │   └── whatwg-spec-adversarial-review.md
    └── tokenizer-spec-review.md
```

## Git 提交历史（近期）

```
5f767da P3-d-10: fix in-cell/applet/marquee/object end-tag namespace check
1b889fc P3-d-9: fix adoption agency furthest block matching SVG elements as special
d7c4eb2 P3-d-8: fix harness parse_dat truncating #document on embedded blank lines
1285460 fix(parser): Noah's Ark clause 移除最早而非最晚的匹配条目
dae15ed fix(parser): <a> 起始标签按 §13.2.6.4.7 正确处理 AFE 搜索范围与重构顺序
9077b14 fix(parser): <select> 起始标签后压入 active formatting 标记    [P3-d-5]
86c8271 feat(parser): 实现 selectedcontent 元素与 option selectedness 处理    [P3-d-4]
be4bba0 fix(parser): adoption agency step 15 经由 adjusted insertion location 插入
e0b5828 fix(parser): InTableBody "anything else" 直接调用 InTable 处理器
5fced9e fix(parser): noembed 起始标签设置 appropriate end tag name
10e4cda fix(parser): AfterAfterFrameset 把 DOCTYPE/whitespace/<html> 委派 InBody
8635e92 feat(parser): 实现 applet/marquee/object 起始与结束标签处理
85e13cd fix(parser): area/br/embed 起始标签设 frameset-ok 为 not ok
b55e31a feat(parser): 实现 Foreign Content (SVG/MathML) 与 Template 交互修复
f901a0d [parser] Phase 5: html5lib tree construction test integration + bug fixes
523d816 feat: 实现处理指令（ProcessingInstruction）节点支持
57eb748 fix(tokenizer): 字符引用属性上下文与命名实体规范合规修复
7296810 style: cargo fmt 格式化
9042c78 [parser] implement remaining 7 insertion modes + template content (Phase 3.5)
14b96c8 [parser] implement table insertion modes + foster parenting skeleton
... (完整历史见 git log)
```

## 下一步

1. **muskitty-css 启动**：建立 `crates/muskitty-css/` 骨架，实现 CSS Syntax Module §5 tokenizer
2. **Tokenizer 遗留**：11 个 `<?...>` 处理指令边界失败，CSS 阶段顺带修
3. **DOM 完整 API**：推迟到 CSS 阶段后，避免返工
4. **拉取工具**：待 crate 数量增多后再做
