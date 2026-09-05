# Goal — 审计修复轮（2026-09-05 性能/安全审计 → 修复）

> **更新时间**：2026-09-05
> **当前状态**：审计完成（[docs/audit-2026-09-05-perf-security.md](docs/audit-2026-09-05-perf-security.md)），7 高危 + 中危若干。本轮修复 **P0 + P1**（安全关键），P2/P3（纯性能/加固）显式排除至下一轮。

---

## 当前阶段定位

- **本轮主线**：审计修复。威胁模型：敌意网页（任意 HTML/CSS）→ panic = 远程 DoS、无界循环 = 挂起、无界内存 = OOM abort。
- **方法**：每项修 bug 先写 failing test（能写则写）→ 修复 → 全绿 → 独立 commit。一次只改一处。
- **验证基线**：各 crate `cargo check`（零 warning）+ `cargo test` 全绿 + `cargo fmt --check` + `cargo clippy -D warnings`。

## 任务列表（每项 = 1 个 commit）

### P0

- [ ] F-0 `[deps]` h2 升级 ≥0.4.16（RUSTSEC-2026-0258，经 network→reqwest 传入）。**退出**：`cargo audit` 零漏洞；`cargo check --workspace` 绿。
- [ ] F-1 `[layout]` taffy 边界钳制：`style_map.rs` extract_* 对 f64→f32 非有限/超界值钳制（参照 Servo ±33,554,432px）；`text.rs::resolve_font_size` 同步钳制；**修正 lib.rs/result.rs 中"taffy 会拒绝 NaN/Inf"的错误文档声明**（已核实 taffy 0.12.2 无此检查）。**退出**：新测试 `width:1e39px`/`1e999` 产出有界值而非 inf；全部测试绿。
- [ ] F-2 `[cascade]` var() 解析深度上限（S-2 栈溢出）：`compute.rs` VarResolver 加深度计数（超限按环处理回退），每元素共享 resolver。**退出**：新测试 2 万条 `--vN: var(--vN+1)` 不栈溢出且深度≤上限的链仍正常解析。
- [ ] F-3 `[css-parser]` 嵌套限深 1024→200（S-M1，浏览器平齐）。**退出**：`(((((...` 深度 200 记录 NestingTooDeep；既有测试适配全绿。

### P1 — 选择器 / tokenizer

- [ ] F-4 `[selectors]` An+B `checked_sub`（debug 溢出 panic）+ 复杂选择器单元数解析期上限（S-3 parse 侧；匹配侧 memo 化排除至下一轮）。**退出**：`:nth-child(±99999999999999999999)` 不 panic；超限选择器解析报错；全部测试绿。
- [ ] F-5 `[html5-tokenizer]` 单 tag 重复属性检查 HashSet 化（T3 O(n²)）。**退出**：既有重复属性语义测试（首个胜出）全绿；新测试 10 万属性 tag 快速完成。
- [ ] F-6 `[html5-tokenizer]` 100 万步兜底从 `panic!` 改为降级（EOF + 错误记录，H-M6）。**退出**：新测试模拟步数耗尽返回 EOF 而非 panic；tokenizer 测试全绿。

### P1 — html5-parser

- [ ] F-7 `[html5-parser]` AFE 列表长度上限 256（S-4b O(n²) 扫描 DoS；超限按 Noah's Ark 逐出最旧）。**退出**：新测试 `<b a=1>…×1000` 解析完成且列表有界；html5lib 树测试通过率不回退。
- [ ] F-8 `[html5-parser]` selectedness 快路径（S-4a）：插入无 `selected` 属性的 `<option>` 跳过全子树扫描（行为保持）。**退出**：新测试无 selected 属性的 N×option 插入不触发子树扫描（计时/计数）；selected 语义既有测试全绿。
- [ ] F-9 `[html5-parser]` `parse_fragment`/`set_inner_html` 非 Element 上下文 `.expect` → `Err`（H-M2）。**退出**：新测试 set_inner_html 作用于 Text 节点返回 Err 不 panic。

### P1 — renderer / chrome / network

- [ ] F-10 `[renderer]` clip mask 有界化（S-1）：裁剪栈复用单一 scratch Mask + 每层路径包围盒裁剪，消除"每层整画布克隆"。**退出**：新测试 1000 层嵌套 `overflow:hidden` 渲染完成且内存有界；既有像素测试全绿。
- [ ] F-11 `[renderer]` 零物理尺寸 panic（R-M1）：`width×scale` 取整为 0 时 canvas_w/h 与 pixmap 一致，`Mask::new(0,0)→expect` 不可达。**退出**：新测试 `render_page(html, css, 1, 1, 0.4)` 不 panic。
- [ ] F-12 `[renderer]` FontSystem/SwashCache 跨渲染持久化（R-M3 部分；保持 cosmic-text 类型不出现在 pub 导出）。**退出**：连续两次 render 仅首次扫描字体（可观测：惰性初始化仅一次）；公共 API 无外部类型泄漏。
- [ ] F-13 `[chrome]` `page.rs:44 expect("layout failed")` → Result 上抛（S-7，遵守 layout crate"调用方不得 expect"约定）。**退出**：flush/render_page_to_png 错误路径返回 Err；chrome 测试全绿（含 --no-default-features）。
- [ ] F-14 `[network]` 响应体上限（默认 64 MiB）+ `timeout(30s)`/`connect_timeout(10s)`（S-6，trait 契约补齐）。**退出**：新测试超限返回 NetworkError::BodyTooLarge；wiremock 测试全绿。

### 收尾

- [ ] F-15 `[docs]` goal.md/PROGRESS.md 同步本轮完成状态 + 审计报告标记已修复项。

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
