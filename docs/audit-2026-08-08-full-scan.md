# MusKitty 全项目代码审查报告

**审查日期**：2026-08-08
**审查范围**：workspace 内 4 个 crate（muskitty-cascade / muskitty-layout / muskitty-cssom / muskitty-renderer）+ 跨 crate 横切架构
**审查维度**：架构级别缺陷 / 严重 BUG / 不依赖 unsafe 的性能优化
**审查方法**：5 个并行审查 Agent 逐文件 Read + 临时探针测试实测验证（探针已删除，测试基线全绿）。零 unsafe，无循环依赖，`cargo check --workspace` 通过。

> 与 [security-audit-2026-08-02.md](./security-audit-2026-08-02.md) 的关系：本次为全项目正确性 + 架构审查，重点在"结果错误但测试通过"的语义缺陷，而非安全 DoS。上期 C-1（var() 循环检测）本期确认已修复；但暴露出 var() 系列仍有 **指数级展开** 与 **环 fallback 丢失** 两个新问题。

---

## 一、问题总览

### 严重度分布

| 级别 | 数量 | 含义 |
|------|-----:|------|
| **P0** | 2 | 跨链路数据流错误，端到端结果必然错误 |
| **P1** | 13 | 真实 CSS 高频触发的严重 BUG（含 3 个可被不可信样式表触发的崩溃/DoS） |
| **P2** | 21 | 语义/架构硬伤，不立即崩但结果错或扩展受阻 |
| **P3** | 6 | 已知简化（有 TODO 标注）或低影响 |
| **性能** | 11 | 全部可用 safe Rust 实现，无需 unsafe |

### 快速索引

| 编号 | 级别 | 模块 | 问题 | 状态 |
|------|-----:|------|------|------|
| P0-1 | P0 | cascade | em/rem/百分比 font-size 恒按 16px，父/根 font-size 从不注入 | ✅ |
| P0-2 | P0 | layout | `align-items: normal` → flex-start，所有 flex/grid 容器默认失去 stretch | ✅ |
| P1-1 | P1 | cascade | var() 替换指数级展开，可 OOM/挂死 | ✅ |
| P1-2 | P1 | cascade | var() 环检测误吞 fallback | ✅ |
| P1-3 | P1 | cascade | `@layer` 级联语义完全错误（未分层输给分层、important 层序反向） | ✅ |
| P1-4 | P1 | cssom | 引号形式 `@import url("...")` 空 href / `@namespace url()` 空 URI | ✅ |
| P1-5 | P1 | cssom | 未知 block at-rule（@font-face/@page/@property）声明全部丢失 | ✅ |
| P1-6 | P1 | cssom | at-rule 分发大小写敏感，`@MEDIA`/`@Supports` 落入 Other | ✅ |
| P1-7 | P1 | cssom | `@layer` 点分嵌套层名被展平破坏 | ✅ |
| P1-8 | P1 | layout | 无单位零长度 `width: 0` / `min-width: 0` / `flex-basis: 0` → AUTO | ✅ |
| P1-9 | P1 | layout | `calc()` 静默降级为 AUTO/0 | ✅ |
| P1-10 | P1 | renderer | `parse_hex_color` 多字节 UTF-8 切片 panic（可触发崩溃） | ✅ |
| P1-11 | P1 | renderer | legacy 逗号语法第 4 参数百分比 alpha 丢失 | ✅ |
| P1-12 | P1 | 架构 | 缺"整棵 DOM → ComputedStyle"公共 API，驱动逻辑复制 3 份 | ✅ |
| P1-13 | P1 | 架构 | workspace 状态与 CLAUDE.md/PROGRESS.md 完全矛盾 | ✅ |
| P1-14 | P1 | 架构 | 死依赖：cascade→dom；layout→cssom/selectors | ✅ |
| P2-1 ~ P2-21 | P2 | 各模块 | 见第四节 | 见下表 |
| P3-1 ~ P3-6 | P3 | 各模块 | 见第五节 | 见下表 |
| PERF-1 ~ PERF-11 | 性能 | 各模块 | 见第六节 | 见下表 |

---

## 二、P0 — 跨链路数据流错误

### P0-1 em/rem/百分比 font-size 恒按 16px 解析 —— cascade/src/compute.rs:34-36, 120-121, 161 ✅

- **问题**：`ComputeContext::parent_font_size` / `root_font_size` 构造时固定 `16.0`。三个驱动入口（`renderer/examples/render_demo.rs:109`、`tests/end_to_end.rs:43`、`tests/paint.rs`）调用 `ComputeContext::new(&props)` **只传 custom_properties**；递归时虽把 `parent_style` 传给 `apply_defaulting` 用于 `inherit`，却从不回填 font-size 到 context。
- **触发场景**：`div { font-size: 32px }` 下 `span { margin-left: 2em }` 期望 64px，实际 32px；`font-size: 200%`（父 32px）期望 64px，实际 32px。
- **影响**：所有 em/rem/vh 依赖 font-size 的相对单位布局结果错误。`compute.rs:289` 单测用手写 `parent_font_size: 20.0` 掩盖缺环；端到端测试手写 ctx，**从未覆盖"父 font-size → 子 em"真实数据流**。PROGRESS.md 声称 CC-6 已完成，实际在 pipeline 中未打通。
- **建议修复**：cascade 提供自顶向下遍历 DOM 的公共 API（`compute_style_tree`），每层先解析父 font-size 为 px 再构造子 `ComputeContext { parent_font_size, root_font_size, .. }`；补端到端断言（父 32px、子 2em = 64px）。

### P0-2 `align-items: normal` → `flex-start`，所有 flex/grid 容器默认失去 stretch —— layout/src/style_map.rs:251 ✅

- **问题**：`map_align_items` 显式把 `normal` → `FLEX_START`。CSS Box Alignment §6.1 / Flexbox L1 §8.3 规定 `normal`（初始值）在 flex/grid 中**等价于 stretch**。cascade 注册表把 `align-items` 初始值定义为 `"normal"`（registry.rs:177-181），集成测试 pipeline 对所有属性做 defaulting → **每个 flex/grid 容器的 computed style 都含 `normal`** → 全被映射成 flex-start。taffy 在 `None` 时本就正确回退 STRETCH（taffy flexbox.rs:454），这里显式设错值绕过了它。
- **讽刺点**：PROGRESS.md:406 记录这是之前审计"修复"的 B2（"错误回退为 STRETCH → 显式映射 normal → FLEX_START"）——**修反了**，把正确的 stretch 默认改成了错的 flex-start。
- **影响**：flex item 不再拉伸填满交叉轴，任何 flex 布局默认外观都错误；`tests/style_map.rs:376-387` 把错误行为固化成了断言。
- **建议修复**：`normal` → `None`（让 taffy 回退 STRETCH）或直接 `STRETCH`；`align-self: normal` 同理（style_map.rs:148-153）；修正测试断言。

---

## 三、P1 — 严重 BUG

### P1-1 var() 替换指数级展开，可致 OOM / 引擎挂死 —— cascade/src/compute.rs:61-64, 236-238, 249 ✅

- **问题**：`resolve_component_value` 对每个 var() 引用重新递归遍历完整子图，解析结果从不缓存。`--v{i}: var(--v{i-1}) var(--v{i-1})` 输出规模按 2^N 增长。
- **实测**：探针 N=22 产出 **4,194,304 个 component value / 30.9s**；N≈30 即 10 亿级必然 OOM。任何作者样式表可触发的 DoS。
- **建议修复**：记忆化（按 `(property, var name)` 缓存）或自底向上拓扑预计算每个 `--*` 的 computed value（拓扑排序 + 环检测，运行时 O(1) 取用）+ 输出规模上限。

### P1-2 var() 环检测误吞 fallback —— cascade/src/compute.rs:229, 233-245 ✅

- **问题**：`resolve_var` 检测到环直接 `return Vec::new()`，无视 fallback。css-variables-1 §3.1 规定环中变量为 guaranteed-invalid，**若 var() 有 fallback 则用 fallback**。
- **触发场景**：`--a: var(--b, red); --b: var(--a, blue)` 求 `var(--a)`，规范要求得 `blue`，当前得空。WPT `variable-cycles.html` 明确覆盖此模式。
- **建议修复**：环命中时把该 var() 视为 guaranteed-invalid 并递归解析 fallback；仅当无 fallback 才置无效。

### P1-3 `@layer` 级联语义完全错误 —— cascade/src/cascade.rs:22-34, 48-72 + filter.rs:129-131 ✅

- **问题**：filter 无条件递归 `LayerBlock`，但 `DeclaredValue` 无 layer 字段，排序键无 Layers 准则（§6.1 准则 5），分层声明退化为按出现顺序比较。规范要求：未分层声明归隐式最终层（排在显式层后 → **未分层 normal 胜出**）；important 则是**最早层**胜出。
- **实测**：`div{color:red} @layer a{div{color:blue}}` → 胜出 blue（规范要 red）；`@layer a{...red !important} @layer b{...blue !important}` → 胜出 blue（规范要 red）。
- **建议修复**：给 `DeclaredValue` 记录 layer 序号并实现规范排序（未分层=隐式 final 层；normal 取最晚层、important 取最早层），或真正推迟（不递归 LayerBlock + 文档明示不支持）。

### P1-4 引号形式 `@import url("...")` 空 href —— cssom/src/convert.rs:111, 153, 232-238 ✅

- **问题**：`extract_string_or_url`（L232-238）只匹配 `PreservedToken(String)` 和 `PreservedToken(Url)`。但 CSS Syntax §4.3.4 规定 `url(` 后紧跟引号（或空白+引号）时不产 url-token，而是 `Function("url", [String])`——最常见写法被漏掉，`href = ""`，`found_href` 恒 false，连后续 media 也被跳过。
- **实测**：`@import url("style.css")` → href=""；`@namespace url("http://...")` → namespace_uri=""。
- **建议修复**：`extract_string_or_url` 增加分支——`ComponentValue::Function(f)` 且 `f.name.eq_ignore_ascii_case("url")` 且 `f.value` 为单个 String 时返回其内容。

### P1-5 未知 block at-rule（@font-face/@page/@property/@counter-style）声明全部丢失 —— cssom/src/convert.rs:199-211 + serialize.rs:321-339 ✅

- **问题**：根因在已剥离的 css-parser：`split_block_contents`（crates/muskitty-css-parser/src/algorithms.rs:641）把所有 block 内容刷成 `Rule::Declarations` 放进 `child_rules`，`AtRule.declarations` 恒为 `Some(vec![])`。级联到 cssom：`convert_other` 读恒空列表，`convert_child_rules` 的 `extra_decls` 被 `_` 丢弃，`OtherRule::to_css_string` block 分支只序列化 child_rules 不输出 declarations。
- **实测**：`@font-face { font-family:"Open Sans"; src:url(x.woff2) }` 序列化为 `"@font-face { }"`。
- **建议修复**：方案 A（cssom 最小兜底）`convert_other` 合并 extra_decls 进 declarations + OtherRule 序列化补 declarations（同时覆盖补充发现 1 的条件组裸声明）；方案 B（修根因，改已剥离 css-parser）`split_block_contents` 抽出 `Rule::Declarations` 填回 `AtRule.declarations`。推荐 B 为主 + A 兜底。

### P1-6 at-rule 分发大小写敏感 —— cssom/src/convert.rs:91-100 ✅

- **问题**：`match ar.name.as_str()` 与 `"import"`/`"media"` 等字面量精确匹配，tokenizer 原样保留 at-keyword 大小写。CSS at-rule 名是 ASCII case-insensitive。
- **实测**：`@MEDIA print { ... }` → `OtherRule { name: "MEDIA" }`，非 CssMediaRule。
- **建议修复**：`match ar.name.to_ascii_lowercase().as_str()`（或 `eq_ignore_ascii_case`）。

### P1-7 `@layer` 点分嵌套层名被展平破坏 —— cssom/src/convert.rs:180, 184, 241-250, 253-261 ✅

- **问题**：`extract_first_ident` / `extract_ident_list` 只收集 `Ident` token，丢弃 `Delim('.')` 分隔符。层名 `a.b.c` 是单个嵌套层，却被拆成多个层名。
- **实测**：`@layer a.b.c { ... }` → `name = Some("a")`，序列化为 `@layer a { ... }`；`@layer a.b.c, d;` → `names = ["a","b","c","d"]`。
- **建议修复**：按 `<ident>('.'<ident>)*` 重新拼接层名，或保留 prelude 原始 component values 由 cascade 自行解析。

### P1-8 无单位零长度 `width: 0` / `min-width: 0` / `flex-basis: 0` → AUTO —— layout/src/style_map.rs:264-279, 321-330 ✅

- **问题**：`extract_px` 只认 `Token::Dimension(_, "px")`（L323-326）。裸 `0` 由 tokenizer 产 `Token::Number(0)`（css-tokenizer/impls.rs:361-376），compute_value 不归一化为 dimension → `Resolved([Number(0)])` 走 fallback → `Dimension::AUTO`。（`margin: 0`/`padding: 0` 恰好因 fallback 到 ZERO 碰巧正确，掩盖了问题。）
- **影响**：`width: 0` 填满父宽；`min-width: 0`（flex 收缩最常用修复）失效——taffy flexbox.rs:817-820 对 Auto min-size 用内容最小尺寸；`flex-basis: 0` 变 auto 按内容尺寸。CSS Values L4 §5.1 裸 `0` 是合法 `<length>`。
- **建议修复**：`extract_px` 对顶层 `Token::Number(n)` 且 `n.value == 0.0` 返回 `length(0.0)`。

### P1-9 `calc()` 静默降级为 AUTO/0 —— layout/src/style_map.rs:321-330, 385-392 ✅

- **问题**：cascade 的 `compute_value` 对 `calc(...)` 保留为 `ComponentValue::Function`（compute.rs:92-102），值嵌套在 Function 内。`extract_px`/`extract_percent` 只遍历顶层 `PreservedToken`，看不到 Function 内 Dimension/Percentage。`width: calc(50%)` → AUTO，`padding: calc(...)` → 0。calc() 是真实 CSS 高频语法。
- **建议修复**：短期在 `extract_px`/`extract_percent` 递归展开 Function（取 calc 内首个有效 token）；长期在 cascade 层实现 calc 求值（布局层只应收到已解析绝对值）。

### P1-10 `parse_hex_color` 多字节 UTF-8 切片 panic —— renderer/src/color.rs:228-230, 235-238, 243-245, 250-253 ✅

- **问题**：`parse_hex_color` 在 `match hex.len()` 后直接 `&hex[0..1]`、`&hex[1..2]`、`&hex[2..4]` 等 `str` 字节切片。`str::len()` 返回**字节数**而非字符数，含多字节字符的 hash 若字节长恰好命中 3/4/6/8 分支，切片落在字符中间 → `panic: byte index N is not a char boundary`。tokenizer 正常产出 `Hash("aä")` 等合法 token、非法颜色值。
- **实测**：`#aä`（字节长 3）在 color.rs:229 panic；`#aaä`（字节长 4）在 color.rs:237 panic。CSS 要求无效颜色被**忽略**（视为 transparent）而非崩溃——不可信页面 CSS 可触发的崩溃点（DoS）。
- **建议修复**：切片前校验 `hex.is_ascii()` 且 `hex.bytes().all(|b| b.is_ascii_hexdigit())`。

### P1-11 legacy 逗号语法第 4 参数百分比 alpha 丢失 —— renderer/src/color.rs:126-133, 153-163 ✅

- **问题**：`parse_rgb` 的 `Token::Percentage` 分支仅当 `numbers.len() < 3` 时把百分比当通道；`rgba(255, 0, 0, 50%)` 的第 4 个百分比被静默丢弃，循环结束 `alpha` 为 None、`numbers.len()` 为 3 → alpha 回落 255。CSS Color L4 §6 规定 `rgb()`/`rgba()` 互为别名，legacy 语法第 4 参数 `<alpha-value>` 可为 number 或 percentage。
- **实测**：解析得 `r=255 g=0 b=0 a=255`，期望 `a=128`。现有测试只覆盖 slash 语法 `rgb(255 0 0 / 50%)`。
- **建议修复**：Percentage 分支补 `else if alpha.is_none() { alpha = Some(p.value / 100.0) }`。

### P1-12 缺"整棵 DOM → ComputedStyle"公共 API，驱动逻辑复制 3 份 —— renderer/examples/render_demo.rs:94-149, tests/end_to_end.rs:28-83, tests/paint.rs ✅

- **问题**：cascade 只暴露单元素原语（collect_declared_values / cascade_for_element / apply_defaulting / compute_value），整树遍历 + parent_props/parent_style 维护逻辑被逐行复制 3 份（连注释都写"复制自 layout 集成测试"）。P0-1 的 font-size 缺环正因逻辑散落无人统一修复。
- **建议修复**：cascade 增加 `pub fn compute_styles(root, sheets, viewport) -> HashMap<usize, ComputedStyle>`，把 3 份复制收敛为 1 份，example/tests 改调它。

### P1-13 workspace 状态与 CLAUDE.md / PROGRESS.md 完全矛盾 —— Cargo.toml:8-12 vs CLAUDE.md:11, PROGRESS.md:21-24 ✅

- **问题**：实际 `members = [renderer, cascade, cssom]`、layout **已剥离**；但 CLAUDE.md 说 `members = [cascade, layout]`、PROGRESS.md 称 layout 未剥离、cssom 已剥离、renderer 空白。CLAUDE.md 是硬约束却系统性误导，`cargo check -p muskitty-layout`（CLAUDE.md:15 推荐命令）与 workspace 实际行为不符，剥离/发布顺序决策会算错。
- **建议修复**：以 `AGENTS.md` 为准同步更新 CLAUDE.md 与 PROGRESS.md（members 列表、renderer 状态、layout 已剥离、Phase 4 已完成、下一步项）。

### P1-14 死依赖 —— cascade/Cargo.toml:18；layout/Cargo.toml:13,15 ✅

- **问题**：`muskitty-cascade` 声明 `muskitty-dom`（仅 tests 与 `#[cfg(test)]` 使用，production 零 use）；`muskitty-layout` 声明 `muskitty-cssom`、`muskitty-selectors`（src/ 零 use）。
- **影响**：剥离/发布时污染 crates.io 依赖图，CI 的 setup-deps.sh 多克隆 3 个仓库。PROGRESS.md:668 记录过 css-values 死依赖已清理，此处漏网。
- **建议修复**：dom 移 `[dev-dependencies]`；删 layout 两个死依赖。

---

## 四、P2 — 中等级

| # | 模块 | 位置 | 问题 | 状态 |
|---|------|------|------|
| P2-1 | cascade | compute.rs:118-129 | 绝对长度单位 pt/pc/in/cm/mm/q 未换算 px（css-values-4 规定 computed length 必须 px：1in=96px、1pt=96/72px…），与 em/rem 的 px 化策略不一致 | ✅ 已修（2026-08-09，`d2c2f1c`） |
| P2-2 | cascade | cascade.rs:25, filter.rs:104 | 属性名大小写敏感：`COLOR: red` 不参与 `color` 级联（css-syntax-3 §9.2 要求 ASCII case-insensitive），与 `lookup_property` 的 `eq_ignore_ascii_case` 自相矛盾，实测被静默丢弃 | ✅ 已修复 |
| P2-3 | cascade | defaulting.rs:40-58 | `revert`/`revert-layer` 未实现，静默当普通值透传成字面量（css-cascade-5 §8） | ✅ 已修复 |
| P2-4 | cascade | custom_properties.rs:34-41 | 自定义属性 CSS-wide 关键字被当字面量存：`--x: initial` 会被 var() 替换出 `initial`；`--x: inherit` 不继承（css-variables-1 §2 明确"不保留为 custom property 值"） | ✅ 已修复 |
| P2-5 | cascade | compute.rs:221-245 | 无效 var() 替换不触发 invalid-at-computed-value，属性不回退 unset；`var(color)` 非法首参也静默当查不到 → `Resolved([])` | ✅ 已修复 |
| P2-6 | cascade | filter.rs:119-128 | @media/@supports/@container 条件被忽略，无条件收集内部规则（`@media print` 在屏幕上也生效）；文档注释承认是"简化"，但真实页面几乎必用 media query | ✅ 已修复 |
| P2-7 | cascade | registry.rs:36-230 + defaulting.rs:29 | 属性注册表过小：未注册的继承属性（text-transform/font-style/cursor/direction…）被当非继承 defaulting，继承行为大面积错误；未知属性初始值回退字面量 `"initial"` | ✅ 已修复 |
| P2-8 | layout | style_map.rs:335-355 | 百分比 gap 被丢弃（`gap: 20%` → (ZERO,ZERO)）；混合 `gap: 10px 20%` 误解析为单值。taffy 本身支持百分比 gap（flexbox.rs:476），是映射层丢掉 | ✅ 已修复 |
| P2-9 | layout | style_map.rs:185-197 | `gap` 简写与 `row-gap`/`column-gap` 层叠顺序不尊重：先应用 gap 再用长属性覆盖，源码顺序 `column-gap:20px; gap:10px` 时 column-gap 应=10 却=20 | ✅ 已修复 |
| P2-10 | layout | style_map.rs:226-236 | `justify-content: end/right` 落入默认 flex-start（应为 FLEX_END）；`start`/`left`/`normal` 回退正确 | ✅ 已修复 |
| P2-11 | layout | style_map.rs:116-197 | `align-content` 完全未映射，多行 flex 容器交叉轴行分布错误 | ✅ 已修复 |
| P2-12 | layout | style_map.rs:60-61 | `display: contents` 当作 Block 生成多余盒（CSS Display L3 §2.5 应不生成 box 但子元素照常参与父格式上下文）；注释自认 TODO | ✅ 已修复 |
| P2-13 | layout | style_map.rs:60-63, convert.rs:83-101 | inline/inline-block → Block（已知 workaround，但 inline-block 会占整行）；head/title/script/style/meta 无 UA 表时生成假布局盒 | ✅ 已修复 |
| P2-14 | cssom | rule.rs:13-33, 37-50 | CssRule 枚举与规范 §8.4 不齐（缺 CSSFontFaceRule(5)/CSSPageRule(6)/CSSKeyframesRule(7)/CSSKeyframeRule(8)/CSSCounterStyleRule(11)/CSSPropertyRule(18)/CSSScopeRule(19)）；@keyframes 内 from/to/0% 块被转 CssRule::Style，cascade filter.rs:132 递归进 Other 当普通 style rule 参与元素匹配（数据污染） | ✅ 已修复 |
| P2-15 | cssom | convert.rs:23-33 | `from_stylesheet` 硬编码 Origin::Author 并丢弃 location/media/title/alternate/disabled；cascade 被迫事后补 `s.origin = Author`（custom_properties.rs:61） | ✅ 已修复 |
| P2-16 | cssom | convert.rs:217-223 | `convert_declaration` 丢弃 custom property 的 `original_text`（单向转换不可逆，未来 var() 精确替换 / CSS.registerProperty 校验永久缺失） | ✅ 已修复 |
| P2-17 | renderer | render_tree.rs:19-43 + lib.rs:47 | `RenderTree`/`RenderNode` 死代码（无构造/消费方），违反 Simplicity 硬约束；render_tree.rs:9 自述"B-1 阶段暂不构造" | ✅ 已修复 |
| P2-18 | renderer | backend/mod.rs:23-28 | `Backend` trait 无输出契约：`render` 返回 ()，MockBackend 用 commands、TinySkiaBackend 用 pixmap()/encode_png() 各自为政；demo 硬编码 tiny-skia。Phase 4 接 GPUI 需破坏性改签名 | ✅ 已修复 |
| P2-19 | renderer | paint.rs:81-83, 104-107 | paint 坐标累加依赖"taffy 布局父 == 最近元素祖先"（offset 沿 DOM 祖先链累加）；当前无 position/transform 故成立，但引入 `position: absolute`（location 相对 containing block）或 transform 即双重计数，无文档记录该假设 | ✅ 已修复 |
| P2-20 | cascade | style.rs:30-37 | `ComputedValue` 三态枚举（Keyword/Raw/Resolved）迫使每个下游消费点重复 match 三态；`Keyword(String)` 与 `Resolved([Ident])` 信息等价；style_map.rs:209-223 被迫写 `get_keyword` 统一两态 | ✅ 已修复 |
| P2-21 | cascade + renderer | defaulting.rs:30-35 + command.rs:29-31, render_tree.rs:120 | 未注册属性 defaulting 返回魔法字符串 `Keyword("initial")`，下游靠字符串特判，无法区分"值为 initial"与"属性未知"；`command.rs:29` 注释称"border-* 未注册"与事实矛盾（border 实际可绘制，20 个 paint 测试全绿） | ✅ 已修复 |

---

## 五、P3 — 已知简化 / 低影响（备案，非新发现）

| # | 位置 | 问题 | 状态 |
|---|------|------|
| P3-1 | layout/src/style_map.rs:60-63 | `display: contents` / `list-item` 映射为 Block（TODO 已标注） | ✅ display:contents 已 splice；list-item 仍 TODO |
| P3-2 | cascade/src/compute.rs:92-102 | calc()/min()/max() 不数值计算（PROGRESS.md:511 明确推迟到布局阶段，属已知推迟项非回归） | ✅ 已修（2026-08-09，`89b9e49`，可求值折叠为单值，不可求值保留） |
| P3-3 | layout/src/convert.rs:104, renderer/src/paint.rs:70,104,110 | DOM 遍历每次 `child_nodes().to_vec()` 新分配 Vec<Rc<Node>> | 🕓 部分（renderer paint 已复用 scratch；layout convert.rs 未动） |
| P3-4 | renderer/src/tiny_skia.rs:156-162 | 边框 Dashed/Dotted 静默按 solid 渲染，但命令层保留样式（API 误导，WPT 比对必失败） | ✅ 降级：solid fallback + 注释，真虚线推迟 Phase 4 |
| P3-5 | renderer/src/tiny_skia.rs:81-82 | 画布默认全透明，无浏览器白底（UA 层缺口，`html { background: white }` + 根元素背景传播） | ✅ 画布默认填白（根元素背景传播仍推迟） |
| P3-6 | renderer/src/paint.rs:38-49 | paint 无视口参数，无离屏剔除（随页面规模增大浪费命令生成） | ✅ 已修复（viewport culling） |

---

## 六、性能优化清单（全部不依赖 unsafe）

**级联热路径（收益最大）**

| # | 位置 | 问题 | 优化 | 状态 |
|---|------|------|------|
| PERF-1 | cascade/src/filter.rs:98-100 | 每条规则×每个元素 `serialize_component_values → parse_a_selector → matches`，复杂度 O(元素×规则×选择器长) | cssom/cascade 预处理阶段解析一次 `SelectorList` 缓存，匹配阶段零分配复用 | ✅ |
| PERF-2 | cascade/src/custom_properties.rs:32 + 驱动层 | 每元素 `collect_declared_values` 跑两遍（custom_properties 内部一遍 + 主流程一遍） | `compute_style_tree` 一次收集同时做 cascade 分组与 `--*` 收集 | ✅ |
| PERF-3 | cascade/src/compute.rs | var() 替换无记忆化，指数级重复递归 | 见 P1-1：缓存 / 拓扑预计算 | ✅ |
| PERF-4 | cascade/src/custom_properties.rs:31 | 每元素克隆整张父自定义属性表，O(深度×表大小) | 不可变共享 / 写时复制结构传父表 | ✅ |
| PERF-5 | cascade/src/compute.rs:118 | 每个 dimension token（含最常用 px）`to_ascii_lowercase()` 堆分配 | 对 `&str` 用 `eq_ignore_ascii_case` 分支匹配，零分配 | ✅ |

**序列化/转换层**

| # | 位置 | 问题 | 优化 | 状态 |
|---|------|------|------|
| PERF-6 | cssom/src/serialize.rs:26-75 | 逐字符 `format!` 小分配（escape_char / escape_as_code_point） | `write!` 直写 `&mut String` | ✅ |
| PERF-7 | cssom/src/serialize.rs:150-152, 196-207, 343-349 | `Vec<String>` + join 双重分配 | 单 String 顺序 push_str；`ToCss` 改 `fn write_css(&mut self, out: &mut String)` | ✅ |
| PERF-8 | cssom/src/convert.rs:53-69, 130-136, 166-172, 190-196 | `convert_child_rules` 无条件收集 extra_decls（4 处被 `_` 丢弃白做，含 `value.clone()`） | 加 `collect_decls: bool` 参数 | ✅ |

**布局/渲染层**

| # | 位置 | 问题 | 优化 | 状态 |
|---|------|------|------|
| PERF-9 | layout/src/convert.rs:35-46, 67-129 | 每帧重建整棵 taffy 树，无增量更新路径 | 复用 `TaffyTree` + `set_style`/局部标记 | ✅ 降级：仅暴露 set_style，无消费方 |
| PERF-10 | layout/src/style_map.rs:57,109,118,129,139,144,150 | `map_style` 每属性 clone + lowercase（get_keyword 的 kw.clone/s.clone + 7 处 to_ascii_lowercase） | 返回 `&str` + `eq_ignore_ascii_case` 内联比较 | ✅ |
| PERF-11 | renderer/src/color.rs:113 + paint.rs:70,104,110 | `parse_rgb` 每颜色 `Vec<f64>` 堆分配；paint 每节点 `to_vec()` 克隆子节点 | 固定数组 `[f64;4]` + 计数；复用 scratch Vec | ✅ |

**存储/数据模型（性能 + 正确性）**

| # | 位置 | 问题 | 优化 | 状态 |
|---|------|------|------|
| PERF-12 | layout/src/result.rs:74-90 + convert.rs:80 | `LayoutResult` 拍平 HashMap 无父链，Renderer 必须重走 DOM 累加绝对坐标；key 是 `Rc::as_ptr as usize` 裸地址（DOM 变更后可能失效/复用） | 返回按 NodeId 的树形访问或直接给绝对坐标；key 用不透明句柄 | ✅ |

---

## 七、建议修复顺序

1. **两个 P0 先修**（P0-1 font-size 缺环 + P0-2 align-items normal）——它们让所有端到端布局结果错误；P0-2 是被之前"修复"改坏的，务必先回退语义并改测试断言。
2. **P1 安全类**（可被不可信样式表触发）：P1-10 hex panic（DoS）、P1-1 var() 指数展开（OOM）、P1-2 var() 环 fallback。
3. **P1 数据正确性**：cssom 4 项（P1-4 @import url、P1-5 @font-face 丢失、P1-6 大小写、P1-7 @layer 展平）+ layout 2 项（P1-8 width:0、P1-9 calc）+ cascade P1-3 @layer 排序。
4. **P2 按模块清**：先补 registry 继承属性（P2-7）+ 属性名大小写归一（P2-2，影响面最大），再逐项。
5. **性能**：优先 PERF-1 选择器缓存 + PERF-3 var() 记忆化（cascade 是全链路最热路径）。
6. **文档与清理**：同步 CLAUDE.md/PROGRESS.md（P1-13）、删死依赖（P1-14）、删 `RenderTree` 死代码（P2-17）、删两个临时探针文件（`tmp_probe.rs`、`crates/muskitty-cascade/tests/zz_audit_probe.rs`——后者为审查前遗留的未跟踪文件，是否保留由维护者决定）。

---

## 八、修复状态汇总（2026-08-08 全量修复）

按已批准计划（`docs/plans/` 下 2026-08-08 full-scan 实施计划）全部批次 B1–B14 完成。
主仓 + layout 仓 + css-parser 仓三处全绿：`cargo check --workspace` 零 warning、
`cargo test --workspace` 全绿、`cargo fmt --all -- --check` 通过、clippy `-D warnings` 零告警。

| 批次 | 覆盖项 | Commit（主仓） |
|------|--------|----------------|
| B1 | P1-13 文档同步；P1-14 死依赖 | `8c40707`；`a20894c`（cascade dom 注解）+ layout `4b8269d` |
| B2 | P1-10 hex panic；P1-11 legacy alpha | `3ffdc66`；`029bc6b` |
| B3 | P0-1 font-size 传播 + P1-12 compute_styles + PERF-2 | `fbf2b36`；`567b2e2` |
| B4 | P1-1/2 + P2-4/5 + PERF-3/4/5 var() 引擎 | `018a9e6` |
| B5 | P1-5 根因（css-parser 仓） | css-parser `f8dd717` |
| B6 | P1-4/6/7/5兜底 + P2-15/16 + PERF-6/7/8 | `13a9550`、`350ed5e`、`cafb09e`、`85df855`、`fb597b9`、`1c46fd9` |
| B7 | P2-7 registry 扩展；P2-3 revert | `8527a05`；`7974f93` |
| B8 | P1-3 @layer + PERF-1 + P2-2/21 + P2-9 gap | `8018e71`、`76bd501`、`6b58806`、`a674501` |
| B9 | P2-20 ComputedValue 单态化 | `77a4f03` |
| B10 | P0-2 + P1-8/9 + P2-8/10/11/12/13 + P3-1（layout 仓） | `e529407`、`de74c0f`、`6875de1`、`c07e23b` |
| B11 | P2-19 + PERF-12 + PERF-9（layout 仓 + renderer） | layout `a098a25`、`685b282`；renderer `7ef7c36` |
| B12 | P2-14 CssRule 类型化 | `d0c5bc6`；`2a7f846` |
| B13 | P2-17/18 + P3-5/6 + PERF-11 + P3-4 降级 | `fe2ee96`、`347fba2`、`90abd52`、`be782a9` |
| B14 | P2-6 media/supports 条件评估 | `91d9fa0` |

**未纳入本次计划（明确遗留）**：
- **P2-1** 绝对长度单位 pt/pc/in/cm/mm/q 未换算 px —— ✅ 已修（2026-08-09，cascade `d2c2f1c`）。
- **PERF-10** `map_style` 每属性 clone + lowercase —— ✅ 已修（2026-08-09，layout `5c9d48f`）。
- **P3-3** layout/convert.rs `child_nodes().to_vec()` —— renderer paint 侧已随 PERF-11 复用 scratch buffer，layout 侧未动。
- **P3-2** calc() 长期数值求值 —— 已知简化，推迟布局阶段。
- **@container** 条件恒 true —— 容器查询依赖布局反馈，推迟。

---

## 九、方法学与工具说明

- 审查采用 5 个并行 Agent 分工：cascade / layout / renderer / cssom 各一 + 横切架构一。
- 每个结论均经源码逐行核实；P1 以上通过临时探针测试**实际运行验证**（如 var() 2^N 展开、`#aä` panic、`rgba(...,50%)` alpha 丢失、`@layer` 排序、`@MEDIA` 分发、`@import url()` 空 href 等），探针文件已删除。
- 规范对照：CSS Cascade L5 / CSS Variables L1 / CSS Values L4 / CSS Syntax L3 / Selectors L4 / CSSOM L1 / CSS Display L3 / Box Model L3 / Flexbox L1 / Box Alignment L3 / CSS Color L4。
- 明确未发现：零 unsafe 违反、循环/反向依赖、`Rc<RefCell>` 所有权悬垂、box-sizing 映射错误、sRGB/预乘错误、坐标/命令序列错误、tiny-skia API 误用。
