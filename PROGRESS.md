# MusKitty — Progress Dashboard

> 最后更新: 2026-08-29 | M-3 batch 1 完成（border 简写展开 → border-width/style/color + media 视口接线：`@media (min-width)` 等条件现按真实窗口逻辑视口求值）；下一里程碑 W-4 Headless 后端 + M-3 batch 2
>
> Phase 3（Layout 层）已完成并剥离：`muskitty-layout` v0.1.0 已拆为独立 git 仓库（muskitty-dev org）。
> Phase 4（Renderer）B-3 / B-4 已完成：`muskitty-renderer`（tiny-skia 后端）DOM→CSS→Layout→Render 全链路打通，最小 demo（HTML+CSS → PNG）工作。
> 主仓库 workspace `members = ["crates/muskitty-renderer", "crates/muskitty-network"]`；renderer/network 在主仓库内（未剥离），其余 11 个 crate 已剥离为独立 git 仓库（muskitty-dev org），由 `fetch-crates.ps1` / `fetch-crates.sh` 一次性拉取。

## 总览

| 模块 | 状态 | 规范覆盖 | 测试通过率 | crates.io | 独立仓库 |
|------|------|---------|-----------|-----------|---------|
| **muskitty-html5-tokenizer** | ✅ 完成 | §13.2.5.1–§13.2.5.80 (80/80) | 99.8% (7022/7036) | v0.1.2 | muskitty-dev/muskitty-html5-tokenizer |
| **muskitty-html5-parser** | ✅ 完成 | §13.2.6 (全 insertion mode + 关键算法) | 100% (1716/1716, 204 skipped) | v0.1.2 | muskitty-dev/muskitty-html5-parser |
| **muskitty-dom** | ✅ 完成 | DOM Living Standard §4–§7 | 单元测试全绿 | v0.1.0 | muskitty-dev/muskitty-dom |
| **muskitty-css-tokenizer** | ✅ 完成 | CSS Syntax §4.3 (§4.3.1–§4.3.13) + span tracking | 单元测试全绿 | v0.2.0 | muskitty-dev/muskitty-css-tokenizer |
| **muskitty-css-parser** | ✅ 完成 | CSS Syntax §5 (§5.2-§5.5 + §5.4.1/§5.4.2 grammar hooks + §5.5.6 original_text) | 74 单元 + 3 source-text | v0.2.0 | muskitty-dev/muskitty-css-parser |
| **muskitty-css** | ✅ 完成 (facade) | 组合 tokenizer + parser | — | v0.5.0 | muskitty-dev/muskitty-css |
| **muskitty-selectors** | ✅ 完成 | Selectors L4 §3/§4/§5/§6/§13/§14/§15/§17/§18 | 145 测试全绿 | v0.1.0 | muskitty-dev/muskitty-selectors |
| **muskitty-css-values** | ✅ 完成 | CSS Values L4 §4/§5/§6/§8/§9 + CSS Variables §2/§3 | 148 测试全绿 | v0.1.0 | muskitty-dev/muskitty-css-values |
| **muskitty-cssom** | ✅ 完成 | CSSOM §3/§8.1/§8.4/§8.5/§8.6 | 81 测试全绿 | v0.1.0 | muskitty-dev/muskitty-cssom |
| **muskitty-cascade** | ✅ 完成 | CSS Cascade L5 §4.1-§4.4/§5/§6.1/§7 | 71 测试全绿 | 本地 v0.1.0 (未发布) | muskitty-dev/muskitty-cascade (已剥离) |
| **muskitty-layout** | ✅ 完成 | CSS Display L3 §2 + Box Model L3 §2/§3 + Flexbox L1 §4-§8 + taffy 0.12 集成 | 46 测试全绿 | 本地 v0.1.0 (未发布) | 🔗 muskitty-dev/muskitty-layout (已剥离) |
| **muskitty-renderer** | ✅ Phase 4 B-3/B-4 | tiny-skia 后端：DOM→CSS→Layout→Render 全链路 + HTML+CSS→PNG demo | — | 本地 v0.1.0 (未发布) | 主仓库内 (未剥离) |
| DOM 完整 API (Events/Style/innerHTML) | ✅ 完成 (2026-08-09) | Events → dom `event.rs` · element.style → cssom `element_style.rs` · innerHTML/outerHTML → html5-parser `serialize.rs`+`parse_fragment` | dom/cssom 全绿 + html5-parser WPT 99.0% (1889/1908) | — | — |
| **muskitty-network** | 🚧 Phase 5 启动 | NetworkFetcher trait 抽象 + reqwest 后端（远期自研 HTTP/1.1+2+3 栈，见 [plan](docs/plans/2026-08-09-phase5-network.md)） | 7 测试全绿 (wiremock 离线) | 本地 v0.1.0 (未发布) | 主仓库内 (未剥离) |

**14 个 html5lib tokenizer 失败说明**：3 个 xmlViolation（infoset 强制转换，规范范围外）+ 11 个 `<?...>` PI 边界（test2/test3，html5lib 测试套件过时，期望 `Comment` 但现行 WHATWG §13.2.5.72-76 规定产生 `ProcessingInstruction`）。代码遵循现行 WHATWG 规范，测试套件过时。对浏览器级应用无影响（真实网页几乎不会触发这些边界）。

## Phase 1 (HTML 解析层) — 已收尾

按 [muskitty-browser-roadmap.md](.trae/archive/muskitty-browser-roadmap.md) 的 6 个 Phase 全部完成（Phase 6 推迟）：

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

### Phase 6 — DOM 完整 API 扩展 ✅ 已完成 (2026-08-09)

- **Events**（DOM §4.4/4.5/4.6）→ muskitty-dom `src/event.rs`：`add/remove_event_listener` + `dispatch_event`（捕获/目标/冒泡三阶段，`Event` 状态机，零依赖纯 leaf）。
- **element.style**（CSSOM §4）→ muskitty-cssom `src/element_style.rs`：扩展 trait（dom 为 source of truth，parse→mutate→serialize→写回 attribute，无缓存对象）。
- **innerHTML/outerHTML**（HTML §13.4.2 / §13.6.4-5）→ muskitty-html5-parser `src/serialize.rs` + `parse_fragment`：fragment parsing（context 重建、reset 替换、tokenizer 初态、unwrap）+ 序列化（Normal/RawText/EscapableRawText 转义、void、template content）；harness 解锁 document-fragment 用例，WPT 99.0% (1889/1908)，12 script-on 跳过。已知遗留：18 个 foreign-context fragment + tests_innerHTML_1 #76（select-context，WPT 夹具早于 2016 reset 删除 select 分支，现行 WHATWG reset 无 select 分支 → InBody 按规范插入 input）。

三个子任务各自独立 commit + push（dom/cssom/html5-parser 独立仓库）。

## Phase 2 (CSS 解析层) — 子阶段 1-5 已完成

按 [roadmap](.trae/archive/muskitty-browser-roadmap.md) Layer 2 推进。子阶段 1（CSS Syntax Module §4.3 tokenizer + §5 parser）按 [phase2-css-parser-cp1-to-cp8.md](.trae/archive/phase2-css-parser-cp1-to-cp8.md) 完成。

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
- ~~**§5.5.6 `original_text` for custom property**~~ ✅ 已完成（2026-07-22，CV-0b）：`CssTokenizer` 加 `next_token_with_span` + `position()`；`TokenStream` 加 `with_source` 构造器 + `source_slice` 方法 + `token_spans`/`source` 字段；`consume_a_declaration` 对 custom property 捕获 `original_text`。
- **§5.5.6 `unicode-range` descriptor re-tokenization**：需要 source-text tracking 用于重新分词。基础设施已就绪（CV-0b），待实际需求出现后补。

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

**11 个 crate 已剥离为独立 git 仓库**（位于 muskitty-dev org 下，含 cascade/cssom，2026-08-09 剥离），并通过 GitHub Actions 自动发布到 crates.io（发布状态见下表）。`muskitty-renderer`、`muskitty-network` 作为主仓库 workspace member 开发（未剥离、未发布）。主仓库 `d:\Muskitty` 的 workspace `members = ["crates/muskitty-renderer", "crates/muskitty-network"]`，`exclude` 列表排除 11 个已剥离 crate。新设备 clone 主仓库后通过 `fetch-crates.ps1` / `fetch-crates.sh` 一次性拉取。

### crates.io 发布状态（截至 2026-08-09）

| crate | 版本 | crates.io 发布时间 | 仓库 |
|-------|------|-------------------|------|
| muskitty-dom | 0.2.0 | — | [muskitty-dev/muskitty-dom](https://github.com/muskitty-dev/muskitty-dom) |
| muskitty-html5-tokenizer | 0.1.3 | — | [muskitty-dev/muskitty-html5-tokenizer](https://github.com/muskitty-dev/muskitty-html5-tokenizer) |
| muskitty-html5-parser | 0.2.0 | — | [muskitty-dev/muskitty-html5-parser](https://github.com/muskitty-dev/muskitty-html5-parser) |
| muskitty-css-tokenizer | 0.2.0 | 2026-07-24 | [muskitty-dev/muskitty-css-tokenizer](https://github.com/muskitty-dev/muskitty-css-tokenizer) |
| muskitty-css-parser | 0.3.0 | 2026-07-24 | [muskitty-dev/muskitty-css-parser](https://github.com/muskitty-dev/muskitty-css-parser) |
| muskitty-css | 0.6.0 | 2026-07-24 | [muskitty-dev/muskitty-css](https://github.com/muskitty-dev/muskitty-css) |
| muskitty-selectors | 0.2.0 | 2026-07-19T12:11:16Z | [muskitty-dev/muskitty-selectors](https://github.com/muskitty-dev/muskitty-selectors) |
| muskitty-css-values | 0.1.0 | 2026-07-24 | [muskitty-dev/muskitty-css-values](https://github.com/muskitty-dev/muskitty-css-values) |
| muskitty-cssom | 0.1.0 | 2026-07-24 | [muskitty-dev/muskitty-cssom](https://github.com/muskitty-dev/muskitty-cssom) |
| muskitty-layout | 0.1.0 | — | [muskitty-dev/muskitty-layout](https://github.com/muskitty-dev/muskitty-layout) |
| muskitty-cascade | 0.1.0 (未发布) | — | [muskitty-dev/muskitty-cascade](https://github.com/muskitty-dev/muskitty-cascade) |

### CI/CD 模式

每个独立 crate 仓库统一采用：

- **CI workflow**（`.github/workflows/ci.yml`）：6 个 job（Check / Unit Tests / Integration Tests / Format / Clippy / MSRV 1.82）；通过 `scripts/setup-deps.sh` 克隆 path 依赖到 `../` 相对路径；通过 `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` 注入避免 anonymous clone 被限流。
- **Publish workflow**（`.github/workflows/publish.yml`）：tag-triggered（`v*`）；幂等设计（先查询 crates.io API，若版本已存在则跳过）；成功后通过 `softprops/action-gh-release` 创建 GitHub Release。
- **`CARGO_REGISTRY_TOKEN` secret**：通过 `gh secret set` 配置到每个独立仓库。

### 主仓库职责

- **workspace 协调中心**：保留 `d:\Muskitty\Cargo.toml` 作为 workspace 根（`members = ["crates/muskitty-renderer", "crates/muskitty-network"]` + `exclude = [11 个已剥离 crate]`），便于本地开发时一次性构建所有 crate。
- **新 crate 孵化器**：`muskitty-renderer` / `muskitty-network` 作为 workspace member 在主仓库内开发（未剥离）；已剥离 crate 各自独立维护。
- **文档中心**：保留 `PROGRESS.md` / `CLAUDE.md` / `AGENTS.md` / `goal.md` / `docs/plans/` 作为项目级文档。

## Phase 2 规划：muskitty-css (CSS 解析层)

按 [roadmap](.trae/archive/muskitty-browser-roadmap.md) Layer 2 推进。

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

主仓库作为 workspace 协调中心；11 个 crate 各自独立 git 仓库（在 `exclude` 列表中），`muskitty-renderer` / `muskitty-network` 作为 workspace member 在主仓库内开发（未剥离）。具体每个独立 crate 的内部结构见各自仓库的 README。

```
d:\Muskitty\                              # 主仓库 (Ink-dark/MusKitty)
├── Cargo.toml                           # workspace 根：members = [renderer, network], exclude = [11 个已剥离 crate]
├── .gitignore                           # 排除已剥离 crate 目录
├── fetch-crates.ps1 / .sh              # 一次性拉取 11 个独立 crate 的脚本
├── PROGRESS.md                          # 本文件
├── CLAUDE.md / AGENTS.md                # 硬约束
├── goal.md                              # 当轮任务清单与退出条件
├── docs/                                # 项目级文档（plans/ audit/ archive/）
├── .trae/archive/                     # 阶段规划文档
└── crates/                              # 子 crate
    ├── muskitty-renderer/              # 主仓库成员 (v0.1.0, 未剥离, tiny-skia 后端)
    ├── muskitty-network/               # 主仓库成员 (v0.1.0, 未剥离, NetworkFetcher + reqwest)
    ├── muskitty-cascade/               # → muskitty-dev/muskitty-cascade (v0.1.0, 已剥离)
    ├── muskitty-cssom/                 # → muskitty-dev/muskitty-cssom (v0.1.0, 已剥离)
    ├── muskitty-layout/                # → muskitty-dev/muskitty-layout (v0.1.0, 独立仓库)
    ├── muskitty-dom/                    # → muskitty-dev/muskitty-dom (v0.2.0, 独立仓库)
    ├── muskitty-html5-tokenizer/        # → muskitty-dev/muskitty-html5-tokenizer (v0.1.3, 独立仓库)
    ├── muskitty-html5-parser/           # → muskitty-dev/muskitty-html5-parser (v0.2.0, 独立仓库)
    ├── muskitty-css-tokenizer/          # → muskitty-dev/muskitty-css-tokenizer (v0.2.0, 独立仓库)
    ├── muskitty-css-parser/             # → muskitty-dev/muskitty-css-parser (v0.3.0, 独立仓库)
    ├── muskitty-css/                    # → muskitty-dev/muskitty-css (v0.6.0, 独立仓库)
    ├── muskitty-selectors/              # → muskitty-dev/muskitty-selectors (v0.2.0, 独立仓库)
    └── muskitty-css-values/             # → muskitty-dev/muskitty-css-values (v0.1.0, 独立仓库)
```

未来 crate 预留：`crates/muskitty-network`（Layer 5）。

## Git 提交历史（近期）

```
d9a8a9b [css-values] CV-6: lib top-level API + doctests (9 tests)
44e80cb [css-values] CV-5: ValuesGrammar impl Grammar + serialization (§5.4.1, §8.1, §9.7)
0c3f519 [css-values] CV-3: MathExpression AST + calc/min/max/clamp parsing (§9)
6edd8f9 [css-values] CV-4: VarReference parsing (CSS Variables §3)
c18c153 [css-values] CV-2: textual types - Keyword/CustomIdent/DashedIdent/CssString/Url
084809a [css-values] CV-1: numeric types + crate skeleton (§4.4-§4.7, §5, §6)
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

1. ~~**Phase 2 子阶段 3 — CSS Values Module**~~ ✅ 已完成（2026-07-22）并已提取发布（2026-07-24）到 crates.io。
2. ~~**Phase 2 子阶段 4 — CSSOM**~~ ✅ 已完成（2026-07-22）并已提取发布（2026-07-24）到 crates.io。
3. ~~**Phase 2 子阶段 5 — Cascade + Computed values**~~ ✅ 已完成（2026-07-23）。
4. ~~**Cascade 收尾**（Phase 3 前置）~~ ✅ 已完成（2026-08-01）：inline `style` 属性收集已实现。
5. ~~**Phase 3 — Layout**~~ ✅ 已完成（2026-08-01）：taffy 0.12 集成，46 个测试全绿，审计修复 7 个 bug。已剥离为独立仓库。
6. ~~**Phase 4 — Renderer**~~ ✅ B-3/B-4 已完成（2026-08-02）：`muskitty-renderer`（tiny-skia 后端）DOM→CSS→Layout→Render 全链路打通，HTML+CSS → PNG demo 工作。
7. ~~**全项目审计修复**~~ ✅ 已完成（2026-08-09）：B1-B14 全部完成，P0/P1/P2 清零，见 `docs/audit-2026-08-08-full-scan.md` 修复状态汇总。收尾：P2-1（绝对长度单位 ✅）/ P3-2（calc 求值 ✅）/ PERF-10（map_style 优化 ✅）/ 简写展开（margin/padding/flex/background/font ✅，cascade `d6d7208`）/ 布局 B6 flex 简写端到端解锁（layout `44b9b1d`）。
8. ~~**DOM 完整 API 扩展**~~ ✅ 已完成（2026-08-09）：Events → muskitty-dom `event.rs`；element.style → muskitty-cssom `element_style.rs`；innerHTML/outerHTML → muskitty-html5-parser `serialize.rs` + `parse_fragment`（WPT harness 解锁 fragment 用例，99.0%）。
9. **Tokenizer 遗留**：14 个 html5lib 失败已确认非 bug，**保持现状**。
10. ~~**文本渲染 + 布局增强 + 窗口化**~~ ✅ 已完成（2026-08-16）：文本渲染（layout 测量 `3d18bf4` + renderer glyph 渲染 `50bc822`）、position 定位（`d721a0b`）、overflow 裁剪（`d09bc78`）、grid 布局（cascade `2c27d7d` + layout `e690d4e`）、winit 窗口化（`07eb0b8`）。均已推送远端。
11. ~~**T-3 换行 + 字体属性**~~ ✅ 已完成（2026-08-22）：taffy measure function 换行（layout `8f18108`）、font-family/font-weight 测量（layout `056ec23`）、renderer 换行/字重/对齐（renderer `8a28bc8`/`fc6f971`/`20606b8`）。补齐端到端用例时发现并修复多行叠行 bug（`draw_text` 漏加 `run.line_y`，glyph 行内局部坐标需加行顶偏移）。line-height 仍为 `font_size * 1.2` 近似，精确解析推迟。
12. ~~**外部依赖解耦**~~ ✅ 已完成（2026-08-16）：layout（taffy/cosmic-text `bf52557`）、renderer（tiny-skia Pixmap `98af15d`）、network（reqwest Error `8cfbdfd`）公共 API 均不再暴露外部依赖类型，上层可抽离。
13. ~~**W-2 DPI（HiDPI 缩放）**~~ ✅ 已完成（2026-08-29）：`Backend::render` / `render_page` / `render_html_file` 加 scale 参数——layout 用逻辑视口（CSS px），栅格化物理分辨率 `round(logical×scale)`（renderer `0192cae` / shell page `b0a645f`）；窗口流读 hidpi scale、脏检查含 scale、`ScaleFactorChanged` 重绘（shell `d53ca2f`）。整数 1x/2x 有 scale=1-vs-2 单测兜底（非插值）；`render_file` 示例 scale=1 输出不变。
14. ~~**W-3 输入（InputEvent 抽象 + shell 快捷键层 + 事件分发结构）**~~ ✅ 已完成（2026-08-29）：input.rs 类型 + 纯函数 `match_shortcut`（shell `953f890`）、`PlatformWindow::handle_event` 页面层入口（`7fed8bb`）、App 事件接线 + 转换函数（`0962c74`）、文档同步（C-4）。架构修正：快捷键层在 `App::dispatch_input`（Esc 需 `event_loop.exit()`、Ctrl+R 需渲染管线，均 App 独有），`handle_event` 为页面层（W-3 无命中测试恒 `false`，仅立分发结构）；页面级命中测试单列延后。
15. ~~**M-3 batch 1（border 简写 + media 视口接线）**~~ ✅ 已完成（2026-08-29）：`border:` 简写按 CSS Backgrounds & Borders L3 §4.4 展开为 border-width/style/color（顺序无关 `<width>||<style>||<color>`、每类至多一次、缺失类别取注册表初始值 medium/none/currentcolor，cascade `b87b820`；renderer 端到端 paint 测试证明 `extract_border` 消费，`de48621`）；media 视口接线——`compute_styles` 用 `StyleTreeOptions.viewport_width/height` 构造 `MediaContext`（cascade `fcde127`），`render_page` 传逻辑布局视口（shell `f0c5619`），默认 1920×1080 行为不变。M-3 余项：@layer 排序已完整（audit B8，无需再做）；background-image / revert 真语义 / 方向性 border（border-left 等）/ outline 延后（renderer 无 image 消费方；revert 需低 origin/层回滚零真实页面需求）。

16. ~~**W-4 Headless 后端（HeadlessWindow + render_to_png + 无窗口测试）**~~ ✅ 已完成（2026-08-29）：`HeadlessWindow: PlatformWindow` 无窗口实现（`present` 保存最近帧、可 `save_png`，无外部依赖类型可公开构造，shell `070e013`）；lib 顶层 `render_to_png` 全管线 → PNG 便捷函数 + `page::encode_png` 编码出口（tiny-skia 升正式依赖但类型不入 pub API；window_demo 示例声明 required-features，shell `25610a6`）；无窗口集成测试——`render_to_png` PNG 解码像素与 `render_page` 直接渲染逐字节一致、HeadlessWindow 帧/编码产物一致、scale=2 分辨率核对（shell `2c83e44`），`--no-default-features` 下 check/test/clippy 全绿，feature gate 兑现 CI 无窗口价值。

17. ~~**W-5 多标签状态管理（WebViewCollection + 标签快捷键 + 脏位延迟更新）**~~ ✅ 已完成（2026-08-29）：`webview.rs`（不 feature 门控）——`WebView`（内容 + 每标签渲染状态 + `needs_repaint`/`close_scheduled` 脏位）+ `WebViewCollection`（新建/延迟关闭/切换，active 不变量，切换自动标脏，shell `a618c4e`）；标签快捷键 Ctrl+T/W/1~9/PageUp/PageDown（`ShortcutAction` 扩展 + `Key::PageUp/PageDown`，`match_shortcut` 5 条新单测，`dispatch_input` 接线，Ctrl+T 开默认内容、全部关闭退出，shell `9e5c3dc`）；脏位延迟更新——shell 动作只标脏 + request_repaint，`RedrawRequested` 统一 flush（关标签延迟移除/空则退出/脏或 stale 才重渲染，shell `ea99d22`）。范围裁剪：favicon 占位、tab strip 不做。窗口化轨道 W-1~W-5 全部完成。

18. ~~**muskitty-chrome 窗口层（自绘 chrome，取代 shell）**~~ ✅ 已完成（2026-08-29）：决策见 ADR `2026-08-29-chrome-window-layer`（egui GPU 管线冲突 / iced 框架开销，选 Chromium Views 式自绘合成）。`chrome::model`（布局纯函数 9 测）/ `paint`（tiny-skia + cosmic-text 0.13 + swash outline，7 像素测）/ `input`（hit_test/apply 6 测）/ `compositor`（页面+chrome 同帧合成）/ `app`（winit + softbuffer、标签集合、脏位 flush）。功能：多标签快捷键、地址栏（Ctrl+L/输入/回车提交 → 占位页 + 标签标题）、**文件热重载**（mtime 200ms 轮询）、`render_window_to_png` 无窗口 CI 测试（62 条，`--no-default-features` 全绿）。真窗口验证（自动化 + 用户实测）发现并修复按键双发（漏 Pressed 过滤）。`muskitty-shell` 退役删除（`84b07a4`，git rename 保留历史），W-1~W-5 语义由 chrome 承接。

## Phase 3 (Layout 层) — 已完成

**时间**：2026-07-23 → 2026-08-01
**最终交付**：`muskitty-layout` v0.1.0（本地，未剥离），46 个测试全绿。

### 子阶段

| 子阶段 | 内容 | 状态 |
|--------|------|------|
| L-0 | crate 骨架 + Cargo.toml + lib.rs 文档 | ✅ |
| L-1 | LayoutTree 类型（taffy TaffyTree + NodeId 映射） | ✅ |
| L-2 | ComputedStyle → taffy Style 映射（style_map.rs） | ✅ |
| L-3 | DOM + ComputedStyle → LayoutTree 转换（convert.rs） | ✅ |
| L-4 | 布局计算（compute_layout 函数 + LayoutResult） | ✅ |
| L-5 | 单元测试（35 style_map + 8 compute） | ✅ |
| L-6 | 端到端集成测试（7 个：HTML+CSS → cascade → layout → result） | ✅ |

### 审计修复（whatwg-spec-adversarial-review skill）

2026-08-01 对照 CSS Display L3 / Box Model L3 / Flexbox L1 / Box Alignment L3 / Cascade L5 规范审计，发现并修复 7 个 bug：

| # | Bug | 优先级 | 修复 |
|---|-----|--------|------|
| B2 | `align-items: normal` 错误回退为 STRETCH | P1 | 显式映射 normal → FLEX_START |
| B1 | `display: inline-flex/inline-grid` 映射为 Block | P1 | 显式分支 inline-flex → Flex, inline-grid → Grid |
| B3 | `box-sizing` 默认值用 taffy 的 BorderBox 而非 CSS 初始值 ContentBox | P2 | 初始化改为 ContentBox + 未知值回退到 ContentBox |
| B4 | `gap: 10px 20px` 双值未解析 | P2 | 新增 extract_gap_pair 正确分离 row-gap/column-gap |
| B7 | 集成测试断言过弱（c2.x >= c1.x） | P2 | 收紧为 c2.x ~= c1.x + c1.width |
| B8 | 集成测试 margin 断言过松（x >= 19.0） | P3 | 收紧为 x == 20.0 ± 1.0 |
| B5 | `flex-grow`/`flex-shrink` 接受负值 | P3 | 添加 >= 0.0 检查 |

**B6（`flex` 简写未实现）**：✅ 已修复（2026-08-09）——cascade 在 collect 阶段展开 `flex` 简写为 grow/shrink/basis（cascade `d6d7208`），布局端到端测试验证（layout `44b9b1d`）。

## Phase 2 子阶段 2 — Selectors Level 4 ✅

按 [phase2-selectors-sp1-to-sp8.md](.trae/archive/phase2-selectors-sp1-to-sp8.md) 8 个 SP batch 全部完成，覆盖 [Selectors Level 4](https://drafts.csswg.org/selectors-4/) §3 / §4 / §5 / §6 / §13 / §14 / §15 / §17 / §18。

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

## Phase 2 子阶段 3 — CSS Values Module ✅

按 [2026-07-22-css-values-module.md](docs/plans/2026-07-22-css-values-module.md) 7 个 CV batch 全部完成，覆盖 [CSS Values Level 4](https://drafts.csswg.org/css-values-4/) §4 / §5 / §6 / §8 / §9 + [CSS Variables Level 1](https://drafts.csswg.org/css-variables-1/) §2 / §3。

**设计原则**：解析与求值分离——本 crate 只构建类型化 AST，不做数值计算和 var() 替换求值（留到子阶段 5 Cascade）。

| CV   | 内容 | 规范 | commit |
|------|------|------|--------|
| CV-0a | `CssTokenizer` 加 `next_token_with_span` + `position()` | 工程基础设施 | — |
| CV-0b | `TokenStream::with_source` + `source_slice` + §5.5.6 `original_text` | CSS Syntax §5.3 / §5.5.6 | — |
| CV-1 | 数值类型：Length/Percentage/Number/Integer/Angle/Time/Frequency/Resolution/Ratio | §4.4-§4.7, §5, §6 | `084809a` |
| CV-2 | 文本类型：Keyword/CustomIdent/DashedIdent/CssString/Url | §3 | `c18c153` |
| CV-4 | VarReference 解析（name + fallback，支持嵌套 var()） | CSS Variables §3 | `6edd8f9` |
| CV-3 | MathExpression AST + calc/min/max/clamp 递归下降解析 | §9 | `0c3f519` |
| CV-5 | ValuesGrammar impl Grammar + ToCss 序列化 | §5.4.1, §8.1, §9.7 | `44e80cb` |
| CV-6 | lib 顶层 API + doctest | — | `d9a8a9b` |

测试矩阵：

| 测试文件 | 测试数 | 覆盖内容 |
|---------|------|---------|
| `tests/numeric.rs` | 33 | 9 个数值类型（正/负/单位/范围检查） |
| `tests/textual.rs` | 25 | 5 个文本类型（keyword/ident/string/url + CSS-wide keyword 排除） |
| `tests/math.rs` | 36 | calc/min/max/clamp + 常量 + 嵌套 + 错误处理 |
| `tests/var.rs` | 12 | var() 解析（name/fallback/嵌套/空 fallback） |
| `tests/integration.rs` | 33 | Grammar hook 入口 + 序列化 roundtrip |
| `src/lib.rs` doctests | 9 | 顶层 API doctest |
| **总计** | **148** | 全部通过 |

架构：

- **numeric.rs** — 9 个数值类型，带单位枚举 + §4.4 范围检查。`single_non_ws_cv` 辅助函数过滤 whitespace。
- **textual.rs** — 5 个文本类型。`CSS_WIDE_OR_RESERVED` 排除 initial/inherit/unset/default/none。
- **math.rs** — `MathExpression` 枚举（Length/Percentage/Number/Constant/Var/Negate/Sum/Product/Quotient/Min/Max/Clamp）+ `CalcParser` 递归下降解析器（calc-sum → calc-product → calc-value，左结合）。
- **var.rs** — `VarReference { name, fallback }`，`from_function` 支持 calc() 内嵌套 var()。
- **grammar.rs** — `ValuesGrammar` impl `Grammar` trait，`ValueKind` 16 变体，`CssValue` 包装枚举。
- **serialize.rs** — `ToCss` trait + 14 类型实现。MathExpression 序列化遵循 §9.7（`+` 两侧空格，`*`/`/` 无空格，Negate → `(-1 * expr)`）。

延后项（推迟到子阶段 5 Cascade）：

- calc() 数值计算（需要布局上下文解析百分比）
- min()/max()/clamp() 比较
- var() 替换求值（§3 的 4 步算法，需要元素上下文 + 循环检测）
- 三角/指数/round/mod/rem/sign/abs（CSS Values 4 新增，布局用不到）

crate 已剥离为独立 git 仓库（[muskitty-dev/muskitty-css-values](https://github.com/muskitty-dev/muskitty-css-values)）并发布到 crates.io v0.1.0（2026-07-24）。

## Phase 2 子阶段 4 — CSSOM ✅

按 [2026-07-22-cssom.md](docs/plans/2026-07-22-cssom.md) 5 个 OM batch 全部完成，覆盖 [CSSOM](https://drafts.csswg.org/cssom-1/) §3 / §8.1 / §8.4 / §8.5 / §8.6。

**设计原则**：单向转换——语法→语义。css-parser 产出语法层 `Stylesheet`（CSS Syntax §5.2），CSSOM crate 将其转换为 CSSOM 语义层 `CssStyleSheet`（§8.1）。转换后 CSSOM 树独立存在，不反向引用 css-parser 的 `Stylesheet`，避免生命周期耦合。

| OM | 内容 | 规范 |
|----|------|------|
| OM-1 | crate 骨架 + `CssDeclaration` + `CssStyleDeclaration` | §8.5 / §8.6 |
| OM-2 | `CssRule` 枚举 + 8 种 rule 类型（Style/Import/Media/Namespace/Supports/LayerBlock/LayerStatement/Container）+ `OtherRule` fallback | §8.4 |
| OM-3 | `CssStyleSheet` 顶层容器 + 元数据（location/media/title/alternate/disabled） | §8.1 |
| OM-4 | 从 css-parser `Stylesheet` → CSSOM `CssStyleSheet` 单向转换层；at-rule 按 name 分发（import/media/namespace/supports/layer/container/other） | §8.4 / §8.6 |
| OM-5 | 序列化（§3 serializing idioms + §8.4-§8.6 rule/declaration/block 序列化）+ `ToCss` trait | §3 / §8.4-§8.6 |

测试矩阵：

| 测试文件 | 测试数 | 覆盖内容 |
|---------|------|---------|
| `src/declaration.rs` (单元) | 10 | CssDeclaration + CssStyleDeclaration CRUD + cascade 语义 |
| `src/rule.rs` (单元) | 13 | CssRule 枚举 + type_id + has_child_rules + 各 rule 类型 |
| `src/stylesheet.rs` (单元) | 5 | CssStyleSheet 容器 + iter + clone |
| `src/serialize.rs` (单元) | 13 | §3 idioms + 数字格式化 + declaration/block/rule 序列化 |
| `tests/convert.rs` (端到端) | 20 | parse → convert → 验证结构（所有 rule 类型 + 嵌套 + 边界） |
| `tests/integration.rs` (roundtrip) | 19 | parse → convert → serialize → 验证输出 |
| `src/lib.rs` doctests | 1 | 顶层 API |
| **总计** | **81** | 全部通过 |

架构：

- **declaration.rs** — `CssDeclaration { name, value: Vec<ComponentValue>, important }`（§8.5）+ `CssStyleDeclaration { declarations, readonly }`（§8.6）。`get_property` 返回最后一个匹配（cascade 语义）。
- **rule.rs** — `CssRule` 枚举（9 变体），用 enum 而非 trait 对象（值语义、pattern matching、避免 `Rc<RefCell<>>`）。`type_id` 返回 §8.4 的 rule type 常量。
- **stylesheet.rs** — `CssStyleSheet { location, media, title, alternate, disabled, css_rules }`。省略 DOM 集成字段（parent/owner node/origin-clean 等）。
- **convert.rs** — `from_stylesheet(&Stylesheet) -> CssStyleSheet`。at-rule 按 name 分发；`@import` 从 prelude 提取 href（string/url）+ media；`@layer` 根据 block 有无决定 LayerBlock/LayerStatement；嵌套裸声明（`Rule::Declarations`）合并到父 style 块。
- **serialize.rs** — `ToCss` trait + §3 serializing idioms（`serialize_identifier`/`serialize_string`/`serialize_url`）。Token/ComponentValue/Function/SimpleBlock 序列化。CssRule 用 match 分发到各 rule 类型序列化。

关键设计决策：

- **枚举 vs trait 对象**：选枚举。CSSOM rule 类型是规范固定的集合，无需开放扩展；枚举值语义、pattern matching 清晰、避免所有权复杂度。
- **嵌套裸声明处理**：CSS nesting 中 `Rule::Declarations`（§5.5.5）简化合并到父 `CssStyleRule.style`，不实现 `CSSNestedDeclarations`（推迟）。
- **Declaration value 存 `Vec<ComponentValue>`**：不做值类型化（那是 muskitty-css-values 的工作），CSSOM 层只关心声明结构。

延后项：

- mutation API（insertRule/deleteRule/setProperty）— Cascade 只读
- CSSStyleSheet construction（JS API）— DOM 集成阶段
- shorthand 序列化合并 — 需要属性数据库，子阶段 5 Cascade
- CSSKeyframesRule / CSSFontFaceRule / CSSPageRule — 按需
- MediaList 接口 — 简化为 `Vec<ComponentValue>`，子阶段 5
- computed flag / owner node / updating flag — DOM 集成阶段
- CSSNestedDeclarations — 嵌套裸声明暂合并到父 style

crate 已剥离为独立 git 仓库（[muskitty-dev/muskitty-cssom](https://github.com/muskitty-dev/muskitty-cssom)）并发布到 crates.io v0.1.0（2026-07-24）。

## Phase 2 子阶段 5 — Cascade + Computed Values ✅

按计划文档 [docs/plans/2026-07-23-cascade.md](docs/plans/2026-07-23-cascade.md) 推进，7 个批次（CC-1 ~ CC-7）全部完成。规范来源：[CSS Cascading and Inheritance Level 5](https://www.w3.org/TR/css-cascade-5/)（本地 `d:\csswg\css-cascade-5\Overview.md`）。

### 批次

| 批次 | 内容 | 规范 | commit |
|------|------|------|--------|
| CC-1 | 前置：CSSOM `Origin` 加 `#[derive(Default)]`（默认 `Author`）；新建 `muskitty-cascade` crate 骨架（依赖 selectors v0.1.0 `features=["dom"]`） | — | `58ac2bb` |
| CC-2 | 属性注册表（20 个内置 CSS 属性：`PropertyDefinition` / `PercentageBasis` / `lookup_property`） | §4.1 / §7.1 | `58ac2bb` |
| CC-3 | Filtering：`collect_declared_values` 递归遍历 stylesheet + 选择器匹配 → `DeclaredValue` 列表 | §5 | `36f0c65` |
| CC-4 | Cascade 排序：按 §6.1 准则 1/4/6/7（Origin×Importance / Element-attached / Specificity / Order）排序 | §6.1 | `e73ba1a` |
| CC-5 | Defaulting：`apply_defaulting` 实现 `initial`/`inherit`/`unset` 三种 CSS-wide 关键字 + 无声明时的继承/初始值回退 | §7.3.1-§7.3.3 / §7.1-§7.2 | `69c520d` |
| CC-6 | Computed Value：`compute_value` 解析相对长度（em/rem/vh/vw/vmin/vmax → px）、百分比（font-size 基于 parent font-size）、`var()` 替换（自定义属性查找 + 递归 fallback） | §4.4 | `13d66be` |
| CC-7 | 端到端集成测试：完整 pipeline `DOM + CssStyleSheet[] → filter → cascade → defaulting → compute` | 全链路 | `aab7395` |

### 测试矩阵

| 测试文件 | 测试数 | 覆盖内容 |
|---------|------|---------|
| `src/registry.rs` (单元) | 7 | 属性注册表 lookup / case-insensitive / 继承标志 / 非继承标志 / percentage basis / 属性数量 |
| `src/filter.rs` (单元) | — | Filtering 内部逻辑（通过 CC-3 `tests/filter.rs` 覆盖） |
| `tests/filter.rs` (集成) | 14 | 选择器匹配 → DeclaredValue 收集（type/class/id/important/media/nesting/specificity/origin/layer/import/multi-sheet） |
| `src/cascade.rs` (单元) | 9 | §6.1 完整排序顺序（origin×importance / style attr / specificity / order） |
| `src/defaulting.rs` (单元) | 13 | initial/inherit/unset 关键字 + 无声明继承/初始值 |
| `src/compute.rs` (单元) | 12 | em/rem/vh/vw/vmin/vmax / font-size 百分比 / var() 替换 + fallback / 混合值 |
| `tests/integration.rs` (端到端) | 15 | 完整 pipeline：single rule / specificity / important / order / defaulting（initial/inherit/unset/no-decl）/ em / % / author vs UA / important UA vs important author / 多属性 / var() 全链路 / 非匹配选择器 |
| `src/lib.rs` doctests | 1 | 顶层 API 编译验证 |
| **总计** | **71**（7 + 14 + 9 + 13 + 12 + 15 + 1） | 全部通过 |

> 工作区回归：`cargo test --workspace` 全部通过（300+ 测试，含 css-values 148 / cssom 81 / cascade 71 / selectors / html5-parser / dom / css 等）。

### 架构（单向数据流）

```text
DOM (DomElement) + CssStyleSheet[]
        │
        ▼  filter::collect_declared_values  (§5)
   Vec<DeclaredValue>
        │  字段：property / value(Vec<ComponentValue>) / important /
        │        origin / specificity / order / from_style_attr
        ▼  cascade::cascade_for_element  (§6.1)
   HashMap<String, Vec<DeclaredValue>>  (按属性分组，每组按 sort key 降序)
        │  cascade::cascade_winner  → 取首项
        ▼
   Option<&DeclaredValue>  (cascaded value)
        │  defaulting::apply_defaulting  (§7)
        │  - 检测 CSS-wide 关键字：initial / inherit / unset
        │  - 无声明：继承属性 → parent_computed；非继承属性 → initial_value
        ▼
   ComputedValue  (Keyword | Raw(Vec<ComponentValue>))
        │  compute::compute_value  (§4.4)
        │  - resolve_dimension: em/rem/vh/vw/vmin/vmax → px
        │  - resolve_percentage: font-size 基于 parent_font_size
        │  - resolve_var: var(--name, fallback) 递归替换
        ▼
   ComputedValue::Resolved(Vec<ComponentValue>)
```

### 关键数据结构

- **`DeclaredValue`**（`src/style.rs`）：Cascade 输入项。携带 `property: String` / `value: Vec<ComponentValue>` / `important: bool` / `origin: Origin` / `specificity: Specificity` / `order: usize` / `from_style_attr: bool`。
- **`ComputedValue`**（`src/style.rs`）：Cascade 输出。三态枚举：
  - `Keyword(String)` — CSS-wide 关键字解析结果或初始值关键字（如 `"black"`）。
  - `Raw(Vec<ComponentValue>)` — 已 defaulting 但未 compute 的中间态。
  - `Resolved(Vec<ComponentValue>)` — 完整 computed value（单位已解析、var() 已替换）。
- **`ComputeContext<'a>`**（`src/compute.rs`）：computed value 解析上下文。字段：`parent_font_size: f64` / `root_font_size: f64` / `viewport_width: f64` / `viewport_height: f64` / `custom_properties: &'a HashMap<String, Vec<ComponentValue>>`。提供 `new(custom_properties)` 构造器（font-size 默认 `16.0`，viewport 默认 `1920.0×1080.0`）。
- **`PropertyDefinition`**（`src/registry.rs`）：`{ name: &'static str, initial_value: &'static str, inherited: bool, percentages: PercentageBasis }`。20 个内置属性覆盖 color/background-color/font-*/margin*/padding*/display/opacity/width/height/visibility/text-align 等。
- **`Origin`**：从 `muskitty-cssom` 重导出（`src/origin.rs`），`UserAgent`/`User`/`Author`，`#[derive(Default)]` 默认 `Author`。

### Cascade 排序键设计（§6.1）

`cascade_sort_key` 返回 `(u8, u8, (u32,u32,u32), usize)` 元组，按 §6.1 准则降序排列（用 `Reverse` 包裹）：

| 准则 | 实现 | 备注 |
|------|------|------|
| 1. Origin × Importance | `(origin, important)` → u8（UA+important=6 / User+important=5 / Author+important=4 / Author=3 / User=2 / UA=1） | 已实现 |
| 2. Context (Shadow DOM) | — | 推迟（无 Shadow DOM 支持） |
| 3. Scope | — | 推迟 |
| 4. Element-Attached Styles | `from_style_attr: bool` → u8 (1/0) | 已实现 |
| 5. Layers | — | 推迟（@layer 仅作为容器透传，未参与排序） |
| 6. Specificity | `(a, b, c)` 三元组 | 已实现 |
| 7. Order of Appearance | `order: usize` | 已实现 |

### 关键设计决策

- **输入用 `Vec<ComponentValue>` 而非类型化值**：Cascade 层只关心声明的结构关系（谁赢、是否继承），值类型化是 CSS Values 层的工作。保持与 css-parser/cssom 的 `Vec<ComponentValue>` 一致。
- **Computed Value 三态**：`Keyword` / `Raw` / `Resolved` 区分 defaulting 后的关键字结果、未 compute 的原始值、已 compute 的解析值。下游（layout）根据变体决定是否进一步处理。
- **var() 解析依赖 `ComputeContext.custom_properties`**：Cascade 本身不收集自定义属性（那是 §4.2 specified value 阶段的工作，需要完整的 inheritance 传递），CC-6 假设 custom_properties 已由上层准备好并传入 context。简化但实用。
- **Percentage basis**：`font-size` 实现 `PercentageBasis::ParentFontSize` 和 `RootFontSize`（均解析为 px）。其他属性的百分比保留原样（layout 阶段处理），因为 percentage basis 依赖 layout 上下文（如 width 基于 containing block）。
- **Filtering 简化**：`collect_declared_values` 无条件收集 `@media`/`@layer`/`@supports`/`@container` 及 `Other` child rules 内的规则（不做 media query 求值或 layer 优先级排序），仅作为容器透传。理由：media query 和 layer 的语义是改变 cascade 优先级，应在 cascade 排序层处理；filtering 只负责"哪些声明匹配此元素"。
- **`@import` / `@namespace` 跳过**：不参与 cascade（import 应在加载时展开为独立 sheet；namespace 影响选择器匹配，selectors 层已处理）。
- **Origin 从 cssom 重导出**：避免在 cascade crate 重新定义 Origin 枚举，保持单一来源。cssom 的 `Origin` 加 `#[derive(Default)]` 默认 `Author`。

### 延后项

- ~~**Inline `style` 属性收集**~~ ✅ 已完成（2026-08-01）：`filter.rs::collect_from_style_attr` 从 DOM `style` 属性解析声明，`from_style_attr = true`，specificity 归零、由准则 4 单独排序。§6.1 准则 4 在真实 pipeline 中生效。
- ~~**`muskitty-css-values` 死依赖**~~ ✅ 已清理：cascade `Cargo.toml` 不再声明 `muskitty-css-values`，直接用 `muskitty-css` 的 `ComponentValue`/`Token`。
- **§6.1 准则 2 Context（Shadow DOM）**：无 Shadow DOM 支持，推迟。
- **§6.1 准则 3 Scope**：`@scope` 规则未实现，推迟。
- **§6.1 准则 5 Layers**：`@layer` 排序优先级未实现（仅作为容器透传规则）。已在 `docs/audit-2026-08-08-full-scan.md` 修复计划 B8（层号跟踪 + 5 元排序键）中，待实施。
- **§7.3.4 `revert` / §7.3.5 `revert-layer`**：依赖 Origin/Layer 完整支持。已在 audit 修复计划 B7（当"无 cascaded value"处理）中，待实施。
- **§4.2 Specified Value 阶段**（自定义属性 inheritance + var() substitution 全局解析）：CC-6 简化为 context 传入，完整实现需要 DOM 树遍历 + 自定义属性 cascade。
- **§4.5 Used Value / §4.6 Actual Value**：依赖 layout（containing block、viewport），推迟到 Phase 3。
- **Animation origin**（§6.1 准则外的 animation declarations）：未实现。
- **Shorthand 展开为 longhand**：未实现（如 `background: red` 不展开为 `background-color: red`）。需要属性数据库。

crate 已剥离为独立 git 仓库（[muskitty-dev/muskitty-cascade](https://github.com/muskitty-dev/muskitty-cascade)），未发布到 crates.io。
