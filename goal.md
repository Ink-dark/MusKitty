# Goal — 下一步任务清单（给 Codex / AI Agent 用）

> **创建时间**：2026-08-02
> **前置状态**：Phase 4 Renderer B-3 / B-4 已完成（DOM→CSS→Layout→Render 全链路打通）；全工作区安全审计完成（见 [docs/security-audit-2026-08-02.md](docs/security-audit-2026-08-02.md)）；未剥离 crate（cascade/cssom/renderer）已纳入主仓库 workspace，剥离任务暂停。
> **用户决策**：硬性剥离没好处后续还是会炸，干脆按工作区需求来一次性在工作区里面 fetch crates 跑的也方便。

---

## 当前阶段定位

- HTML/CSS/Layout/Renderer 四层链路打通，最小可运行 demo 已工作（HTML+CSS → PNG）
- **当前阻塞**：安全审计发现 7 个待修复漏洞，其中 P0 两个会直接导致崩溃或核心功能失效
- **本目标**：清掉所有 P0/P1/P2 漏洞，使工作区进入"可安全处理任意输入"状态
- **不在本目标范围**：M-1（节点稳定键）/ M-2（错误策略统一）属架构级大改，留到下一轮；crate 剥离任务暂停

---

## 任务列表（按优先级）

### Task 1 — 修复 C-1：`var()` 循环引用导致栈溢出（P0）

**位置**：[crates/muskitty-cascade/src/compute.rs](crates/muskitty-cascade/src/compute.rs) `resolve_var` 函数

**根因**：`resolve_var` 递归调用 `resolve_component_value`，对替换值中嵌套的 `var()` 再次解析，无循环检测机制。`--a: var(--b); --b: var(--a);` 即触发栈溢出。

**规范依据**：CSS Variables Level 1 §3 "Cycles in Custom Properties"
> If a custom property's value depends on itself, the var() must be treated as invalid.

**参考实现**：Servo `components/style/custom_properties.rs::substitute()` 使用 `seen: &mut HashSet<Name>` 跟踪已访问的 var 名

**实施步骤**：
1. 在 `resolve_var` 签名中加入 `visited: &mut HashSet<String>` 参数
2. 进入函数后先检查当前 `--name` 是否已在 `visited` 中：是则返回 `Vec::new()`（视为 invalid）
3. 否则插入 `visited`，继续解析替换值
4. 递归调用 `resolve_component_value` 时透传 `visited`
5. 更新所有 `resolve_var` 的调用点

**测试用例**（必须在 `crates/muskitty-cascade/tests/` 或 `src/compute.rs` 的 `#[cfg(test)]` 模块中新增）：
- `--a: var(--a)` 自引用 → 返回空 Vec
- `--a: var(--b); --b: var(--a)` 双环 → 返回空
- `--a: var(--b); --b: var(--c); --c: var(--a)` 三角环 → 返回空
- `--a: var(--b); --b: red` 正常链 → 仍能解析为 `red`

**退出条件（全部满足才可 commit）**：
- [ ] `resolve_var` 签名含 `visited: &mut HashSet<String>` 参数
- [ ] 至少 4 个循环检测单元测试存在并通过
- [ ] `cd crates/muskitty-cascade && cargo test` 全绿
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --all -- --check` 通过
- [ ] 单 commit，message 格式 `[cascade] fix var() cycle detection, refs CSS-Vars §3`
- [ ] 已 `git push origin main`

---

### Task 2 — 修复 H-4：自定义属性收集缺失，`var()` 在集成路径中完全失效（P0）

**位置**：[crates/muskitty-cascade/src/](crates/muskitty-cascade/src/) 与 [crates/muskitty-renderer/tests/](crates/muskitty-renderer/tests/)

**根因**：集成测试和 paint 入口从未实现"从 ComputedStyle 提取 `--*` 属性 → 填充 `custom_properties`"，导致 `var()` 在端到端链路中永远命中 fallback 或返回空。这会掩盖 C-1 的真实风险。

**规范依据**：CSS Cascading Level 4 §4.3 "Computed Value"
> Custom property declarations are part of the cascade. Their values are collected from the cascade and made available for var() substitution.

**参考实现**：Servo `components/style/cascade.rs::compute_style` 在 cascade 完成后从已 cascaded 的声明中提取 `--*`

**实施步骤**：
1. 新增 `crates/muskitty-cascade/src/custom_properties.rs`
2. 实现 `collect_custom_properties(element, sheets, parent_props) -> HashMap<String, Vec<ComponentValue>>`
3. 子元素继承父级 custom_properties（CSS 变量是继承属性）
4. 修改 `compute_styles_recursive` 在递归计算时传递收集到的 props
5. 在 `crates/muskitty-renderer/tests/end_to_end.rs` 与 `tests/paint.rs` 中新增用例验证 `var()` 端到端生效

**测试用例**：
- `:root { --brand: red } div { color: var(--brand) }` → div 的 color 解析为 red
- `:root { --brand: red } .child { --brand: blue } .child .grand { color: var(--brand) }` → grand 解析为 blue（继承 + 覆盖）
- `:root { --x: var(--y); --y: green } p { color: var(--x) }` → 解析为 green（链式 var）
- 父级未声明 → 子级 `var(--missing)` 命中 fallback 或返回空

**退出条件**：
- [ ] `crates/muskitty-cascade/src/custom_properties.rs` 存在并 export `collect_custom_properties`
- [ ] `compute_styles_recursive` 在递归时传递 custom_properties
- [ ] `crates/muskitty-renderer/tests/end_to_end.rs` 至少 2 个 var() 端到端用例通过
- [ ] `cd crates/muskitty-renderer && cargo test` 全绿
- [ ] `cd crates/muskitty-cascade && cargo test` 全绿
- [ ] `cargo clippy --all-targets -- -D warnings`（在工作区根跑）零警告
- [ ] `cargo fmt --all -- --check` 通过
- [ ] 单 commit，message `[cascade] collect custom properties for var() substitution, refs Cascade L4 §4.3`
- [ ] 已 `git push origin main`

---

### Task 3 — 修复 H-3：HTML 解析器 reprocess 计数超限触发 `panic!`（P1）

**位置**：[crates/muskitty-html5-parser/src/parser/mod.rs](crates/muskitty-html5-parser/src/parser/mod.rs) `run` 方法

**根因**：reprocess 计数超限时触发 `panic!` 而非 `Result::Err`，对外暴露为进程崩溃。畸形 HTML 可能稳定触发。

**规范依据**：WHATWG HTML §13.2.6 reprocess 是状态机正常机制，规范允许 parser "stop parsing"

**参考实现**：所有主流浏览器遇到这种异常都会停止当前 token 处理，继续后续 token

**实施步骤**：
1. 在 `crates/muskitty-html5-parser/src/error/mod.rs` 的 `ParseError` 枚举新增 `ReprocessLimitExceeded { limit: u32 }` 变体
2. 在 `run` 方法中将 `panic!(...)` 改为 `errors.push(ParseError::ReprocessLimitExceeded { limit }); return;`
3. 确认 `ParseOutput.errors` 能正确传递该错误

**测试用例**：
- 构造能触发 reprocess limit 的输入（参考 `MAX_REPROCESS_COUNT` 常量），验证返回 `ParseOutput` 而非 panic
- `parse_with_limits` 返回的 `errors` 中含 `ReprocessLimitExceeded`
- document 仍为有效（部分构造的）DOM 树

**退出条件**：
- [ ] `ParseError::ReprocessLimitExceeded` 变体存在
- [ ] `run` 方法中无 `panic!` 调用（针对 reprocess 限制路径）
- [ ] 至少 1 个单元测试验证畸形输入不触发 panic
- [ ] `cd crates/muskitty-html5-parser && cargo test` 全绿
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --all -- --check` 通过
- [ ] 单 commit，message `[html5-parser] convert reprocess limit panic to ParseError, refs WHATWG §13.2.6`
- [ ] 已 `git push origin main`

---

### Task 4 — 修复 H-2：CSS 解析器递归无深度保护（P2）

**位置**：[crates/muskitty-css-parser/src/algorithms.rs](crates/muskitty-css-parser/src/algorithms.rs) `consume_a_component_value` / `consume_a_simple_block` / `consume_a_function`

**根因**：间接递归无深度保护。`{{{{...}}}}` 或 `(((((...)))))` 嵌套 10,000+ 层即栈溢出。

**规范依据**：CSS Syntax Level 3 §5 未规定（属实现层）

**参考实现**：
- Chromium `kMaxCSSTokenizerNestingLevel = 100`
- Firefox `kMaxNesting = 200`
- Servo `MAX_PARSER_NESTING_DEPTH = 100`

**实施步骤**：
1. 在 `TokenStream` 上加 `depth: Cell<u32>`（或 `RefCell<u32>`，取决于现有实现）
2. 定义常量 `MAX_NESTING_DEPTH = 1024`（取参考实现上界，留足余量）
3. 在 `consume_a_simple_block` / `consume_a_function` 入口处 `depth.replace(depth.get() + 1)`，超限时返回 `ParseError::NestingTooDeep`
4. 出口处 `depth.replace(depth.get() - 1)`
5. 在 `ParseError` 枚举中新增 `NestingTooDeep { depth: u32, limit: u32 }` 变体

**测试用例**：
- 构造 10,000 层 `{` 嵌套 → 不触发栈溢出，返回 `NestingTooDeep` 错误
- 构造 10,000 层 `(` 嵌套 → 同上
- 正常深度（如 100 层）→ 正常解析

**退出条件**：
- [ ] `TokenStream` 上有 `depth` 字段
- [ ] `MAX_NESTING_DEPTH = 1024` 常量定义
- [ ] `ParseError::NestingTooDeep` 变体存在
- [ ] 至少 2 个单元测试验证深嵌套输入不栈溢出
- [ ] `cd crates/muskitty-css-parser && cargo test` 全绿
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --all -- --check` 通过
- [ ] 单 commit，message `[css-parser] enforce MAX_NESTING_DEPTH to prevent stack overflow`
- [ ] 已 `git push origin main`

---

### Task 5 — 修复 M-3：`Cargo.lock` 未纳入版本控制（P3）

**位置**：[.gitignore](.gitignore) 第 3 行 `*.lock`

**根因**：`*.lock` 被全局忽略，CI / 不同构建机器可能拉到不同 patch 版本，引入行为漂移，削弱 CVE 复现性

**实施步骤**：
1. 从 `.gitignore` 移除 `*.lock` 行，或改为 `*.lock` + `!Cargo.lock`（保留其他 lock 文件忽略）
2. `git add Cargo.lock`
3. 验证 `cargo check --workspace` 仍通过

**退出条件**：
- [ ] `.gitignore` 不再忽略 `Cargo.lock`
- [ ] `Cargo.lock` 已纳入版本控制
- [ ] `cargo check --workspace` 通过
- [ ] 单 commit，message `[workspace] track Cargo.lock for reproducible builds`
- [ ] 已 `git push origin main`

---

## 全部任务完成的最终退出条件

当且仅当以下全部满足时，本轮目标完成：

- [ ] Task 1 ~ Task 5 各自的退出条件全部满足
- [ ] `cargo check --workspace` 零错误零警告
- [ ] `cargo test --workspace` 全绿
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --all -- --check` 通过
- [ ] [docs/security-audit-2026-08-02.md](docs/security-audit-2026-08-02.md) 中 C-1 / H-4 / H-3 / H-2 / M-3 五项状态从"待修复"改为"✅ 已修复"并附 commit hash
- [ ] 所有 commit 已 push 到 `origin/main`
- [ ] 未引入新的 `unsafe` 代码
- [ ] 未进行任何 crate 剥离操作（按用户决策暂停）

完成后 agent 自行退出，等待用户下一轮指令。

---

## 不在本目标范围（显式排除）

- **M-1 节点稳定键** — 跨模块大改，放到下次架构重构
- **M-2 错误策略统一** — 跨模块大改，放到下次架构重构
- **crate 剥离**（cascade / cssom / renderer 独立 git 仓库）— 用户明确暂停
- **Phase 5 Network** — 远期工作
- **GPUI 后端集成** — Phase 4 后续，不在本轮安全修复范围
