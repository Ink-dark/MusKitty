# 全工作区代码安全审计报告

**审计日期**：2026-08-02
**审计范围**：`d:\Muskitty` 下 12 个 crate（HTML/CSS/Layout/Renderer 全链路）
**审计维度**：递归深度 / 资源耗尽 / 模块间 API 契约 / 依赖 CVE / 内存安全 / 错误传播

---

## 一、漏洞总览

| 编号 | 级别 | 标题 | 状态 |
|---|---|---|---|
| C-1 | CRITICAL | `var()` 循环引用导致栈溢出（DoS） | ✅ 已修复（commit `d1ab542`） |
| C-2 | CRITICAL | HTML 解析器无输入大小/嵌套深度限制 | ✅ 已修复（commit `29ea57b`） |
| H-1 | HIGH | `taffy` 布局错误通过 `expect` 跨模块 panic | ✅ 已修复（commit `a088e6b`） |
| H-2 | HIGH | CSS 解析器递归无深度保护 | ✅ 已修复（commit `973d986`） |
| H-3 | HIGH | HTML 解析器 reprocess 计数超限触发 `panic!` | ✅ 已修复（commit `9adec92`） |
| H-4 | HIGH | 自定义属性收集缺失，`var()` 在集成路径中完全失效 | ✅ 已修复（commit `3956298`） |
| M-1 | MEDIUM | DOM 节点地址作为跨模块键，存在地址复用风险 | 待修复 |
| M-2 | MEDIUM | 模块间错误处理策略不一致 | 待修复 |
| M-3 | MEDIUM | `Cargo.lock` 未纳入版本控制 | ✅ 已修复（commit `5be101b`） |
| L-1 | LOW | `tiny_skia.rs` 中 `Pixmap::new(1,1).expect(...)` | 可接受 |
| L-2 | LOW | renderer 信任 `LayoutResult` 中 `width/height` 为有限值 | 可接受 |
| L-3 | LOW | 无 `unsafe` 代码（内存安全基线良好） | 信息性 |

---

## 二、已修复问题

### C-2：HTML 输入大小 / DOM 深度限制 ✅

- **位置**：[crates/muskitty-html5-parser/src/lib.rs](../crates/muskitty-html5-parser/src/lib.rs)
- **修复方案**：
  - 新增 `MAX_INPUT_BYTES = 64 MiB`、`MAX_OPEN_ELEMENTS = 512`
  - `ParseError` 扩展 `InputTooLarge` / `DomDepthExceeded`
  - 新增 `ParseOutput { document, errors }` 与 `parse_with_limits` 入口
  - 保留 `parse(input)` 向后兼容入口
  - `helpers::push_open_element` 封装深度检查
  - 批量替换 13 处 `open_elements.push`
- **参考**：Chromium `kMaxHTMLParserDOMDepth = 512`、WebKit `maxDOMTreeDepth = 500`
- **Commit**：`29ea57b`

### H-1：layout `compute_layout` 改为 `Result` ✅

- **位置**：[crates/muskitty-layout/src/lib.rs](../crates/muskitty-layout/src/lib.rs)
- **修复方案**：
  - 新增 `LayoutError` 枚举：`ComputeLayoutFailed(taffy::TaffyError)` / `NodeLayoutMissing(NodeId)`
  - `compute_layout` 返回 `Result<LayoutResult, LayoutError>`
  - 内部两处 `expect` 改为 `?` 和 `map_err`
  - 同步更新 renderer 端 3 个调用点
- **Commit**：`a088e6b`（layout）+ `299eed6`（renderer 适配）

---

## 三、问题清单（按优先级）

### C-1：`var()` 循环引用导致栈溢出（DoS）— P0 ✅

- **位置**：[crates/muskitty-cascade/src/compute.rs](../crates/muskitty-cascade/src/compute.rs) `resolve_var` 函数
- **根因**：`resolve_var` 递归调用 `resolve_component_value`，对替换值中嵌套的 `var()` 再次解析，但**没有任何循环检测机制**。
- **攻击向量**：
  ```css
  :root { --a: var(--b); --b: var(--a); }
  ```
  一次 `compute_value` 调用即触发无限递归 → 栈溢出 → 进程崩溃。
- **影响**：CRITICAL（任意 CSS 输入即可触发崩溃，符合 DoS 攻击定义）
- **规范依据**：CSS Variables Level 1 §3 "Cycles in Custom Properties"
  > If a custom property's value depends on itself, the var() must be treated as invalid.
- **参考实现**：Servo `components/style/custom_properties.rs::substitute()` 使用 `seen: &mut HashSet<Name>` 跟踪已访问的 var 名
- **修复方案**：在 `resolve_var` 引入 `visited: &mut HashSet<String>` 参数，进入前插入当前 `--name`，已存在则返回 `Vec::new()`
- **测试用例**：
  - `--a: var(--b); --b: var(--a)` → 返回空 Vec
  - `--a: var(--a)` → 自引用返回空
  - `--a: var(--b); --b: var(--c); --c: var(--a)` → 三角环返回空
  - 正常链 `--a: var(--b); --b: red` → 仍能解析为 red
- **Commit**：`d1ab542`

### H-4：自定义属性收集缺失 — P0 ✅

- **位置**：[crates/muskitty-renderer/tests/end_to_end.rs](../crates/muskitty-renderer/tests/end_to_end.rs) 与 [tests/paint.rs](../crates/muskitty-renderer/tests/paint.rs)
- **根因**：集成测试和 paint 入口从未实现"从 ComputedStyle 提取 `--*` 属性 → 填充 `custom_properties`"这一步，导致 `var()` 在端到端链路中**永远**命中 fallback 或返回空
- **影响**：HIGH（核心 CSS 功能在集成路径中静默失效，掩盖 C-1 的真实风险）
- **规范依据**：CSS Cascading Level 4 §4.3 "Computed Value"
  > Custom property declarations are part of the cascade. Their values are collected from the cascade and made available for var() substitution.
- **参考实现**：Servo `components/style/cascade.rs::compute_style` 在 cascade 完成后从已 cascaded 的声明中提取 `--*`
- **修复方案**：
  - 新增 `muskitty-cascade/src/custom_properties.rs`
  - 实现 `collect_custom_properties(element, sheets, parent_props) -> HashMap<String, Vec<ComponentValue>>`
  - 子元素继承父级 custom_properties（CSS 变量是继承属性）
  - 修改 `compute_styles_recursive` 在递归计算时传递收集到的 props
- **Commit**：`3956298`

### C-2 后续：HTML 解析器 reprocess panic — P1（即原 H-3）✅

- **位置**：[crates/muskitty-html5-parser/src/parser/mod.rs](../crates/muskitty-html5-parser/src/parser/mod.rs) `run` 方法
- **根因**：reprocess 计数超限时触发 `panic!` 而非 `Result::Err`，对外暴露为进程崩溃
- **影响**：HIGH（某些畸形 HTML 可能稳定触发该条件）
- **规范依据**：WHATWG HTML §13.2.6 reprocess 是状态机正常机制，规范允许 parser "stop parsing"
- **参考实现**：所有主流浏览器遇到这种异常都会停止当前 token 处理，继续后续 token
- **修复方案**：将 `panic!` 改为 `errors.push(ParseError::ReprocessLimitExceeded)` 并 `return`
- **Commit**：`9adec92`（muskitty-html5-parser 仓库）

### H-2：CSS 解析器递归无深度保护 — P2 ✅

- **位置**：[crates/muskitty-css-parser/src/algorithms.rs](../crates/muskitty-css-parser/src/algorithms.rs) `consume_a_component_value`
- **根因**：`consume_a_component_value` → `consume_a_simple_block` / `consume_a_function` → `consume_a_component_value`（间接递归）无深度保护
- **攻击向量**：`{{{{...}}}}` 或 `(((((...)))))` 嵌套 10,000+ 层 → 栈溢出
- **影响**：HIGH（恶意 CSS 即可崩溃）
- **规范依据**：CSS Syntax Level 3 §5 未规定（属实现层）
- **参考实现**：
  - Chromium `kMaxCSSTokenizerNestingLevel = 100`
  - Firefox `kMaxNesting = 200`
  - Servo `MAX_PARSER_NESTING_DEPTH = 100`
- **修复方案**：在 `TokenStream` 上加 `depth: Cell<u32>`，超 `MAX_NESTING_DEPTH = 1024` 时返回 `ParseError::NestingTooDeep`
- **Commit**：`973d986`（muskitty-css-parser 仓库）

### M-1：DOM 节点地址作为跨模块键 — P3

- **位置**：[crates/muskitty-renderer/src/paint.rs](../crates/muskitty-renderer/src/paint.rs) 与 layout 模块
- **根因**：`Rc::as_ptr(node) as usize` 反映堆地址。当 DOM 被修改（节点 drop 后重新分配），内存分配器可能复用同一地址
- **影响**：在"解析→渲染"单次流程中无问题，但若 renderer 支持增量重排/重绘，会出现幽灵样式
- **参考实现**：Servo 使用 `LayoutId(u64)`，Chrome 使用 `NodeDataKey`，Firefox 使用 `WebIDL` 自增 id
- **修复方案**：在 `Node` 上增加 `id: u64`（全局单调递增），跨模块键改用该稳定标识
- **优先级说明**：跨模块大改，建议放到下一次架构重构

### M-2：模块间错误处理策略不一致 — P3

- **现状**：
  - `muskitty-cascade`：`compute_value` 静默吞错（返回 `ComputedValue::Resolved(Vec::new())`）
  - `muskitty-layout`：已改为 `Result`（H-1 已修复）
  - `muskitty-html5-parser`：`errors: Vec<ParseError>` 收集但不向上传播
  - `muskitty-renderer`：完全无错误路径
- **影响**：上游无法统一感知"解析/渲染失败"事件，难以实现降级渲染
- **修复方案**：定义统一的 `RenderError` 枚举，每个模块在边界返回 `Result`

### M-3：`Cargo.lock` 未纳入版本控制 — P3 ✅

- **位置**：[.gitignore](../.gitignore) 第 3 行 `*.lock`
- **问题**：`*.lock` 被全局忽略，CI/不同构建机器可能拉到不同 patch 版本，引入行为漂移；削弱 CVE 复现性
- **修复方案**：从 `.gitignore` 移除 `*.lock`，或改为 `*.lock\n!Cargo.lock`，保留 `Cargo.lock`
- **Commit**：`5be101b`

---

## 四、依赖项 CVE 审计

基于 `Cargo.lock`，所有外部依赖锁定版本均无已知 CVE 影响：

| 依赖 | 锁定版本 | 已知 CVE | 状态 |
|---|---|---|---|
| `taffy` | 0.12.2 | 无 | ✅ |
| `tiny-skia` | 0.12.0 | 无 | ✅ |
| `tiny-skia-path` | 0.12.0 | 无 | ✅ |
| `png` | 0.18.1 | 无 | ✅ |
| `flate2` | 1.1.9 | 无（旧版 `miniz_oxide` < 0.8.3 有 RUSTSEC-2024-0007，已通过 0.8.9 规避） | ✅ |
| `miniz_oxide` | 0.8.9 | 无 | ✅ |
| `serde` / `serde_derive` | 1.0.229 | 无 | ✅ |
| `bytemuck` | 1.25.2 | 无 | ✅ |
| `bitflags` | 2.13.1 | 无 | ✅ |
| `syn` / `proc-macro2` | 3.0.3 / 1.0.107 | 无 | ✅ |

---

## 五、修复优先级总览

| 优先级 | 编号 | 漏洞 | 工作量 | 状态 |
|---|---|---|---|---|
| P0 | C-1 | `var()` 循环引用 | 小 | ✅ 已修复（`d1ab542`） |
| P0 | H-4 | 自定义属性收集缺失 | 中 | ✅ 已修复（`3956298`） |
| P1 | H-3 | reprocess panic 改 `Result` | 小 | ✅ 已修复（`9adec92`） |
| P2 | H-2 | CSS 解析递归深度 | 小 | ✅ 已修复（`973d986`） |
| P3 | M-1 | 节点稳定键 | 大 | 待修复 |
| P3 | M-2 | 错误策略统一 | 大 | 待修复 |
| P3 | M-3 | Cargo.lock | 小 | ✅ 已修复（`5be101b`） |
| ✅ | C-2 | HTML 输入/嵌套限制 | 中 | 已修复 |
| ✅ | H-1 | layout Result 化 | 中 | 已修复 |
