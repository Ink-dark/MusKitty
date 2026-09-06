# MusKitty 漏洞 / 架构 / 性能全量扫描报告

- **日期**: 2026-09-06
- **基线**: 主仓库 `main @ b03a896`（PR #35 合并后）+ 11 个独立 crate 当前 HEAD：

  | crate | commit | crate | commit |
  |---|---|---|---|
  | muskitty-cascade | ab6ab46 | muskitty-dom | 24f277e |
  | muskitty-cssom | b758011 | muskitty-html5-parser | 7484572 |
  | muskitty-layout | 6bbe4c7 | muskitty-html5-tokenizer | b9ed0ea |
  | muskitty-css | 362fb3e | muskitty-selectors | 75b84ff |
  | muskitty-css-parser | d5d0abc | muskitty-css-tokenizer | 0f18e32 |
  | muskitty-css-values | 5fe62ac | | |

- **范围**: 除窗口层（muskitty-chrome）外全部 13 个 crate 的非测试源码；三维度（安全漏洞 / 架构改进 / 性能优化）
- **方法**: 四条并行深审线（renderer+network / HTML5 栈 / CSS 语法栈 / selectors+cascade+layout）+ 工具链扫描 + P0 级发现逐条人工复核（本报告所有 P0 的代码证据均已二次核实）
- **威胁模型**: 敌意网页（任意 HTML/CSS）→ panic/abort = 远程 DoS、无界内存 = OOM、指数回溯 = 挂起
- **与上轮关系**: 上轮（[audit-2026-09-05-perf-security.md](audit-2026-09-05-perf-security.md)）P0/P1 已由 F-0~F-14 修复；本轮含 F 系列修复的回归审查、延后 P2/P3 项的现状核对、以及上轮未覆盖的**架构维度**首次系统评估

## 工具链结果

| 检查 | 结果 |
|---|---|
| `cargo check --workspace`（renderer/network/chrome） | ✅ 全绿（1m16s） |
| `unsafe` 全库扫描（13 crate src/） | ✅ 0 处（符合硬约束） |
| h2 版本（F-0 修复确认） | ✅ `Cargo.lock` h2 0.4.19 |
| `cargo audit` | ⚠️ 本环境未安装 cargo-audit；advisory 状态沿用上轮结论（h2 已升级；rustybuzz/ttf-parser unmaintained 警告链仍在，随 cosmic-text 上游迁移，见 RUSTSEC-2026-0206/0192） |

## 执行摘要

本轮最重要结论：**上轮修复的三个"深度上限"守卫各留有一个结构性盲区，构成 4 个新 P0**；同时架构维度确认外部依赖解耦 ADR 有 1 处正式违规（layout 错误类型泄漏 taffy）与 2 处拓扑漂移（css-values 未接线、cssom→dom 意外依赖边）。

| 类别 | P0 | P1 | P2 | P3 |
|---|---|---|---|---|
| 安全（S） | 4 | 4 | 9 | 8 |
| 性能（P） | — | 5 | 9 | 7 |
| 架构（A） | — | 1 | 3 | 7 |

---

## 一、P0：新发现的高危漏洞（全部经人工复核）

### V-1 · var() 记忆化输出的指数放大无预算 → ~1 KB CSS 触发 OOM abort
`crates/muskitty-cascade/src/compute.rs:271-308`（F-2 残余，已复核）
F-2 的 `MAX_VAR_DEPTH=32` 只封了**递归深度**，不封**输出规模**。`resolve_var_ref` 缓存命中的是整条展开 `Vec<ComponentValue>`（:280 `.cloned()` 全量克隆），`resolve_tokens`（:296）递归展开。`--v0:red; --v{i}: var(--v{i-1}) var(--v{i-1})`（i≤25，深度 ≤32 完全合法）+ `*{color:var(--v25)}` → 每属性物化 2^25 ≈ 3300 万 token ≈ 1.5-3 GB；i=28 达 10+ GB → OOM abort。叠加放大：每属性每元素各自新建 `VarResolver` 重走整条链（compute.rs:154 + style_tree.rs:237-250，上轮 S-2 后半部分原样残留）→ 2^N 内存 × N 属性 × N 元素的双重 DoS。
**修复**: VarResolver 加输出 token 预算（如 10 万/属性），`resolve_tokens` 的 `out.extend` 处累减，超限按 guaranteed-invalid 处理（与 F-2 同语义；Chromium 对 var 替换有同类 substitution 长度预算）。

### CSS-P1 · 规则级嵌套完全绕过深度守卫 → ~24-60 KB CSS 栈溢出 abort
`crates/muskitty-css-parser/src/algorithms.rs:555-562`（F-3 盲区，已复核）
`MAX_NESTING_DEPTH=200`（F-3）只挂在 `consume_a_simple_block` 与 `consume_a_function`；而规则级递归环 `consume_a_blocks_contents → consume_an_at_rule / consume_a_qualified_rule → consume_a_block → consume_a_blocks_contents` 中，`consume_a_block` 直接 `discard_token()` 吞掉 `{`，**从不调用 `enter_nesting`**。`@x{@x{@x{…`（3 字节/层）或 `a{a{a{…`（2 字节/层）约 8k-30k 层即栈溢出——仅需 ~24-60 KB 敌意 CSS，且这是 CSS 最自然的嵌套形态（嵌套 style rule / at-rule）。附带 CPU 放大：qualified-rule 路径每层先 declaration-lookahead 失败再 `restore_mark` 重放，深 d 的内容被消费 d+1 次 → O(N×depth)。
**修复**: `consume_a_block` 开头统一 `enter_nesting()`；超限时 leave + 吞 token 到匹配 `}` 或 EOF，返回空 BlockContents 并记录既有 `NestingTooDeep`；补 `parse_a_stylesheet("@x{".repeat(n))` 回归测试。

### SEL-1 · 选择器匹配仍是全量回溯，无 memo / 无祖先过滤 / 无步数预算（F-4 只修了栈维度）
`crates/muskitty-selectors/src/matching/mod.rs:183-239`（`walk_leftward` Descendant 分支）
F-4 的 1024 单元上限封住了栈深，但时间维度原封未动：对每个祖先匹配即递归 `continues_leftward`，无任何记忆化。`.x .x … .x`（10 个 combinator）对每层都有 `.x` 的 100 深 DOM ≈ C(99,9) ≈ 1.7×10¹² 条候选路径/元素/规则——单条规则即可挂起数分钟；1024 单元上限下是 512^1023。敌意页面 1 MB 内可塞数百条此类规则。
**修复**（按性价比）: ① 匹配侧全局步数预算（每 元素×规则 10 万次 compound 匹配，超限按不匹配降级）——一行计数器即可止血；② Descendant 分支记忆化 `HashMap<(元素, 剩余单元数), bool>`，把 D^k 降为 D×k；③ 祖先 bloom/预扫描快速否决（Gecko 式）。②③ 是正道，① 是止血。

### SEL-2 · `:has()` 嵌套未被解析拒绝（WPT 明确要求 invalid），匹配侧 O(N²)/层且无任何闸门
`crates/muskitty-selectors/src/parser/simple.rs:733-736`（已复核）+ `matching/pseudo_matcher.rs:227-236`
解析侧 `"has"` 分支直接 `parse_relative_selector_list`，无"has 内禁止 has"检查——而仓库自带 WPT 期望 `tests/data/wpt/parse-has-disallow-nesting-has-inside-has.json` 明确 `.a:has(.b:has(.c))` 为 invalid。当前实现成功保留嵌套 `:has`，且 `tests/wpt_parsing.rs` 的 harness 是"informational"模式（只 assert 夹具已加载），失败被静默吞掉。匹配侧 `collect_descendants` 收集全部后代逐个匹配：单层 `div:has(div)` 在 N 元素页面 = O(N²) 次复合匹配（每次含 String 分配），嵌套 k 层 = O(N^(k+1))，嵌套深度当前无上限。
**修复**: ① 解析期把 `has_depth: u8` 穿进 parse 上下文，`:has/:not` 参数内出现 `:has` 直接 Err、`:is/:where`（forgiving）内当 unknown 丢弃——同时让两个 WPT 夹具转绿；② 匹配侧给 `collect_descendants` 加候选数上限；③ WPT harness 对新夹具升级为硬断言防回归。

---

## 二、P1：高危 / 高收益项

### 安全

- **P-1 · `<option selected>` 插入路径仍 O(n²)，~1 MB 输入挂起**（F-8 半修复，已复核）
  `html5-parser/src/parser/helpers.rs:1925-1934`。skip 判定 `!o_selected && …`——带 `selected` 属性的 option 恒走慢路径 `selectedness_setting_algorithm` → `get_list_of_options` 全子树 DFS。`<select><option selected>x</option> × 6.5 万` ≈ 8.5×10⁹ 节点访问。攻击成本比原 S-4a 仅多 9 字符/条目。**修复**: memo 增加 `last_selected: Weak<Node>` 与计数，带 selected 插入只做 O(1) 定点取消旧选中。
- **D-1 · `replaceChild` 陈旧索引：越界 panic + 静默树损坏**（新发现，已复核）
  `dom/src/tree.rs:115-140`。`idx` 在 :120 计算，:130-131 `remove_child_internal` 移除 new_child 时若其父即本 parent（new 是 old 的兄弟），children 位移，:135/:138 用陈旧 idx 索引——`[A,B,C]` + `replaceChild(A,C)` 越界 panic；`replaceChild(A,B)` 错误替换 C。公开 DOM API，与 F-9 同判据：脚本桥接入前必修。**修复**: 移除后重新 `position()`；显式处理 `new == old` 早退。
- **SEL-3 · `:is/:not/:where` 嵌套 × 每层独立 1024 单元 → 匹配栈深无界**
  `selectors/src/parser/simple.rs:719-727` + `matching/pseudo_matcher.rs:146-153`。1024 上限是**每个 complex** 的；逻辑组合嵌套仅受 css-parser 括号深度约束。`:is(:is(…))` × 1000 层、每层 1000 个 Child 单元 → 匹配栈 ~2×10⁶ 帧（40-80 MB）→ 栈溢出 abort，构造仅需 ~4 MB CSS，Child combinator 下时间线性、先溢栈。**修复**: 匹配入口传 `depth_budget`（逻辑组合嵌套 + 已走单元数 ≤ 2048）或解析期限逻辑组合嵌套 ≤32；顺带修正 `complex.rs:70` "栈深 ≤ ~2k 帧" 的失实注释。
- **RN-1 · Mask 懒重建时机缺陷：空 Clip 对纯浪费 O(画布) → CPU 挂起**（F-10 引入）
  `renderer/src/backend/tiny_skia.rs:139-154`。Mask 在**命令循环顶**重建而非绘制命令消费时：`[Clip(A), EndClip]` 空对（paint.rs:189-198 对有布局盒的 overflow 元素无条件生成）也触发一次整画布 `Mask::new` + `fill_path`。10 万个兄弟空 `overflow:hidden` div（~3-5 MB HTML）× 1080p ≈ 200 GB 无效 memset。**修复**: 构建移到 Rect/Text 分支实际消费 clip 时（dirty 标志）；空对成本归零。

### 性能（样式/布局管线三大分配源 + 两大重复计算）

- **CAS-1 · 每元素继承 = 1 次整表深克隆 + ~20 次逐属性克隆**（S-M5 残余）
  `cascade/src/style_tree.rs:159`（父 `ComputedStyle` 全表 `.cloned()`）+ `defaulting.rs:44/49/63`。N=5 万元素 ≈ 400 万次 `Vec<ComponentValue>` 深拷贝，直接抵消 PERF-2/4 的链式继承成果。**修复**: `ComputedValue` 内部改 `Arc<[ComponentValue]>`，继承走写时复制；最小改法先去掉 :159 与 defaulting 的二重克隆之一。
- **CAS-2 · 声明值三重克隆**：`filter.rs:200`（prepare 一次，可接受）+ `filter.rs:425` `push_declared` **每元素每命中规则每声明** `value.to_vec()` + `defaulting.rs:72` 再一次。100 规则 × 5 声明 × 1 万元素 = 500 万次深拷贝。**修复**: `PreparedDecl.value` 改 `Arc<[ComponentValue]>`。
- **CAS-3 · 全属性盲算 + registry 线性扫描**（S-M7 残余，两半都在）
  `style_tree.rs:244-250` 对全部 ~65 内建属性逐个 `compute_one`（未声明也走全流程）；`registry.rs:513-517` `lookup_property` 线性 `eq_ignore_ascii_case`，被每声明/每默认值/每百分比 token 调用。**修复**: `OnceLock<HashMap<&str, &PropertyDefinition>>`；未声明属性用预生成 initial 值常量表直填，只算 declared ∪ inherited。
- **LAY-2 · 每次 `build_layout_tree` 新建 `FontSystem`**：`layout/src/tree.rs:61`。`FontSystem::new()` 枚举解析系统字体（50-300 ms），每次布局/resize/热重载都付一遍——与上轮 R-M3（renderer 侧，F-12 已修）同源但发生在 layout 侧。**修复**: FontSystem 由页面/会话级持有，`LayoutTree` 借用注入。
- **LAY-3 · 文本测量无缓存**：`layout/src/text.rs:30-58` 每次 measure 全量 `Buffer::new` + Advanced shaping；taffy 嵌套布局对同一节点多次调用 measure。文本测量占布局耗时 80%+，这是最大热点。**修复**: `NodeContext::Text` 旁挂 `RefCell<Option<(f32, Size)>>`（key=available width）。

### 架构

- **LAY-1 · `LayoutError` 公开变体携带 `taffy::TaffyError` / `taffy::NodeId`——ADR 正式违规**
  `layout/src/result.rs:44-47` 经 `lib.rs:33` pub 导出。[外部依赖解耦 ADR](decisions/2026-08-16-external-dependency-decoupling.md) 明文"外部依赖类型不出现在任何 pub 导出"，其"已验证 layout 无 taffy 泄漏"漏掉了错误载荷。上层 match 此错误即被迫依赖 taffy，换布局引擎时错误处理全部返工。**修复**: `ComputeLayoutFailed(String)` + `NodeLayoutMissing(usize)`；CI 加 `grep taffy::` pub 守卫防回归。

---

## 三、P2：中危 / 中收益

### HTML5 栈

- **P-2** 解析错误 `Vec<ParseError>` 无上限（`parser/mod.rs:102`）：64 MiB 输入 `<div>`×2100 万 → ~800 MB 纯错误数据（每超限 token 一条 `DomDepthExceeded`）。修复：错误上限 1024 + 每 variant 首例 + 计数（Chromium 做法）。
- **P-3** AFE reconstruct 残余（`helpers.rs:932-950`）：F-7 封顶 256 后单次 ≤131k 比较，但逐条目全栈扫描 `open_elements.iter().any(ptr_eq)` + 可反复触发（`</div>` 弹栈不清 AFE）仍构成 O(token × 131k)。修复：AFE 条目加 `on_stack: bool` 标志。
- **D-3** DOM 公开 API 无深度限制（H-M8 残余）：`append_child`/`clone_node`/`serialize_node`/递归 Drop 均无界，程序化 10 万层链栈溢出。防线应放 dom 层而非只放 parser 层（数据结构不变式 vs 某个生产者）。
- **D-4** 事件监听器 Rc 循环引用泄漏（`event.rs:160-176`）：回调捕获强 Rc 即整子树不可释放。修复：`Node::Drop` 清空 `event_listeners`（零 unsafe 可行）。
- **D-5** 兄弟查找 O(n) 且每候选 `borrow()`（`node.rs:333-364`）：`nextSibling` 遍历 = O(n²)。修复：Node 加 `prev/next: Weak` 双链（tree.rs 是唯一修改点）。
- **D-6** `normalize` O(k²)（`tree.rs:365-412`）：每合并一对重扫 + `child_nodes().to_vec()`。单趟双指针化。
- **T-1/H-M4** `Vec<char>` 输入 4× 内存放大（html5/css 两个 tokenizer 同病，`impls.rs:26` / `css-tokenizer/impls.rs:36-38`）：64 MiB ASCII → 256 MiB 起步，且阻塞流式路径。全部前瞻/匹配均为 ASCII，可迁 `Box<str>` + 字节游标 + `char_indices`。
- **T-2/H-M7** 逐码点 `Token::Character(char)`：每字符 ~10 层调用 + 3 次 RefCell borrow + Rc clone。Data/RCDATA/RAWTEXT 的 anything-else arm peek 连续 run 合并，文本密集页面管线成本降 5-10 倍。

### CSS 栈

- **CSS-P2** `with_source` 的 9-14B/字符代价为**死链路**支付：唯一产出 `Declaration::original_text` 全工程无生产消费者（cascade 的 var() 用 `Vec<ComponentValue>`）。默认入口 `entry_points.rs:29-31` 摘除 source 追踪，或删除 original_text 机制；附带 CRLF 预处理破坏 span 映射的潜伏 bug（`token_stream.rs:92`，char_to_byte 按原始 source 建表却被预处理后索引查询——original_text 接入前必修）。
- **CSS-P3** §5.5.5 歧义 mark/restore 双重解析（`algorithms.rs:603-630`）：每个嵌套规则内容被消费"块深度+1"次，常规样式表 2-3× 常数。Chromium 式 fast-path 预判（轻扫至 `:`/`;`/`{`/`}` 再决定路径）。
- **S-M3** `next_token()` 整体 clone Token（`token_stream.rs:182-184`，未修）：最内层循环每 token 克隆 2-4 次（含 String 堆分配）。改 `Option<&Token>` peek 槽。
- **CSSV-P2** css-values 整 crate 无生产消费者，cascade 自带第二套 calc 解析器（`compute.rs:433-506` vs `css-values/math.rs:87-120`）——双实现漂移已显形（语法约束不一致）。短期收敛或长期按拓扑接线。
- **CSSOM-P2** `element.style` 全操作重解析（`element_style.rs:55-97`）：每次 get/set/remove 都 parse→mutate→serialize，与 cascade `filter.rs:338` 叠加后同一 style 属性一次样式计算被完整解析多遍。惰性缓存 `Rc<CssStyleDeclaration>` + attribute 写穿透失效。
- **CAS-4** 选择器列表用 `specificity_max` 参与级联（`filter.rs:194`）——**正确性 bug**：`#x, div {…}` 给 div 命中记 (1,0,0)，与后出现的 `div {…}` 级联结果错序。改为逐 complex 匹配 + 用命中条的 specificity。

### selectors / renderer / network

- **SEL-4/S-M6** 匹配接口是 String/Vec 分配工厂（`dom_impl.rs:85-126, 190-221`）：`local_name()` 每次克隆、`classes()` 每次 split 收集、`:nth-of-type` 每兄弟克隆。1 万元素 × 1000 规则 ≈ 3×10⁷ 次堆分配。最小改法：匹配入口构造一次 `ElementSnapshot`；中期 dom 引入 `Rc<str>`/Atom（与 P-4 的 tag interner 一并决策）。
- **RN-2** `render()` 画布尺寸无上限（`tiny_skia.rs:99-108`）：`render(cmds, 30000, 30000, 1.0)` → 3.6 GB `vec![0]` 先分配后失败 abort（分配发生在 renderer，上次记在 chrome 侧）。入口加画布面积上限。
- **RN-3** `draw_text` 不校验 font_size/width 有限性（`tiny_skia.rs:348-355`）：S-5 的 inf/NaN 在 renderer 侧仍直通 cosmic-text（layout 侧 F-1 已钳制，但 renderer 公开 API 独立可达）。两行 `is_finite()` 纵深防御。
- **RN-4** `RenderOutput::Pixels` 每帧全画布 `to_vec()`（`tiny_skia.rs:240-249`）：4K ≈ 33 MB/帧 + 与自持 pixmap 构成 2× 画布常驻（chrome 侧 C-M2 的上游源头）。`Arc<Vec<u8>>` 或借用式变体。
- **LAY-4** 根级 `display:contents`/根为 absolute 时多余 in-flow 盒被静默丢弃（`convert.rs:60-67`）：内容不可见不报错。合成 Block 根承载即可。

---

## 四、P3：低危 / 架构卫生（摘要）

- **html5-parser**: `<select size=0>` 应视 display size 1（规范 §4.10.10，`helpers.rs:1720-1726`）；foreign 属性名解码 off-by-2（`foreign.rs:371-374`，`&name[3..]` 应为 `[1..]`，xlink 前缀错乱）；AfterAfterBody DOCTYPE 被 match guard 吞入错误分支（`dispatch.rs:3299`，树形无差异仅错误计数错位）；`parse_fragment` 超限静默返回空 fragment 无错误通道（`lib.rs:136-138`）；foreign end-tag 循环内每迭代 `to_ascii_lowercase` 分配（`foreign.rs:755`）；栈扫描热路径 `html_local_name` 每调用克隆 String（`helpers.rs:568-588`）。
- **html5-tokenizer**: `reset()` 遗漏 `current_attr_names` 清空（`impls.rs:416-440`，当前无正确性影响但状态复位不完整）；命名实体回退扫描未用"最长实体名 32"截断（`impls.rs:2177-2193`）。
- **dom**: `insertBefore` 同族 ref_idx 陈旧致插入位置错（`tree.rs:73-94`，DOM §4.2.6 语义）；fragment 插入非原子（中途 Err 树已半改）；只读遍历分配（`children()` 每次全扫 + Vec）。
- **css-parser**: `nesting_error` 无入口暴露——超限恢复后返回"成功"的树、深层内容被静默上提，调用方无从得知；自定义属性名判定与 css-values 不一致（`--: red` 一侧接受一侧拒绝）；`tokenize()` 无容量预估。
- **css-tokenizer**: 指数 `1e99999999999999` 的 i32 解析失败静默归 0（应 ±inf，浏览器一致行为）；`Token::Comment` 死变体 + 文档失实（全 crate 无构造点）。
- **css-values**: `min/max/clamp` 允许尾随逗号（`math.rs:174-198`）；`format_number(inf)` 输出 "inf"（§9.3 要求 "infinity"）；var() fallback 的 `Token::String` 序列化不转义内部引号；`from_cvs`/`extract_url_from_function` 宽松接受非法输入（`"10px var(--x)"`、`url("a" "b")`）。
- **cssom**: `Hash(Unrestricted)` 裸拼序列化可产破损 round-trip（`serialize.rs:107-108`）；`setProperty("color", "red; font-size:1px")` 静默截断（JS 接入前修）；convert 全树深拷贝（消耗式 API 可免）；`style_css_text` 返回原始文本而非规范化序列化（文档标注）。
- **cascade**: `is_css_wide_keyword` 遗漏 `revert-layer`（`custom_properties.rs:46-51`）；`collect_custom_properties` 原语路径已被链式来源取代（标 deprecated）；F-2 残留 V-2（fallback 内嵌套 var() 不计深度，受 css-parser 1024 括号深度边缘保护）。
- **selectors**: `relative.rs:79` expect 防御化；`walk_tree`/`collect_descendants` 递归无界（依赖 parser 512 深度兜底，程序化 DOM 可破）。
- **layout**: `convert.rs:145,262,266` 跨模块 expect 违反自身文档约定；`add_child(...).ok()` 静默吞错致 absolute 盒脱挂；`text.rs:118` NaN weight 经 clamp 仍 NaN。
- **renderer**: paint 每节点 3-4 次 `borrow()` + 3 次 `styles.get` + 非 Element 无条件 `font_family.to_string()`（`paint.rs:92-151`）；`family_from_css` 每命令小写化分配；usize 裸地址 style key 的 ABA 复用风险未文档化（`paint.rs:88`）；Backend trait 无错误通道 + `RenderOutput::None` 死变体 + 三层三种错误策略。
- **network**: 便捷 `fetch()` 每次新建 Client（`lib.rs:67-68`）；非 UTF-8 header 静默丢弃（`reqwest_impl.rs:70-78`，建议 from_utf8_lossy）；`NetworkError::InvalidUrl` 死变体 + stringly 错误无超时/TLS 分类；trait 契约未固化"体积上限/scheme 白名单/重定向有界"实现要求；`charset` feature 启用但 `text()` 恒 lossy UTF-8（GBK 页面乱码进 parser）；`len as usize` 32 位截断（chunk 兜底不可利用）。

---

## 五、架构维度评估（本轮新增维度）

### 5.1 外部依赖解耦 ADR（2026-08-16）合规矩阵

| crate | 合规 | 违规/漂移 |
|---|---|---|
| renderer | ✅ tiny-skia/cosmic-text 类型全私有 | ⚠️ cosmic-text 非 optional 依赖但仅 feature-gated 模块使用（`Cargo.toml:26`，Mock-only 消费者被迫携带 rustybuzz 链）|
| network | ✅ NetworkError 无 reqwest 类型 | — |
| selectors / cascade / css 系 | ✅ 零外部依赖 | — |
| layout | ❌ | **LAY-1**：`LayoutError` 公开携带 `taffy::TaffyError`/`NodeId`（唯一正式违规）|

### 5.2 依赖拓扑漂移（AGENTS.md 拓扑图 vs 实际）

1. **css-values 无消费者**：拓扑图画了 `css-values → cascade` 的边，实际 cascade 不依赖它且自带第二套 calc 解析器（CSSV-P2 双实现漂移）。
2. **cssom → dom 意外依赖边**：`ElementStyle` 实现于 `Rc<RefCell<Node>>`，cssom 脱离纯 CSS 栈。应移到 glue 层（chrome / 未来 script 层）或至少在拓扑图标注。
3. 建议：要么接线要么修图，避免"文档拓扑"与"真实拓扑"持续分叉。

### 5.3 数据模型结构性问题（多数 P1/P2 性能项的共同根因）

- **ComputedStyle 全拥有**（`cascade/src/style.rs:69-73`）：键 `String`、值 `Vec<ComponentValue>`、继承靠深拷贝——CAS-1/2/3 的根源。演进路径：短期 `Arc<[ComponentValue]>` + `Arc<str>` 键 → 中期 Servo 式"每元素存 diff + 指向父"的写时继承。
- **DOM `Rc<RefCell<Node>>` 模型**：在零 unsafe 约束下是合理选择（父子环已用 Weak 正确打断），真实短板三个且可渐进修补：兄弟无双向链（D-5）、RefCell 借用纪律靠人工维持（D-1/D-2 的温床，建议 crate 级"跨递归不持 Ref"铁律 + debug_assert）、事件回调强引用环（D-4）。arena（slotmap+NodeId）能一并解决但波及 14 crate，建议作为文本渲染完成后的独立 ADR 议题，现阶段不做。
- **tokenizer `Vec<char>` 输入模型**（html5 + css 双侧）：4× 内存 + 阻塞流式；全部前瞻均为 ASCII，`Box<str>` + 字节游标等价且多字节安全。
- **错误上报机制**：双侧（tokenizer/parser）无界 Vec + 30+ 处逐条 push 的调用形态在鼓励堆积（P-2 根因）。规范把 parse error 定位为可报告事件——正确形态是首例 + 计数 + truncated 标志。

### 5.4 trait 边界与契约质量

- **RendererBackend**：object-safe ✓、借用式零克隆 ✓；缺错误通道（RN-2 的 OOM 只能静默 1×1）、`RenderOutput::None` 死变体。
- **NetworkFetcher**：极简 GET-only + `impl Future + Send`，与 ADR"不重复抽象"一致、迁移路径已文档化 ✓；但 F-14 的安全属性（体积上限/超时）是**实现细节而非 trait 契约**——自研栈实现者不会从 trait 文档得知必须兑现。接主链路前把"实现要求"写入 trait 文档（零代码改动）。
- **Tokenizer pull 模型 + 控制面 setter**：reentrancy 骨架正确（当前已是逐 token 流式消费，非遗留物化）✓；改进：`set_foreign_content` 每字符虚调用改"栈变化置脏 + 惰性拉取"。
- **cascade 对 dom 硬耦合**：`filter.rs` 全签名用具体 `DomElement`、`style_tree.rs` 直接走 `Rc<RefCell<Node>>`——selectors 的 `Element` trait 抽象只在匹配点生效，管线本体不可替换 DOM。泛型化 + 最小 `DomTree` 游标 trait。

### 5.5 死代码 / 假承诺清单（架构卫生）

`RenderOutput::None`、`Token::Comment`（doc 声称会产出，实际 0 构造点）、`NetworkError::InvalidUrl`（0 构造点）、`Declaration::original_text` 机制（0 消费者，但每次解析付 9-14B/字符代价）、`collect_custom_properties` 原语路径（被链式来源取代）。建议统一清偿或修正文档承诺。

---

## 六、上轮遗留项核对结论（F-0~F-14 回归 + 延后项现状）

### 修复回归审查

| 修复 | 结论 |
|---|---|
| F-0 h2 0.4.19 | ✅ Cargo.lock 确认 |
| F-1 taffy 钳制 | ✅ `clamp_length` 是唯一 `as f32` 出口、五条数值路径全覆盖、文档已修正。P3 备注：百分比按"长度"钳制语义怪异但无 NaN 通路 |
| F-2 var() 深度 | ⚠️ 栈溢出已关，但漏了**输出预算**（本轮 V-1 升级为 P0）与 fallback 嵌套计数（V-2，P3） |
| F-3 嵌套限深 200 | ⚠️ 组件值路径 enter/leave 全路径配对、恢复保证 token 前进、树深真实封顶 ✓；但**规则级嵌套完全无守卫**（本轮 CSS-P1 升级为 P0） |
| F-4 An+B + 1024 单元 | ⚠️ An+B i128 无溢出 ✓、栈维度封顶 ✓；时间维度（SEL-1，P0）与逻辑组合嵌套（SEL-3，P1）未覆盖 |
| F-5/F-6 重复属性+步数兜底 | ✅ 语义与旧扫描一致（保首个）、bound 随输入缩放合法输入不可达、降级 EOF 不 panic |
| F-7 AFE 256 上限 | ✅ 逐出策略与 Noah's Ark 同语义；对 >256 互异格式化元素偏离规范但真实页面/测试套件远低于此（WebKit 同款实践），**可接受的有意偏离** |
| F-8 selectedness 备忘录 | ⚠️ 无 selected 路径四分支证明为 no-op ✓、Weak 防地址复用 ✓；**带 selected 插入恒走慢路径**（本轮 P-1，P1） |
| F-9 fragment 兜底 | ✅ 非 Element 上下文兜底 `<div>` 推导正确，测试覆盖 |
| F-10 裁剪栈单 rect | ⚠️ 内存 O(画布) 达成、save/restore 语义正确 ✓；**懒重建时机在循环顶而非消费点**（本轮 RN-1，P1） |
| F-11 1×1 钳制 | ✅ 全路径（NaN/0/负/巨大/子像素）无 panic |
| F-12 FontSystem 持久化 | ✅ 懒创建 + 跨 render 持久 + 无借用冲突；字形复用部分（静态页 resize 重 shaping）仍开放（R-M3 后半） |
| F-13 Result 化 | ✅（chrome 侧，本轮范围外抽查无异常） |
| F-14 体积上限+超时 | ✅ CL 撒谎有逐 chunk 兜底、总超时覆盖 body 读取全程、恰好等于上限成功；两处 P3 微瑕（`len as usize`、未 with_capacity） |

### 延后 P2/P3 项现状（全部未动，维持原判）

R-M2（paint 递归无深度上限，parser 512 兜底）、R-M4（**viewport: None 恒传，生产管线 7 处全 None，视口裁剪是死代码**——离屏元素照常 shaping）、H-M3/H-M4/H-M7/H-M8、dom 三项（兄弟 O(n)/Rc 环/normalize）、S-M3/S-M4（本轮证实更糟：为死链路付费）/S-M6/S-M7、network 便捷 fetch 新建 Client + 非 UTF-8 header。

---

## 七、已验证安全 / 正确（覆盖声明）

- 零 unsafe（13 crate grep）；workspace cargo check 全绿。
- **F 系列修复本体**（除已列出的盲区外）逐行核对通过，配对/终止性/边界推演见第六节。
- **tokenizer 双侧无 panic 路径**：全部 unwrap/unreachable 由状态机构造保证；数值字符引用全程 saturating，代理区/越界 → U+FFFD；实体表二分查找基于声明排序。
- **EOF 终止性**（css-parser）：全部主循环消费前检查 EOF、index 单调、mark/restore 回滚后仍保证吞 token——无死循环路径。
- **转义安全**：CSS `\` 转义解码（§4.3.7）、HTML 序列化转义、cssom identifier/string 序列化（CSSOM §3）均无注入回路；多字节字符边界安全（全部 `.get()` 或 char_indices 派生）。
- **F-1 钳制后无 NaN/Inf 进入 taffy 的通路**（layout 侧）；renderer 侧 Rect 构造经 tiny-skia FiniteF32 校验，非有限几何自动降级跳过。
- **级联准则次序**（origin×importance > style-attr > layer > specificity > order）与 §6.1 核对无误；媒体/支持查询 fail-closed。
- **HTML5 多层防线真实有效**：MAX_INPUT_BYTES=64MiB / MAX_OPEN_ELEMENTS=512 / MAX_REPROCESS_COUNT=50 / AFE 256 / AAA 8+64；F-6 步数 bound 随输入缩放。
- **F-12 字段无别名冲突**；F-14 兜底完备（含恰好等于上限、reqwest 默认重定向 10 跳有界）。
- **事件派发 snapshot-then-invoke 模式**杜绝跨回调 Ref 重入 panic。

---

## 八、修复优先级建议

| 优先级 | 项 | 位置 | 理由 |
|---|---|---|---|
| P0 | V-1 var() 输出 token 预算 | cascade/compute.rs | ~1 KB CSS → 2^N 物化 OOM，一行预算计数器关闭 |
| P0 | CSS-P1 规则级嵌套守卫 | css-parser/algorithms.rs:555 | ~24-60 KB CSS → 栈溢出 abort，F-3 最大盲区 |
| P0 | SEL-1 匹配步数预算（止血）+ memo（正道） | selectors/matching/mod.rs | 1 KB CSS + 深 DOM 挂起，F-4 未覆盖的时间维度 |
| P0 | SEL-2 `:has` 嵌套解析禁止 + 匹配预算 | selectors/parser/simple.rs | WPT 违背 + O(N²)/层挂起，零闸门 |
| P1 | P-1 option selected 备忘录补全 | html5-parser/helpers.rs | F-8 半修复，1 MB 挂起 |
| P1 | D-1 replaceChild 索引重定位 | dom/tree.rs | 公开 API panic + 树损坏，脚本桥前必修 |
| P1 | SEL-3 逻辑组合深度预算 | selectors | ~4 MB CSS 线性构造栈溢出 |
| P1 | RN-1 Mask 消费点重建 | renderer/tiny_skia.rs | F-10 引入的 CPU 挂起向量 |
| P1 | LAY-1 LayoutError 去 taffy 化 | layout/result.rs | ADR 唯一正式违规，改动极小 |
| P1 | CAS-1/2/3 Arc 化 + registry 哈希化 + 盲算消除 | cascade | 样式管线三大分配源，一次重构顺带解决 |
| P1 | LAY-2/3 FontSystem 上移 + 测量缓存 | layout | 布局耗时数量级改善 |
| P2 | P-2 错误上限、P-3 AFE on_stack、D-3/4/5/6 DOM 加固、T-1+T-2 输入模型改造、CSS-P2 with_source 摘除、S-M3 peek 槽、CSSV-P2 calc 双实现收敛、CSSOM-P2 style 缓存、CAS-4 per-selector specificity、SEL-4 匹配快照、RN-2/3/4、LAY-4 根级 contents | 各处 | 挂起缓解 / 泄漏与 overflow 加固 / 热路径主轴 |
| P3 | 第四节全部 + V-2、拓扑图修正、死代码清偿 | 各处 | 正确性补丁与架构卫生，多数 ≤10 行 |

### 建议的批次划分

1. **止血批（全部 P0，均为小改动）**: 四项加起来预计 < 200 行 + 回归测试。
2. **正确性批（P-1/D-1/CAS-4/LAY-4/P-5/P-6 等）**: 小而独立，各自一个 commit。
3. **性能主轴批（CAS-1/2/3 + LAY-2/3 + SEL-4 快照）**: cascade Arc 化是重头，建议独立 goal 轮。
4. **输入模型批（T-1+T-2+CSS-P2）**: 两个 tokenizer 的 `Vec<char>` → 字节游标迁移，与流式能力解锁一并决策。

## 九、本轮扫描的局限

- cargo-audit 不可用（环境未安装），advisory 层面仅静态确认 h2 0.4.19；rustybuzz/ttf-parser unmaintained 链沿用上轮结论。
- 4 个独立 crate 为浅克隆，修复回归审查基于源码内 F 系列注释 + 回归测试核对，未逐 commit 回溯 diff。
- 未含 WPT 语义实跑（本轮纯静态扫描 + 抽查验证）；selectors 74.8% 的已知差距见 [wpt-compliance-2026-09-06.md](wpt-compliance-2026-09-06.md)。
