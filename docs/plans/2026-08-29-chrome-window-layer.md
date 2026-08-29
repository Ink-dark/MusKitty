# 规划：muskitty-chrome — 浏览器 Chrome 窗口层（自绘非原生 UI）

> **日期**：2026-08-29
> **状态**：本轮执行
> **决策**：[ADR 2026-08-29-chrome-window-layer](../decisions/2026-08-29-chrome-window-layer.md)
> **前置**：W-1~W-5（muskitty-shell 窗口轨道）已完成并被本规划**整体取代**；shell 的
> page/webview/快捷键/headless 资产迁移进 chrome crate，shell crate 删除。

## 目标

浏览器获得自绘 chrome：标签栏（标签标题 + 关闭 × + 新建 +）+ 工具栏（后退/前进/刷新 +
圆角地址栏：点击聚焦、输入、退格、回车提交）+ 页面视口。参考 Chromium Views /
Servo servoshell / Zed 的"chrome 即合成像素"设计；不走 egui/iced 框架（理由见 ADR）。

## 模块布局

```
crates/muskitty-chrome/
├── Cargo.toml          # feature winit-backend（默认开）门控 winit/softbuffer
└── src/
    ├── lib.rs
    ├── chrome/
    │   ├── model.rs    # ChromeState / ChromeRects / layout_chrome（纯函数）
    │   ├── paint.rs    # chrome → tiny-skia Pixmap（cosmic-text+swash 文本）
    │   └── input.rs    # hit_test / apply（纯函数，ChromeHit / ChromeEffect）
    ├── compositor.rs   # 页面 RGBA + chrome → 全窗口 RGBA（纯函数）
    ├── app.rs          # winit 事件循环 + 标签集合 + flush（winit-backend 门控）
    ├── main.rs         # 演示二进制（winit-backend 门控）
    ├── page.rs         # ← shell 迁移（HTML+CSS → 像素管线）
    ├── webview.rs      # ← shell 迁移（WebView + WebViewCollection）
    ├── shortcut.rs     # ← shell input.rs 迁移（Ctrl+T/W/1~9/PageUp/Down/Esc）
    └── headless.rs     # ← shell 迁移（render_to_png + HeadlessWindow 语义）
```

依赖方向：`muskitty-chrome → {html5-parser, css, cssom, cascade, layout, renderer}`（渲染管线输入侧），
过渡期临时依赖 `muskitty-shell`（page/webview 迁移前），迁移 commit 后移除。
文本绘制：cosmic-text 0.13 + swash outline → tiny-skia 路径（与 renderer `draw_text` 同方案，本地化实现避免扩大 renderer pub API）。

## Chrome v1 视觉规格（Chromium light 基线，逻辑 px，物理 = 逻辑 × scale）

- 标签条 36 高，bg `#dee1e6`；标签高 30（顶部对齐），宽 `min(220, 可用宽/n)`，
  活动标签白底、非活动透明（hover `#e8eaed`）；标题 12px 截断加 `…`；关闭 `×` 16×16。
- 新建按钮 28×28，`+`（两矩形）。
- 工具栏 44 高，bg 白，底部 1px `#cfd4da` 分隔线；后退/前进（禁用态灰）/刷新三个
  32×32 按钮，图标为矢量路径（箭头/环形箭头）。
- 地址栏：圆角胶囊（半径=高/2），高 30，左右留白；bg `#f1f3f4`，聚焦白底 +
  `#1a73e8` 1.5px 边框；文本 13px 左对齐，光标 1px 竖线（恒在文本末尾）。
- 页面视口：y = (36+44)×scale 起。

## Commit 序列（每 commit 独立可编译 + 全门禁绿）

- [ ] D-0 `[docs]` ADR + 本规划 + goal.md 重写
- [ ] D-1 `[chrome]` crate 骨架 + workspace member（feature gate；过渡期 path 依赖 muskitty-shell；`--no-default-features` 可编译）
- [ ] D-2 `[chrome]` `chrome::model`：ChromeState / ChromeRects / layout_chrome（纯函数 + 单测：矩形随窗口宽度/标签数/缩放变化，边界不越界）
- [ ] D-3 `[chrome]` `chrome::paint` + `compositor`：chrome 绘制（标签/按钮/地址栏/文本）+ 页面合成；无窗口像素断言（chrome 背景色、活动标签白底、地址栏胶囊、文本墨迹、页面像素位置）
- [ ] D-4 `[chrome]` `chrome::input`：hit_test + apply（纯函数 + 单测：点标签切换、点 × 关闭、点 + 新建、点地址栏聚焦、字符/退格/回车、空白区未命中）
- [ ] D-5 `[chrome]` app.rs + main.rs：winit 循环 + 标签集合 + 脏位 flush + 快捷键（Ctrl+T/W/1~9/PageUp/Down/Esc）+ 地址栏提交（UrlSubmitted → 标签标题更新，观测闭环）；真窗口桌面自动化验证（截图断言 chrome 可见、快捷键、地址栏输入）
- [ ] D-6 `[chrome]` headless：render_to_png（纯页面）+ render_window_to_png（页面+chrome 合成）无窗口测试（`--no-default-features` 全绿，W-4 CI 价值平移）
- [ ] D-7 `[migration]` `git mv` page/webview/shortcut/headless 自 shell → chrome，删除 muskitty-shell crate（members 更新、引用修正）；windowing 规划文档标注"被 chrome 层取代"；goal.md/PROGRESS.md 同步

## 退出条件（每 commit 全部满足）

- [ ] `cargo check --workspace` / `cargo test --workspace` 通过
- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 零警告
- [ ] `cargo check -p muskitty-chrome --no-default-features` / `cargo test -p muskitty-chrome --no-default-features` 无头可编译可测（D-6 起）
- [ ] chrome 公共 API 零 winit/softbuffer/tiny-skia/cosmic-text 类型泄漏（decoupling ADR）
- [ ] 真窗口验证：chrome 可见（标签条/工具栏/地址栏）、Ctrl+T/W/1/PageUp、地址栏输入回车后标签标题更新、×/+ 按钮点击生效（D-5）

## 不在本轮范围（显式排除）

- 自定义窗口边框（decorations-off + 自绘标题栏）、标签拖拽重排
- 地址栏光标移动 / 选区 / IME / 自动补全
- 后退/前进历史栈（按钮禁用态）、favicon、页面内命中测试（事件仍不进页面）
- GPU 合成 / WebRender 式瓦片化
