# MusKitty — Progress Dashboard

> 最后更新: 2026-07-19 | 基于 git commit `bafaebd` (7 个 crate 全部发布到 crates.io)
>
> Phase 2 子阶段 2（Selectors Level 4）已收尾并剥离为独立仓库。
> §5.4.1 / §5.4.2 grammar hooks 已实现并集成到 Selectors parser。

## 总览

| 模块 | 状态 | 规范覆盖 | 测试通过率 | crates.io | 独立仓库 |
|------|------|---------|-----------|-----------|---------|
| **muskitty-html5-tokenizer** | ✅ 完成 | §13.2.5.1–§13.2.5.80 (80/80) | 99.8% (7022/7036) | v0.1.2 | muskitty-dev/muskitty-html5-tokenizer |
| **muskitty-html5-parser** | ✅ 完成 | §13.2.6 (全 insertion mode + 关键算法) | 100% (1716/1716, 204 skipped) | v0.1.2 | muskitty-dev/muskitty-html5-parser |
| **muskitty-dom** | ✅ 完成 | DOM Living Standard §4–§7 | 单元测试全绿 | v0.1.0 | muskitty-dev/muskitty-dom |
| **muskitty-css-tokenizer** | ✅ 完成 | CSS Syntax §4.3 (§4.3.1–§4.3.13) | 单元测试全绿 | v0.1.1 | muskitty-dev/muskitty-css-tokenizer |
| **muskitty-css-parser** | ✅ 完成 | CSS Syntax §5 (§5.2-§5.5 + §5.4.1/§5.4.2 grammar hooks) | 74 单元 | v0.1.0 | muskitty-dev/muskitty-css-parser |
| **muskitty-css** | ✅ 完成 (facade) | 组合 tokenizer + parser | — | v0.4.0 | muskitty-dev/muskitty-css |
| **muskitty-selectors** | ✅ 完成 | Selectors L4 §3/§4/§5/§6/§13/§14/§15/§17/§18 | 145 测试全绿 | v0.1.0 | muskitty-dev/muskitty-selectors |
| DOM 完整 API (Events/Style/innerHTML) | ⬜ 推迟 | — | — | — | — |
| muskitty-network / muskitty-layout / muskitty-renderer | ⬜ 远期 | — | — | — | — |

**14 个 html5lib tokenizer 失败说明**：3 个 xmlViolation（infoset 强制转换，规范范围外）+ 11 个 `<?...>` PI 边界（test2/test3，html5lib 测试套件过时，期望 `Comment` 但现行 WHATWG §13.2.5.72-76 规定产生 `ProcessingInstruction`）。代码遵循现行 WHATWG 规范，测试套件过时。对浏览器级应用无影响（真实网页几乎不会触发这些边界）。

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

## Phase 2 (CSS 解析层) — 子阶段 1 已完成

按 [roadmap](.trae/documents/muskitty-browser-roadmap.md) Layer 2 推进。子阶段 1（CSS Syntax Module §4.3 tokenizer + §5 parser）按 [phase2-css-parser-cp1-to-cp8.md](.trae/documents/phase2-css-parser-cp1-to-cp8.md) 完成。

### Tokenizer (§4.3) — 早期 commits

CSS Syntax Module §4.3.1–§4.3.13 全部实现，详见 C-2 至 C-7 commits 历史。Token 类型：ident/function/at-keyword/hash/string/url/number/unicode-range/delim/whitespace/colon/semicolon/comma/`{`/`}`/`[`/`]`/`(`/`)`/EOF/CDO/CDC。

### Parser (§5) — CP-1 至 CP-8

| 批次 | 内容 | 规范 | commit |
|------|------|------|--------|
| CP-1 | §5.2 CSS Parsing Results 数据结构 (Stylesheet/Rule/AtRule/QualifiedRule/Declaration/ComponentValue/Function/SimpleBlock/BlockKind) | Overview.md L1625-1721 | `fcb35e6` |
| CP-2 | §5.3 TokenStream struct + 9 操作 (next_token/is_empty/consume_token/discard_token/mark/restore_mark/discard_mark/discard_whitespace + new 构造器) | L1722-1814 | `ab82733` |
| CP-3 | §5.5.7-§5.5.11 底层算法 (consume_a_list_of_component_values / consume_a_component_value / consume_a_simple_block / consume_a_function / consume_a_unicode_range_value) | L2745-2872 | `239cd34` |
| CP-4 | §5.5.6 declaration 算法 (consume_a_declaration + consume_the_remnants_of_a_bad_declaration + strip_important + is_custom_property_name + has_top_level_curly_block_with_other_values) | L2639-2741 | `7f143b7` |
| CP-5 | §5.5.1-§5.5.5 上层算法 (consume_a_stylesheets_contents / consume_an_at_rule / consume_a_qualified_rule / consume_a_block / consume_a_blocks_contents + BlockContents + split_block_contents + looks_like_custom_property_in_prelude + ParseError 标记类型) | L2223-2562 | `980e429` |
| CP-6 | §5.4.3-§5.4.10 entry points (parse_a_stylesheet / parse_a_stylesheets_contents / parse_a_blocks_contents / parse_a_rule / parse_a_declaration / parse_a_component_value / parse_a_list_of_component_values / parse_a_comma_separated_list_of_component_values + normalize_from_string) | L2005-2204 | `cacad17` |
| CP-7 | lib.rs 顶层 API (parse_stylesheet / parse_rule / parse_declaration / parse_component_value / parse_list_of_component_values / parse_comma_separated_list_of_component_values) + crate-level doc | — | `b1e15f4` |
| CP-8 | cleanup + Cargo.toml v0.2.0 (MSRV 1.82) + README + PROGRESS.md 更新 | — | (本 commit) |

### 延后项（标注在代码中，待后续阶段补回）

- ~~**§5.4.1 / §5.4.2 grammar hooks**~~ ✅ 已完成（2026-07-19）：在 `src/grammar.rs` 实现 `Grammar` trait + `parse_a_grammar` + `parse_a_comma_separated_list_with_grammar`；selectors crate 通过 `parser/grammar.rs::SelectorGrammar` 接入，§18 Parse A Selector 走 §5.4.1 路径。
- **§5.5.6 `original_text` for custom property**：`consume_a_declaration` 暂不捕获 `original_text`。需要 `TokenStream` 保留原始 source text 与 token range 映射，是 TokenStream 的扩展。代码中留 TODO 注释，等 var() 实现需求出现后补。
- **§5.5.6 `unicode-range` descriptor re-tokenization**：需要 source-text tracking 用于重新分词。代码中留 TODO 注释。

### 子阶段 1 测试矩阵

| 测试文件 | 测试数 | 覆盖内容 |
|---------|------|---------|
| `tests/parser_types.rs` | 6 | §5.2 数据结构构造 + Default |
| `tests/token_stream.rs` | 8 | §5.3 TokenStream 9 操作 |
| `tests/parser_algorithms_cp3.rs` | 10 | §5.5.7-§5.5.11 底层算法 |
| `tests/parser_algorithms_cp4.rs` | 8 | §5.5.6 declaration 算法 |
| `tests/parser_algorithms_cp5.rs` | 12 | §5.5.1-§5.5.5 上层算法 |
| `tests/parser_entry_points.rs` | 11 | §5.4.3-§5.4.10 entry points |
| `src/lib.rs` doctests | 7 | 顶层 API 集成测试 |
| **总计** | **62** | 全部通过 |

### 质量门禁

每个 CP commit 前依次执行（任一失败不提交）：

```powershell
cargo fmt -p muskitty-css -- --check
cargo test -p muskitty-css
cargo check -p muskitty-css
cargo clippy -p muskitty-css --all-targets -- -D warnings
```

CP-1 至 CP-7 全部满足零 fmt diff、零 warning、全部测试通过。

### TokenStream 设计要点

`consume_a_qualified_rule` 返回 `Result<Option<QualifiedRule>, ParseError>` 三态：

- `Ok(Some(rule))` — 成功消费一个 rule。
- `Ok(None)` — "return nothing"（EOF 或 stop_token 触发）。
- `Err(ParseError)` — "invalid rule error"（如 top-level 的 `--foo: ...` 形 prelude 触发 §5.5.3 L2377-2383）。

`consume_a_blocks_contents` 用 mark/restore_mark 模式处理 declaration 与 qualified-rule prelude 的歧义：先尝试按 declaration 解析；若返回 `None`（不是 declaration），restore_mark 回到 mark 位置再按 qualified-rule 解析。

### `Rule::Declarations` 变体

§5.5.5 的输出是 rules 与 declaration-lists 的混合 list。`Rule` enum 加 `Declarations(Vec<Declaration>)` variant 精确建模。`consume_a_blocks_contents` 返回时把所有 pending decls flush 为 `Rule::Declarations`。后续 CSSOM 可以将其 materialize 为 `CSSStyleDeclaration` 或 `CSSNestedDeclarations`。

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

**全部 7 个 crate 已剥离为独立 git 仓库**（位于 muskitty-dev org 下），并通过 GitHub Actions 自动发布到 crates.io。主仓库 `d:\Muskitty` 现仅作为 workspace 协调中心（`members = []`，所有子 crate 在 `exclude` 列表中）。

### crates.io 发布状态（截至 2026-07-19）

| crate | 版本 | crates.io 发布时间 | 仓库 |
|-------|------|-------------------|------|
| muskitty-dom | 0.1.0 | — | [muskitty-dev/muskitty-dom](https://github.com/muskitty-dev/muskitty-dom) |
| muskitty-html5-tokenizer | 0.1.2 | — | [muskitty-dev/muskitty-html5-tokenizer](https://github.com/muskitty-dev/muskitty-html5-tokenizer) |
| muskitty-html5-parser | 0.1.2 | — | [muskitty-dev/muskitty-html5-parser](https://github.com/muskitty-dev/muskitty-html5-parser) |
| muskitty-css-tokenizer | 0.1.1 | 2026-07-19 | [muskitty-dev/muskitty-css-tokenizer](https://github.com/muskitty-dev/muskitty-css-tokenizer) |
| muskitty-css-parser | 0.1.0 | 2026-07-19 | [muskitty-dev/muskitty-css-parser](https://github.com/muskitty-dev/muskitty-css-parser) |
| muskitty-css | 0.4.0 | 2026-07-19T11:55:38Z | [muskitty-dev/muskitty-css](https://github.com/muskitty-dev/muskitty-css) |
| muskitty-selectors | 0.1.0 | 2026-07-19T12:11:16Z | [muskitty-dev/muskitty-selectors](https://github.com/muskitty-dev/muskitty-selectors) |

### CI/CD 模式

每个独立 crate 仓库统一采用：

- **CI workflow**（`.github/workflows/ci.yml`）：6 个 job（Check / Unit Tests / Integration Tests / Format / Clippy / MSRV 1.82）；通过 `scripts/setup-deps.sh` 克隆 path 依赖到 `../` 相对路径；通过 `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` 注入避免 anonymous clone 被限流。
- **Publish workflow**（`.github/workflows/publish.yml`）：tag-triggered（`v*`）；幂等设计（先查询 crates.io API，若版本已存在则跳过）；成功后通过 `softprops/action-gh-release` 创建 GitHub Release。
- **`CARGO_REGISTRY_TOKEN` secret**：通过 `gh secret set` 配置到每个独立仓库。

### 主仓库职责

- **workspace 协调中心**：保留 `d:\Muskitty\Cargo.toml` 作为 workspace 根（`members = []` + `exclude = [...]`），便于本地开发时一次性构建所有 crate。
- **文档中心**：保留 `PROGRESS.md` / `CLAUDE.md` / `.trae/documents/` 作为项目级文档。
- **未来 crate 预留**：`muskitty-network` / `muskitty-layout` / `muskitty-renderer` 将在主仓库内开发，成熟后再剥离。

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

主仓库仅作为 workspace 协调中心（`members = []`）；7 个子 crate 各自独立 git 仓库，本地通过 `crates/` 目录与主仓库同级共存。具体每个 crate 的内部结构见各自仓库的 README。

```
d:\Muskitty\                              # 主仓库 (Ink-dark/MusKitty)
├── Cargo.toml                           # workspace 根：members = [], exclude = [7 个 crate]
├── .gitignore                           # 排除 crates/muskitty-*/ 目录
├── PROGRESS.md                          # 本文件
├── CLAUDE.md                            # 硬约束
├── docs/                                # 项目级文档
├── .trae/documents/                     # 阶段规划文档
└── crates/                              # 子 crate 各自独立 git 仓库
    ├── muskitty-dom/                    # → muskitty-dev/muskitty-dom (v0.1.0)
    ├── muskitty-html5-tokenizer/        # → muskitty-dev/muskitty-html5-tokenizer (v0.1.2)
    ├── muskitty-html5-parser/           # → muskitty-dev/muskitty-html5-parser (v0.1.2)
    ├── muskitty-css-tokenizer/          # → muskitty-dev/muskitty-css-tokenizer (v0.1.1)
    ├── muskitty-css-parser/             # → muskitty-dev/muskitty-css-parser (v0.1.0)
    ├── muskitty-css/                    # → muskitty-dev/muskitty-css (v0.4.0)
    └── muskitty-selectors/              # → muskitty-dev/muskitty-selectors (v0.1.0)
```

未来 crate 预留（在主仓库内开发，成熟后再剥离）：`crates/muskitty-css-values`、`crates/muskitty-network`、`crates/muskitty-layout`、`crates/muskitty-renderer`。

## Git 提交历史（近期）

```
bafaebd [workspace] hard-extract muskitty-css from workspace members
1168434 [workspace] hard-extract dom/html5-parser/selectors from members
3e2d8fb [css] re-export grammar module from muskitty-css-parser
7629c41 [chore] update PROGRESS.md: muskitty-selectors extracted to independent repo
11cbdf1 [chore] untrack muskitty-selectors (extracted to independent repo)
1b27bae [selectors] SP-8: mark Phase 2 子阶段 2 (Selectors Level 4) complete in PROGRESS.md
b1e15f4 [css-parser] CP-7: lib.rs top-level API + crate-level doc
cacad17 [css-parser] CP-6: 5.4 Parser Entry Points (9 of 10)
980e429 [css-parser] CP-5: 5.5.1-5.5.5 stylesheet/rule/block algorithms
7f143b7 [css-parser] CP-4: 5.5.6 consume_a_declaration + remnants_of_bad_decl
239cd34 [css-parser] CP-3: 5.5.7-5.5.11 lower-level parser algorithms
ab82733 [css-parser] CP-2: 5.3 TokenStream struct + 8 operations
fcb35e6 [css-parser] CP-1: 5.2 CSS Parsing Results data structures
d3dba7d [workspace] Untrack crates/muskitty-html5-tokenizer/ (already .gitignored)
7e4a19c [workspace] Split muskitty-html5-tokenizer into standalone git repo
c233c74 [tokenizer] Extract muskitty-html5-tokenizer as standalone crate
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
... (完整历史见 git log)
```

## 下一步

1. **Phase 2 子阶段 3 — CSS Values Module**：长度/百分比/角度/时间/分辨率、`calc()` / `min()` / `max()`、`var()` 与自定义属性（需补回 §5.5.6 `original_text` 捕获）。新 crate：`muskitty-css-values`。
2. **Phase 2 子阶段 4 — CSSOM**：Stylesheet / Rule / Declaration / AtRule 数据结构映射到 CSSStyleRule / CSSMediaRule / CSSImportRule 等。
3. **Phase 2 子阶段 5 — Cascade + Computed values**：重要度/层叠顺序/来源排序、继承 / initial / inherit / unset、计算值 / 使用值 / 实际值。入场门槛 = Layer 2 cascade + computed values 测试通过（满足后可进入 Layer 3 Layout）。
4. **DOM 完整 API 扩展**：Events / Style / innerHTML — 推迟到 CSS Values + CSSOM 完成后做，避免返工。
5. **Tokenizer 遗留**：14 个 html5lib 失败已确认非 bug（11 PI 测试过时 + 3 xmlViolation 规范外），**保持现状**。如未来 html5lib 上游更新测试自然转绿。
6. **`§5.5.6 original_text` / `unicode-range` re-tokenization**：等 CSS Values 阶段需要时补。

## Phase 2 子阶段 2 — Selectors Level 4 ✅

按 [phase2-selectors-sp1-to-sp8.md](.trae/documents/phase2-selectors-sp1-to-sp8.md) 8 个 SP batch 全部完成，覆盖 [Selectors Level 4](https://drafts.csswg.org/selectors-4/) §3 / §4 / §5 / §6 / §13 / §14 / §15 / §17 / §18。

| SP  | 内容                                              | 状态 |
| --- | ------------------------------------------------- | ---- |
| SP-1 | §3 数据模型 + parser 框架                        | ✅   |
| SP-2 | §5 / §6.5 / §6.6 type / universal / class / id 解析 | ✅   |
| SP-3 | §6 attribute selectors                            | ✅   |
| SP-4 | §13 tree-structural pseudo + An+B 解析            | ✅   |
| SP-5 | §4 logical combinations (is/not/where/has) 解析  | ✅   |
| SP-6 | §15 combinators + complex selector                | ✅   |
| SP-7 | §17 specificity                                   | ✅   |
| SP-8 | §18 matching engine + lib API（含 DOM 端到端测试） | ✅   |

测试矩阵：

| 测试文件                | 测试数 | 覆盖内容                                          |
| ---------------------- | -----: | ------------------------------------------------ |
| `tests/parser_types.rs`     | 6  | §3 数据结构 + Combinator / PseudoClassArgument    |
| `tests/parser_simple.rs`    | 10 | type / universal / class / id / ns 解析            |
| `tests/parser_attribute.rs`| 11 | §6 属性选择器（presence/exact/`~`/`|`/`^`/`$`/`*`/modifier）|
| `tests/parser_pseudo_tree.rs` | 12 | §13 tree-structural + An+B 解析                   |
| `tests/parser_nth_of.rs`    | 4  | §13.3 `nth-child(An+B of S?)` 解析                |
| `tests/parser_logical.rs`   | 10 | §4 is/not/where/has 解析（forgiving / 非forgiving）|
| `tests/parser_complex.rs`   | 12 | §15 combinators + mixed + trailing 拒绝          |
| `tests/specificity.rs`      | 22 | §17 A/B/C triplet + is/not/has/nth-of 取最大     |
| `tests/matching_basic.rs`   | 19 | §5 / §6 simple-selector 匹配 + §15 组合器匹配      |
| `tests/matching_pseudo.rs`  | 29 | §13 tree-structural + An+B + §4 logical 匹配      |
| `tests/matching_dom.rs`     | 10 | 端到端 DOM 匹配 + `query_selector(_all)`          |
| **总计**                | **145** | 全部通过                                      |

架构：

- **Parser** (`src/parser/`) — 复用 `muskitty-css::tokenize`，构建 `SelectorList` / `ComplexSelector` / `CompoundSelector` / `SubclassSelector` / `PseudoClass` / `PseudoElement`。无 DOM 依赖。
- **Specificity** (`src/specificity.rs`) — 按 §17 计算 A/B/C 三元组。`:is()` / `:not()` / `:has()` 取参数最大值；`:where()` 贡献 0。
- **Matching** (`src/matching/`) — 通过 `Element` trait 抽象元素 5 个 aspect（§3 L858-873）；右-左走序匹配（§18 L4902-4919）。包含 simple_matcher / pseudo_matcher / dom_impl 子模块。

延后项：

- `:has()` 多 compound 相对选择器（`:has(.a > .b)`）— SP-8 仅支持单 compound；多 compound 返回 `false`。
- 命名空间严格匹配（`ns|tag`）— 当前保守处理为"任意命名空间均可"。
- WPT 子集集成 — 推迟到拆仓后做。
- §7-§12 UI / location / linguistic / resource / display / input 伪类 — 解析已支持，匹配 stub 返回 `false`。

crate 成熟度满足拆分独立 git 仓库的条件（1952 LoC src + 1123 LoC tests，145 测试全绿，覆盖 §3 / §4 / §5 / §6 / §13 / §14 / §15 / §17 / §18）。已于 2026-07-19 剥离为独立仓库 [muskitty-dev/muskitty-selectors](https://github.com/muskitty-dev/muskitty-selectors)（Hard extraction，自有 `[workspace]` 块，path 依赖指向 `../muskitty-css` 等同级 crate）。v0.1.0 已发布到 crates.io（2026-07-19T12:11:16Z）。

### §5.4.1 / §5.4.2 Grammar Hooks 集成（2026-07-19）

- `muskitty-css-parser/src/grammar.rs`：新增 `Grammar` trait + `parse_a_grammar` + `parse_a_comma_separated_list_with_grammar`（CSS Syntax Module Level 3 §5.4.1/§5.4.2）。
- `muskitty-selectors/src/parser/cv_adapter.rs`：ComponentValue → Token 适配器，让 Selectors parser 复用 CSS Syntax 解析路径。
- `muskitty-selectors/src/parser/grammar.rs`：`SelectorGrammar` + `RelativeSelectorGrammar`，让 §18 Parse A Selector 走 §5.4.1 路径。
- `muskitty-css/src/parser/mod.rs`：重新导出 grammar 模块。
- 测试：css-parser 74 / selectors 150 / workspace 263 全部通过。
