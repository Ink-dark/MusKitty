# Changelog

本文件记录 MusKitty 主仓库的对外可见变更（版本对齐 / 规则 / 交付物）。
各 standalone crate 的详细提交见各自 git 仓库。

格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。

## [未发布]

### 版本对齐（10 个 standalone crate 升 patch，对齐提交进度）
- 背景：排查 Linux 构建"无法解析仅有 `<p>` 标签的 HTML"时确认——`<p>` 开始标签处理位于独立 crate `muskitty-html5-parser`，且该环境未检出 11 个独立 crate 导致构建失败；当前源码在检出后即可正确解析/渲染 `<p>`。
- 为消除"打了 tag 但代码已领先 tag（`git describe --tags` 的 `N>0`）"的版本失配，将 10 个 standalone crate 的 `Cargo.toml` `version` patch +1（`muskitty-css` 的 tag 即 HEAD，未改；chrome/network/renderer 为主仓库内无独立 tag 的 member，未改）：

| crate | version | tag 后提交数 |
|---|---|---|
| muskitty-cascade | 0.1.0 → 0.1.1 | 16 |
| muskitty-css-parser | 0.3.0 → 0.3.1 | 4 |
| muskitty-css-tokenizer | 0.2.0 → 0.2.1 | 2 |
| muskitty-css-values | 0.1.0 → 0.1.1 | 1 |
| muskitty-cssom | 0.1.0 → 0.1.1 | 4 |
| muskitty-dom | 0.2.0 → 0.2.1 | 4 |
| muskitty-html5-parser | 0.2.0 → 0.2.1 | 9 |
| muskitty-html5-tokenizer | 0.1.3 → 0.1.4 | 1 |
| muskitty-layout | 0.1.0 → 0.1.1 | 16 |
| muskitty-selectors | 0.2.0 → 0.2.1 | 8 |

- 影响面：均为 patch 升级，`path =` 依赖由 path 覆盖版本号、仅作安全校验，不破坏任何依赖方 `^X.Y.Z` 要求；`cargo build --workspace` 与 `cargo test --workspace` 全绿。

### 规则
- [AGENTS.md](AGENTS.md) 新增 **Versioning Discipline**：改逻辑（修 bug / 加特性 / 换行为）必须随手 bump `Cargo.toml` `version`（patch）；判据为 `git describe --tags` 的 `N>0`；发版动作（打 tag / push / 发布 crates.io）另行人工执行。
- [Cargo.lock](Cargo.lock) 同步更新到对齐后的版本。

### 交付物
- 10 个 standalone crate 各打一个版本对齐 commit（`[muskitty-xxx] bump version ...`），均在各自仓库本地、**未 push / 未打 tag**。