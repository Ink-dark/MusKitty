# MusKitty 性能/安全代码审计报告

- **日期**: 2026-09-05
- **基线**: `main @ 39a2f2f`（PR #33 合并后）
- **范围**: 全部 14 个 crate 的非测试源码（约 43,000 行），三条并行深审线 + 工具链扫描
- **威胁模型**: 敌意网页（任意 HTML/CSS/HTTP 响应）→ panic = 远程 DoS / abort，无界循环 = 挂起，无界内存 = OOM abort

## 工具链结果

| 检查 | 结果 |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 全绿 |
| `unsafe` 全库扫描 | ✅ 0 处（符合硬约束） |
| `cargo audit` | ❌ 1 漏洞 + 4 unmaintained 警告 |
| `cargo deny check advisories bans`（仓库根未跟踪的 deny.toml） | ❌ advisories FAILED（与 audit 同源：h2 + unmaintained），bans ✅ |

### 依赖漏洞（RUSTSEC）

1. **RUSTSEC-2026-0258 · h2 0.4.15 无界空 DATA 帧**（`Cargo.lock:735`，经 muskitty-network → reqwest 传入）
   修复：升级 `h2 >= 0.4.16`。network 接入页面加载前必须处理。
2. **RUSTSEC-2026-0206 / 0192 · rustybuzz 0.14.1、ttf-parser（0.20/0.21/0.25）unmaintained**
   两条引入链：cosmic-text（文本渲染）与 winit → sctk-adwaita → ab_glyph → owned_ttf_parser（窗口层）。无直接漏洞，ttf-parser 官方替代为 skrifa；关注 cosmic-text 上游迁移。

## 高危发现（按可利用性排序）

### S-1 · renderer 裁剪栈：整画布 Mask 逐层克隆 → 单页 OOM/freeze
`crates/muskitty-renderer/src/backend/tiny_skia.rs:193-205`
每个嵌套 `RenderCommand::Clip` 克隆一张**全画布**尺寸 Mask 并做 O(画布像素) 的 `intersect_path`。敌意页面 N 层嵌套 `overflow:hidden` → O(N×画布) 内存与 CPU：4K 画布 × 1000 层 ≈ 8 GB 分配 + 80 亿像素求交。修复：用精确矩形求交 / 每层裁剪包围盒预裁后再 intersect。

### S-2 · CSS var() 链无递归深度限制 → 栈溢出
`crates/muskitty-cascade/src/compute.rs:214-315`（`resolve_component → resolve_var_ref → resolve_tokens → resolve_component`）
环检测（`in_progress` 集合）正确，但**无深度计数**。语法解析器的 `MAX_NESTING_DEPTH` 对此无效——每个 `--vN` 值都是深度 1 的平坦 token 列表。`--v1: var(--v2); --v2: var(--v3); …` ×2 万条 + `* { width: var(--v1) }` 即栈溢出 abort。且每属性每元素新建 `VarResolver`（compute.rs:154, style_tree.rs:237-250），链路被反复重走，时间也被放大。修复：`resolve_var_ref` 加深度上限（如 32）+ 每元素共享一个 resolver。

### S-3 · 选择器匹配：无递归深度限制 + 指数回溯 + 零缓存
`crates/muskitty-selectors/src/matching/mod.rs:183-259`（`walk_leftward`）、`mod.rs:139-144`（`walk_tree`）、`pseudo_matcher.rs:222-232`
- 栈：复杂选择器每个匹配单元 2 帧，选择器解析无 combinator 数上限 → 5 万单元选择器 + 深 DOM = 栈溢出。
- 时间：右到左后代匹配全量回溯，无 memo/bloom/祖先过滤。`.x .x … .x`（10 个）对每层都有 `.x` 的 100 深 DOM ≈ 100¹⁰ 次候选遍历，每元素每规则一次 → 挂起。
修复：解析期限制 combinator 数 / 匹配期限深；加祖先过滤或规则缓存。

### S-4 · HTML5 解析器两个 O(n²) DoS（约 1 MB 页面即可触发）
- **`<select>` + N×`<option>`**：每插一个 `<option>` 全量 DFS 已构建的 select 子树（`crates/muskitty-html5-parser/src/parser/helpers.rs:1817-1821 → 1683-1756 → 1575-1633`）。1 MB（~6.5 万 option）≈ 2×10⁹ 节点访问 → 数分钟挂起。修复：增量维护 option 列表或惰性计算 selectedness。
- **无界 AFE（active formatting elements）列表**：`<b a=1><b a=2><b a=3>…`（不同属性绕过 Noah's Ark 子句）使列表涨到 ~17 万项，之后每个格式化标签的 `find_formatting_element` / AAA `in_afe` / Ark 扫描都是全列表扫描（helpers.rs:793-826, 1072-1090, 1148-1151）→ O(n²)。修复：AFE 列表长度上限（WebKit 同款方案）。

### S-5 · 数字直通：敌意 CSS 的 NaN/Inf 全链路无拦截，且 layout 文档声明错误
`crates/muskitty-css-tokenizer/src/impls.rs:455-459` → `cascade/src/compute.rs:565-618` → `layout/src/style_map.rs:406,429,445,461-467` → taffy
- tokenizer：`1e999` → `10f64.powi(999)` = **inf**，400 位十进制字面量同样解析为 `Ok(inf)`，无范围检查。
- layout：`extract_px` 等用裸 `as f32`，`1e300px` → f32 inf；`flex-grow: 1e39` → taffy flex 算法 inf/inf → NaN；`grid 1e39fr` 同理。
- **已对照 vendored taffy 0.12.2 源码验证**：taffy 的 `TaffyError` 只有节点查找变体，compute 路径**没有任何 NaN/Inf 检查**。`muskitty-layout/src/lib.rs:55-58` 与 `result.rs:36-42` 中"taffy 会对 NaN/Inf 报错"的文档声明**不成立**，需修正。
- 后果：NaN 坐标进 `LayoutResult` → 渲染错乱；inf 字号进 cosmic-text（`layout/src/text.rs:86`）→ 潜在巨大分配/abort。
修复：在 style_map 边界 clamp f64→f32（参照 Servo/Gecko 的 ±33,554,432 px）并在 `Dimension::length` 前拒绝非有限值。

### S-6 · network：无响应体上限、无超时
`crates/muskitty-network/src/reqwest_impl.rs:54` `resp.bytes().await?.to_vec()` 无上限缓冲整个响应体（chunked 无需 Content-Length）→ 敌意服务器直接 OOM abort。`reqwest_impl.rs:35` 未设 `timeout()/connect_timeout()` → slow-loris 永久挂起，而 trait 契约（fetcher.rs:24）明确承诺超时错误。当前 crate 未接入主链路（M-1 暂缓），但接入前必须修。顺带：`Bytes → to_vec()` 多一次全量拷贝。

### S-7 · chrome：`expect("layout failed")` 违反 layout crate 自己的约定
`crates/muskitty-chrome/src/page.rs:44`
layout crate 文档（`muskitty-layout/src/lib.rs:44-47`）明确要求调用方**不得跨模块 expect**。当前 taffy 实际只在节点查找错误时返回 Err（见 S-5），CSS 难以直接触发，但 `App::flush` / 热重载 / `render_page_to_png` 全走此处，一旦 Err 即整个浏览器 abort。修复：返回 Result 上抛。

## 中危发现

### chrome
- **C-M1** `app.rs:81,83,85`：softbuffer `resize/buffer_mut/present` 全部 `.expect`，设备丢失/窗口销毁竞态（Wayland/X11 常见）→ 硬崩溃；仅防了 0×0。
- **C-M2** `compositor.rs:40`：每次重组整页像素 `to_vec()`（4K ≈ 33 MB/次），加上 `app.rs:82-84` 每帧共 3 次全帧拷贝。用 `PixmapRef` 可免所有权。
- **C-M3** `chrome/paint.rs:158-172` `fit_text`：O(n²) 文本测量，逐字符重建 cosmic-text Buffer；地址栏输入无长度上限（input.rs:126-129），粘贴超长 URL 卡死 UI 线程；未来 `<title>` 接入后即攻击者可控。另 `paint_chrome` + `draw_text` 对同一文本双重 shaping（paint.rs:371-381, 448-458）。
- **C-M4** `webview.rs:208-213` + `app.rs:255-274`：切 tab 无条件 `needs_repaint`，缓存帧从不复用，切页成本 = 首次加载。

### renderer
- **R-M1** `tiny_skia.rs:86-89,108-109,200`：`width×scale` 四舍五入为 0 时 pixmap 回退 1×1 但 `canvas_w/h` 仍为 0 → 首个 Clip 命令 `Mask::new(0,0) → None → .expect("mask alloc")` panic。公开 API `render_page(html, css, 1, 1, 0.4)` 可达。
- **R-M2** `paint.rs:75-227`：`paint_recursive` 无深度上限，深 DOM（非 parser 构建）栈溢出 abort（不可捕获）。
- **R-M3** `tiny_skia.rs:305-311`：每 Text 命令每帧新建 cosmic-text Buffer 全量 shaping，无字形复用（叠加每次 render 重建 FontSystem/SwashCache，tiny_skia.rs:161-162 —— 已抽查确认）；`paint.rs:149-151` 每节点克隆 text/font_family。
- **R-M4** `paint.rs:230-240` + `page.rs:49`：视口裁剪（P3-6）已实现但真实管线恒传 `viewport: None`，离屏元素照常生成命令并 shaping。

### HTML5 栈
- **H-M1** `helpers.rs:890-975`：AFE reconstruct O(|AFE|×栈深) 且可反复触发（`</div>` 弹栈不清 AFE）→ O(m·k)。
- **H-M2** `lib.rs:192,220`：`parse_fragment`/`set_inner_html` 对非 Element 上下文 `.expect("context must be an Element")` panic；脚本桥接入前必须改 Result。
- **H-M3** `parser/mod.rs:92` + dispatch 全局：解析错误 Vec 无上限，每条错一个堆 String（`</zzz>` × 6400 万字节 ≈ 900 万 String）。
- **H-M4** `impls.rs:26,118`（tokenizer）：整个输入缓冲为 `Vec<char>` —— 4× 内存放大，64 MiB 输入 ≈ 320 MiB；无流式。
- **H-M5** `impls.rs:2372-2385`：单 tag 重复属性扫描 O(n²)（`<p a1=1 a2=1 …>` 17.5 万属性 ≈ 1.5×10¹⁰ 比较）。
- **H-M6** `impls.rs:360-379`：100 万步兜底 `panic!` —— 后备机制应降级返回而非 panic。
- **H-M7** 逐码点 Character token，无 run 合并（impls.rs → lib.rs:101-119 → dispatch.rs:779-784 → helpers.rs:281-340），全管线常数放大数倍。
- **H-M8** `dom/src/tree.rs:39-95,253-256`：`append_child/insert_before` 无深度限制；100k 深 DOM 在序列化/克隆/递归 Drop 时栈溢出（parser 树有 512 上限，程序化构建无）。脚本桥前必须加深度检查。

### CSS 栈
- **S-M1** `css-parser/src/token_stream.rs:25`：嵌套限深 1024，为浏览器（100/200）的 5-10 倍；calc/selector 每层 ~6 帧，1000 层 ≈ 1-2.4 MB 栈，多线程边缘。建议 ≤200。
- **S-M2** `selectors/src/matching/pseudo_matcher.rs:128-136`：An+B `index - b` debug 下溢 panic（`:nth-child(±99999999999999999999)`），release 回绕出错误结果。改 `checked_sub`。
- **S-M3** `token_stream.rs:178-180`：`next_token()` 每次 peek 整体 clone Token（String 堆分配），解析最内层循环每 token 克隆 2-3 次 —— 全库最恶劣热路径分配。改 `&Token` 或 peek 槽位。
- **S-M4** `token_stream.rs:85-123`：`with_source` 每次解析建 8B/字符的 char→byte 映射 + 全源拷贝；`cascade/filter.rs:333-361` 对每个元素的 style 属性每轮重复整套流程。
- **S-M5** `cascade/style_tree.rs:159`：每元素深克隆整个父 `ComputedStyle`，抵消已记录的 PERF-2/4 工作。
- **S-M6** `matching/dom_impl.rs`：`local_name()` 每次调用克隆 String（每元素×每规则×每单元）；`classes()` 每次检查重新 split；`:nth-of-type` 每兄弟重新分配 —— 无 interning/缓存。
- **S-M7** `cascade/registry.rs:513-517`：`lookup_property` 对 ~65 项线性 `eq_ignore_ascii_case`，每声明/每百分比 token/每默认属性都调；`style_tree.rs:244-250` 对所有内建属性逐个 `compute_one` 即使不存在。建议 `phf`/`OnceLock<HashMap>`。

## 低危（摘要）

- chrome：`about_to_wait` 无条件 200ms 轮询（app.rs:433-439）；每 tab 常驻全分辨率帧（webview.rs:36-41）；headless `render_page_to_png` 无尺寸上限（30000×30000 ≈ 3.6 GB 先分配后拒绝）；`paint.rs:55`/`model.rs:167-169` 经公开 API 传 NaN/negative scale 可达 panic。
- html5-parser：foreign.rs:357-384 属性名解码 off-by-2（`&name[3..]` 应为 `[1..]`，产生乱码 prefix 元数据）；AAA 内层 64 次截断对深合法内容产生 html5lib 差异；`dispatch.rs:3290` DOCTYPE 落入错误分支。
- dom：兄弟查找 O(n)（node.rs:333-364）；事件监听器 Rc 循环引用泄漏（event.rs:160-176）；`normalize` O(k²)。
- css/values：Ratio 允许 0 分母（numeric.rs:442-493，当前无消费者）；若干 invariant-guarded `unwrap` 建议换 `debug_assert`。
- network：便捷 `fetch()` 每次新建 Client；非 UTF-8 header 值静默丢弃。

## 已验证为安全/正确的重点（覆盖声明）

- **全库 0 unsafe**；clippy -D warnings 全绿。
- **CJK/多字节安全**：HTML5 tokenizer 全程 `Vec<char>`，CSS `source_slice` 用 `get()` 返回 Option —— 无任何对不可信数据的 panic-able 字符串切片。
- **HTML5 无限循环防线真实有效**：tokenizer 100 万步上限（需改降级）、parser `MAX_REPROCESS_COUNT=50` 带回归测试、"pop until X" 循环按构造终止、AAA 8/64 次上限。
- **HTML 解析深度防线**：`MAX_OPEN_ELEMENTS=512` 封顶树构建深度，递归 Drop 同样有界。
- **字符引用数值溢出**：全程 saturating 运算，0/代理区/超 U+10FFFF → U+FFFD。
- **序列化转义正确**（serialize.rs:223-250）：无注入回路。
- **CSS 语法解析器** `MAX_NESTING_DEPTH` 生效（`((((((…` 炸弹记录 `NestingTooDeep` 不 panic）；calc 除以字面量 0 被拒绝；var() 环检测正确（仅缺深度限制，见 S-2）。
- **network 默认特性关闭**：reqwest 用 rustls（无 C TLS）、无 gzip/brotli（无解压炸弹放大）。
- chrome 命中测试索引全部有界检查；`flush_close` 无下溢；URL 经 HTML 转义再内嵌；无 static/Mutex 全局态。

## 修复优先级建议

| 优先级 | 项 | 理由 |
|---|---|---|
| P0 | S-2 var() 深度限制 | 一行计数器即可关掉栈溢出 abort |
| P0 | S-4 两个 O(n²)（option / AFE 上限） | ~1 MB 页面即可挂起 |
| P0 | S-5 NaN/Inf clamp + 修正 layout 文档 | 敌意 CSS 直通布局核心 |
| P0 | h2 升级 0.4.16 | 唯一已知 CVE |
| P1 | S-1 裁剪 Mask 整画布克隆 | 单页 OOM/freeze |
| P1 | S-3 选择器限深 + 回溯缓解 | 指数回溯挂起 |
| P1 | S-6 network 上限+超时 | 接入主链路前必须 |
| P1 | S-7 / H-M2 / R-M1 / H-M6 panic 消除 | 公开 API 可达的 abort |
| P2 | S-M3 token peek 克隆、S-M6 interning、FontSystem 持久化（R-M3）、每帧拷贝削减（C-M2）、tab 缓存复用（C-M4）、字符 token 合并（H-M7） | 热路径性能 |
| P3 | 低危清单 + 深度上限补齐（R-M2/H-M8）、视口裁剪接线（R-M4） | 加固 |
