# Goal — 审计修复轮（2026-09-05 性能/安全审计 → 修复）

> **更新时间**：2026-09-05（修复轮完成）
> **当前状态**：F-0~F-15 全部完成并推送（[docs/audit-2026-09-05-perf-security.md](docs/audit-2026-09-05-perf-security.md)），7 高危 + 中危若干。本轮修复 **P0 + P1**（安全关键），P2/P3（纯性能/加固）显式排除至下一轮。

---

## 当前阶段定位

- **本轮主线**：审计修复。威胁模型：敌意网页（任意 HTML/CSS）→ panic = 远程 DoS、无界循环 = 挂起、无界内存 = OOM abort。
- **方法**：每项修 bug 先写 failing test（能写则写）→ 修复 → 全绿 → 独立 commit。一次只改一处。
- **验证基线**：各 crate `cargo check`（零 warning）+ `cargo test` 全绿 + `cargo fmt --check` + `cargo clippy -D warnings`。

## 任务列表（每项 = 1 个 commit）

### P0

- [x] F-0 `[deps]` h2 升级 ≥0.4.16（RUSTSEC-2026-0258，经 network→reqwest 传入）。**退出**：`cargo audit` 零漏洞；`cargo check --workspace` 绿。
- [x] F-1 `[layout]` taffy 边界钳制：`style_map.rs` extract_* 对 f64→f32 非有限/超界值钳制（参照 Servo ±33,554,432px）；`text.rs::resolve_font_size` 同步钳制；**修正 lib.rs/result.rs 中"taffy 会拒绝 NaN/Inf"的错误文档声明**（已核实 taffy 0.12.2 无此检查）。**退出**：新测试 `width:1e39px`/`1e999` 产出有界值而非 inf；全部测试绿。
- [x] F-2 `[cascade]` var() 解析深度上限（S-2 栈溢出）：`compute.rs` VarResolver 加深度计数（超限按环处理回退），每元素共享 resolver。**退出**：新测试 2 万条 `--vN: var(--vN+1)` 不栈溢出且深度≤上限的链仍正常解析。
- [x] F-3 `[css-parser]` 嵌套限深 1024→200（S-M1，浏览器平齐）。**退出**：`(((((...` 深度 200 记录 NestingTooDeep；既有测试适配全绿。

### P1 — 选择器 / tokenizer

- [x] F-4 `[selectors]` An+B `checked_sub`（debug 溢出 panic）+ 复杂选择器单元数解析期上限（S-3 parse 侧；匹配侧 memo 化排除至下一轮）。**退出**：`:nth-child(±99999999999999999999)` 不 panic；超限选择器解析报错；全部测试绿。
- [x] F-5 `[html5-tokenizer]` 单 tag 重复属性检查 HashSet 化（T3 O(n²)）。**退出**：既有重复属性语义测试（首个胜出）全绿；新测试 10 万属性 tag 快速完成。
- [x] F-6 `[html5-tokenizer]` 100 万步兜底从 `panic!` 改为降级（EOF + 错误记录，H-M6）。**退出**：新测试模拟步数耗尽返回 EOF 而非 panic；tokenizer 测试全绿。

### P1 — html5-parser

- [x] F-7 `[html5-parser]` AFE 列表长度上限 256（S-4b O(n²) 扫描 DoS；超限按 Noah's Ark 逐出最旧）。**退出**：新测试 `<b a=1>…×1000` 解析完成且列表有界；html5lib 树测试通过率不回退。
- [x] F-8 `[html5-parser]` selectedness 快路径（S-4a）：插入无 `selected` 属性的 `<option>` 跳过全子树扫描（行为保持）。**退出**：新测试无 selected 属性的 N×option 插入不触发子树扫描（计时/计数）；selected 语义既有测试全绿。
- [x] F-9 `[html5-parser]` `parse_fragment`/`set_inner_html` 非 Element 上下文 `.expect` → `Err`（H-M2）。**退出**：新测试 set_inner_html 作用于 Text 节点返回 Err 不 panic。

### P1 — renderer / chrome / network

- [x] F-10 `[renderer]` clip mask 有界化（S-1）：裁剪栈复用单一 scratch Mask + 每层路径包围盒裁剪，消除"每层整画布克隆"。**退出**：新测试 1000 层嵌套 `overflow:hidden` 渲染完成且内存有界；既有像素测试全绿。
- [x] F-11 `[renderer]` 零物理尺寸 panic（R-M1）：`width×scale` 取整为 0 时 canvas_w/h 与 pixmap 一致，`Mask::new(0,0)→expect` 不可达。**退出**：新测试 `render_page(html, css, 1, 1, 0.4)` 不 panic。
- [x] F-12 `[renderer]` FontSystem/SwashCache 跨渲染持久化（R-M3 部分；保持 cosmic-text 类型不出现在 pub 导出）。**退出**：连续两次 render 仅首次扫描字体（可观测：惰性初始化仅一次）；公共 API 无外部类型泄漏。
- [x] F-13 `[chrome]` `page.rs:44 expect("layout failed")` → Result 上抛（S-7，遵守 layout crate"调用方不得 expect"约定）。**退出**：flush/render_page_to_png 错误路径返回 Err；chrome 测试全绿（含 --no-default-features）。
- [x] F-14 `[network]` 响应体上限（默认 64 MiB）+ `timeout(30s)`/`connect_timeout(10s)`（S-6，trait 契约补齐）。**退出**：新测试超限返回 NetworkError::BodyTooLarge；wiremock 测试全绿。

### 收尾

- [x] F-15 `[docs]` goal.md/PROGRESS.md 同步本轮完成状态 + 审计报告标记已修复项。

## 每 commit 退出条件（全部满足才可 commit）

- [ ] 该 crate `cargo check` 零 warning + `cargo test` 全绿
- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] workspace member（renderer/network/chrome/layout）改动时：`cargo check --workspace` + `cargo test --workspace` 绿；chrome 改动加 `--no-default-features` 测试绿

## 不在本轮范围（显式排除，进下一轮 P2/P3）

- S-3 匹配侧 memo/祖先过滤；S-M3 token peek 克隆；S-M4 with_source 内存；S-M5 父 ComputedStyle 深克隆；S-M6 interning；S-M7 lookup_property
- C-M1 softbuffer expect、C-M2 每帧拷贝、C-M3 fit_text/地址栏上限、C-M4 tab 缓存复用
- R-M2 paint 递归深度、R-M4 视口裁剪接线；H-M1 reconstruct、H-M3 错误 Vec 上限、H-M4 Vec<char> 流式、H-M7 字符 token 合并；H-M8/D1 DOM 深度上限
- 全部 L 级（Ratio 0 分母、debug_assert 化、错误分支修正等）


## 完成记录（2026-09-05）

- F-0：`Cargo.lock` h2 0.4.19，`cargo audit` 无漏洞行。主仓库 commit 70b8584。
- F-1：`clamp_length`（NaN→0、±inf/超界→±2^25px）覆盖 px/percent/number/fr/font-size 六处出口；lib.rs/result.rs 文档修正。layout 仓库 6bbe4c7。
- F-2：`MAX_VAR_DEPTH=32`，超限按环同语义回退；2 万链不溢出。cascade 仓库 ab6ab46。
- F-3：`MAX_NESTING_DEPTH=200`，200/201 边界测试。css-parser 仓库 cb7aaf6。
- F-4：An+B i128；`MAX_COMPLEX_SELECTOR_UNITS=1024`（超限 InvalidSelector）。selectors 仓库 683bf01。
- F-5+F-6 **合并提交**：F-5 的 10 万属性性能测试暴露**审计漏掉的新 DoS**——合法大 tag 在单个 `next_token()` 内累积超固定 1M 步 → panic；F-6 一并修复（上限 = 10×len+1024，耗尽降级 EOF，测试注入点 `test_step_bound`）。tokenizer 仓库 b9ed0ea。
- F-7：`MAX_ACTIVE_FORMATTING_ELEMENTS=256`，白盒断言 + 3k 冒烟；5 万规模剩余成本属 H-M1（P2）。parser 仓库 2212cc4。
- F-8：`SelectSelectednessMemo{select(Weak), has_selected, all_disabled}`；plain 插入可证 no-op 时跳过子树扫描，selected 插入/备忘失效走完整算法；开发中发现的 step2 fall-through 回归被新测试拦截后修复。parser 仓库 8367d62。
- F-9：`recreate_context_element → Option`，非 Element 上下文兜底中性 `<div>`（html 上下文会落 BeforeHead 吞内容，弃用）；`set_inner_html` 对 Text 节点不 panic。parser 仓库 7484572。
- F-10：裁剪区单 rect + Mask 懒重建（内存 O(画布)），空交集退化为零面积 rect；1000 层嵌套 + 几何 + 恢复测试。主仓库 f2a61b4。
- F-11：Pixmap 回退同步钳制 phys 尺寸；子像素 scale 测试。主仓库 06c6303。
- F-12：`TinySkiaBackend{font_system, swash_cache}` 私有字段持久化（无 pub 泄漏），懒创建测试。主仓库 32e59ce。
- F-13：`render_page → Result`；headless `?` 上抛；`App::flush` 失败保留上一帧 + 记录视口防刷屏。主仓库 ec341d8。
- F-14：`MAX_BODY_BYTES=64MiB` 流式 chunk 检查 + Content-Length 预检 + 30s/10s 超时；`with_max_body_bytes` 可覆盖；顺带消除 Bytes→Vec 拷贝。主仓库 af9624f。
- F-15：goal/PROGRESS 同步 + Mimosa 深度扫描收尾（引擎源码零发现，详见 PROGRESS 附录）。
