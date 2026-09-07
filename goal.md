# Goal — P0 止血批核查与集成轮（2026-09-07）

> **更新时间**：2026-09-07
> **当前状态**：已完成。审计 [audit-2026-09-06-vuln-arch-perf.md](docs/audit-2026-09-06-vuln-arch-perf.md)
> 的 4 项 P0（V-1 / CSS-P1 / SEL-1 / SEL-2）**已由架构师在 muskitty-dev 各 crate 仓库
> 远端先行落地**（`ink-dark/p0-fixes` → PR #1/#2）；本轮工作为**合并、全量验证与
> 主仓库集成**，非重复实现。
> 上一轮（网络层接驳 + 真机检测抓出 rgba_to_0rgb R/B 互换）已完成推送（9562f0f）。

## 核查结论（以实跑为准）

| P0 | 修复 commit（crate 仓库） | 本轮验证 |
|---|---|---|
| V-1 var() 输出预算 | cascade `34f94b5`（emitted 全局计数 + extend 前预检，100k/属性） | 本地曾并行实现同修复（987950f），merge 以架构师版本为准（预检不分配超限缓冲，更优）；89 lib + 108 集成全绿 |
| CSS-P1 规则级嵌套守卫 | css-parser `efbdfb8`（consume_a_block enter/leave + skip_to_matching_close_brace） | ff 至 ed45ba5；84 测试全绿；fmt/clippy 干净 |
| SEL-1 匹配步数/记忆化 | selectors `271fd50`（祖先链记忆化 D^k→D×k + 100k 步数预算——正道方案，优于纯止血计数器） | ff 至 c3d565b；169 测试 0 失败 |
| SEL-2 :has 嵌套禁止 | selectors `95388df`（has_depth 全链穿透 + forgiving 丢弃 + 候选上限 10k + 裸伪类拒绝；WPT has 夹具硬断言，380→384） | 同上；WPT harness 含硬断言通过 |

附注：架构师顺带落地了 P1 的 SEL-3（逻辑组合深度 bound，`27646a5`）与 cascade CAS-1/2/3
（Arc 化 + registry 哈希化，`8515331`）。WPT 审计中":not 内禁止 :has"的表述与 WPT
夹具不符（`parse-has.json` 明确 `.a:not(:has(.b))` valid），实现以夹具为准（仅 :has
参数内经非 forgiving 路径禁止）。

## 主仓库集成验证

- `cargo check --workspace` / `cargo test --workspace` 全绿（chrome 85+3+6、network
  10+4 等）——新 cascade 的 Arc 值模型与 chrome 渲染链路兼容。
- cascade merge commit `0f57125` 已推送 muskitty-dev/muskitty-cascade main。

## 显式排除

- P1/P2/P3 各项（除架构师已顺带落地的 SEL-3、CAS-1/2/3 外）仍开放，按审计第八节
  批次建议排后续轮。
- 真机 GUI 检测（导航分流 / 过期导航 / demo 颜色真机确认）暂挂——控制台锁定无法
  解锁；工具与脚本就绪（`Temp\muskitty-nav\`），解锁后可续。
