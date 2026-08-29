# Goal — 下一步任务清单（给 Codex / AI Agent 用）

> **更新时间**：2026-08-29
> **当前状态**：**新方向（用户决策）**：为浏览器单独做窗口层，弃用 muskitty-shell 的窗口角色（过于简陋：无标签栏/地址栏，标签状态只能写系统标题栏）。新建 **`muskitty-chrome`**——自绘非原生 UI 的浏览器 chrome（参考 Chromium Views / Firefox / Servo servoshell / Zed GPUI 的"chrome 即合成像素"设计），决策与取舍见 [docs/decisions/2026-08-29-chrome-window-layer.md](docs/decisions/2026-08-29-chrome-window-layer.md)。规划见 [docs/plans/2026-08-29-chrome-window-layer.md](docs/plans/2026-08-29-chrome-window-layer.md)。W-1~W-5（shell 窗口轨道）已完成并**整体被本轮取代**；shell 的 page/webview/快捷键/headless 资产迁移进 chrome crate 后删除 shell。

---

## 当前阶段定位

- **本轮主线**：chrome 窗口层（D-1~D-7 commit 序列，见规划文档）。
- **历史**：W-1~W-5（PlatformWindow→DPI→输入→Headless→多标签）保持 git 历史，其功能语义（每标签渲染状态、脏位延迟 flush、快捷键、无窗口渲染测试）由 chrome crate 承接。
- **中期主线**：M-1 网络接轨 / M-2 交互基础排在 chrome 层之后；M-3 余项（revert 真语义 / background-image / 方向性 border / outline）按需求裁剪延后（PROGRESS.md item 15）。

## 任务列表：muskitty-chrome 窗口层

规划源：[docs/plans/2026-08-29-chrome-window-layer.md](docs/plans/2026-08-29-chrome-window-layer.md)（视觉规格、模块布局、范围裁剪均在其中）。

- [ ] D-0 `[docs]` ADR + 规划 + goal.md 重写
- [ ] D-1 `[chrome]` crate 骨架 + workspace member（feature gate winit-backend；过渡期 path 依赖 muskitty-shell；`--no-default-features` 可编译）
- [ ] D-2 `[chrome]` `chrome::model`：ChromeState / ChromeRects / layout_chrome 纯函数 + 单测
- [ ] D-3 `[chrome]` `chrome::paint` + `compositor`：chrome 绘制（标签/按钮/地址栏/文本，cosmic-text+swash 同 renderer 方案）+ 页面合成 + 无窗口像素断言
- [ ] D-4 `[chrome]` `chrome::input`：hit_test + apply 纯函数 + 单测（标签切换/关闭/新建、地址栏聚焦/输入/回车）
- [ ] D-5 `[chrome]` app.rs + main.rs：winit 循环 + 标签集合 + 脏位 flush + 快捷键 + 地址栏提交（UrlSubmitted → 标签标题更新）；真窗口桌面自动化验证
- [ ] D-6 `[chrome]` headless：render_to_png + render_window_to_png（页面+chrome 合成）无窗口测试（`--no-default-features` 全绿）
- [ ] D-7 `[migration]` git mv page/webview/shortcut/headless 自 shell → chrome；删除 muskitty-shell crate；文档同步

**每 commit 退出条件（全部满足才可 commit）**：

- [ ] `cargo check --workspace` / `cargo test --workspace` 通过
- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零警告
- [ ] `cargo check -p muskitty-chrome --no-default-features` 无头可编译（D-1 起；D-6 起 test 同样全绿）
- [ ] chrome 公共 API 零外部依赖类型泄漏（winit/softbuffer/tiny-skia/cosmic-text）
- [ ] D-5 真窗口验证：chrome 可见（标签条/工具栏/地址栏）、Ctrl+T/W/1/PageUp、地址栏输入回车标题更新、×/+ 点击生效

## 不在本轮范围（显式排除）

- 自定义窗口边框（decorations-off）、标签拖拽重排
- 地址栏光标移动 / 选区 / IME / 自动补全
- 后退/前进历史栈（按钮禁用态）、favicon、页面内事件命中测试
- GPU 合成 / 瓦片化；Network 自研 HTTP 栈；完整 inline formatting context；文本渲染性能优化
