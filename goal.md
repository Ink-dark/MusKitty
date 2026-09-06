# Goal — 网络层接驳（Phase 5 → chrome 导航，2026-09-06）

> **更新时间**：2026-09-06
> **当前状态**：已完成。T-1~T-4 全部落地（commits `412aa68` / `6c7c3b7` / `215ffc9`
> / docs），测试 10+4（network）与 85+3（chrome，`--no-default-features` 75+3）全绿。
> **上一轮**（WPT 套件补全 + 合规度实测）已完成推送。
> **用户指令**：接驳网络层——把 Phase 5 的 `muskitty-network`（trait + reqwest 后端）
> 接进 chrome 地址栏导航，结束"地址栏提交只回显占位页"的状态
> （前置条件 F-14 体积上限/超时已修，见 audit S-6）。

## 当前阶段定位

- **接驳点**：chrome `app.rs` 的 `ChromeEffect::UrlSubmitted`（目前占位页，注释"M-1 延后"）。
- **方式**：chrome 新增 `navigation` 模块（URL 分类 + 后台线程抓取 + 响应→文档转换）；
  `muskitty-network` 补同步便捷入口 `fetch_blocking`——chrome 是同步 UI 层，异步运行时
  细节留在网络 crate，未来换自研后端时 chrome 零改动（trait 抽象初衷）。
- **范围**：**顶级文档 GET 导航**（HTML Standard §7.2 navigation 的极简子集）。
  子资源（`<link>`/`<img>`）、历史栈、刷新语义不在本轮。

## 任务清单（每项 = 1 个 commit）

- [x] T-1 `[network]` `fetch_blocking` 同步便捷函数 + `NetworkResponse::new` 转正为
      pub（chrome 层测试需构造响应）。**退出（`412aa68`）**：wiremock 离线测试
      （blocking 成功 + 连接拒绝）全绿，10 + 4 doc-tests；check/fmt/clippy 零警告。
- [x] T-2 `[chrome]` `navigation` 模块：`classify_url`（http/https/file/不支持 scheme
      分发；无 scheme 补 https，localhost 补 http）、`document_from_response`
      （Content-Type 分发：html / plain→pre / 其他→提示页；4xx/5xx 正文照常渲染）、
      `error_page`、`spawn_http_navigation`（独立线程 + channel 回传，不阻塞 UI）；
      `WebView` 加导航代数字段（过期结果丢弃）。**退出（`6c7c3b7`）**：纯函数单测 +
      原生 TcpListener 离线端到端全绿（真 reqwest → 真线程 → 转换），
      `--no-default-features` 下照常编译测试。
- [x] T-3 `[chrome]` app 接线：地址栏提交按分类导航（加载期间保留旧页、标题先更新、
      到站后回填 + 重绘）；事件循环 `about_to_wait` 吸干结果；过期导航/已关标签
      静默丢弃。**退出（`215ffc9`）**：app 层 5 个新单测（到站应用 / 过期丢弃 /
      错误页 / file 加载 / 占位页 / 入队）全绿，85+3。
- [x] T-4 `[docs]` goal / PROGRESS / phase5 计划 / AGENTS 同步。**退出**：
      四处文档与实况一致（"暂不接轨"表述清除）。

## 显式排除（后续轮）

- 子资源加载（`<link rel=stylesheet>` / `<img>`）、相对 URL 解析、历史栈
  （后退/前进按钮仍为占位）、MIME 嗅探、cookie、cache。
- 自研 HTTP 栈（N-1~N-7 路线图不变）。

## 验证

```bash
cargo check --workspace
cargo test -p muskitty-network
cargo test -p muskitty-chrome        # 含 --no-default-features（CI 无窗口）
cargo fmt -p muskitty-network -p muskitty-chrome -- --check
cargo clippy -p muskitty-network -p muskitty-chrome --all-targets -- -D warnings
# 人工验证（架构师，需窗口环境）：
cargo run -p muskitty-chrome   # 地址栏输入 example.com → 渲染真实页面
```
