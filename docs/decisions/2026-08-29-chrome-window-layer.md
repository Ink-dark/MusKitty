# ADR: 浏览器 Chrome 窗口层（muskitty-chrome，自绘非原生 UI）

> **日期**：2026-08-29
> **状态**：已接受
> **关联**：[chrome-window-layer 规划](../plans/2026-08-29-chrome-window-layer.md)、[Servo 窗口层研究](../research/2026-08-23-servo-window-layer-analysis.md)、[外部依赖解耦 ADR](2026-08-16-external-dependency-decoupling.md)

## 背景

W-1~W-5 交付的 `muskitty-shell` 只有裸像素窗口（PlatformWindow + softbuffer present），
无标签栏、无地址栏，标签状态只能写进系统标题栏（W-5 验证时暴露的观测缺口）。
用户决策：**为浏览器单独做一个窗口层，弃用 shell 的窗口角色**，参考
Chromium / Firefox / Servo / Zed 的非系统原生 UI 窗口设计——即 chrome（浏览器外壳
UI）由应用自绘并与页面合成，不使用系统原生控件。

## 参考（research 结论）

| 项目 | chrome 方案 | 借鉴点 |
|------|------------|--------|
| Chromium | Views/aura 自绘合成，标签入标题栏 | "chrome 即合成像素" + 独立 chrome 布局/命中测试/绘制分层 |
| Firefox | XUL → 自绘（Gecko 渲染） | 同上；工具栏布局模型 |
| Servo | servoshell `gui.rs`：egui 自绘工具栏 + 页面合成（§3 研究文档） | 合成模型：chrome 与页面同帧呈现 |
| Zed | GPUI 全自绘，无系统控件 | 自绘文本/命中测试的可行性上限 |

## 备选与取舍

| 方案 | 结论 |
|------|------|
| **egui**（servoshell 同款） | ❌ egui 仅有 GPU 绘制后端（glow/wgpu），与 MusKitty 的 tiny-skia CPU 页面管线冲突（需引入 GPU 纹理上传路径）；通用 immediate-mode 样式体系对浏览器 chrome 是负资产；违背"从零重写"精神 |
| **iced** | ❌ 有 tiny-skia 软件后端，但 retained 组件/样式模型拥有事件循环与渲染入口，逐帧更新页面纹理别扭；定制浏览器外观要与框架样式系统对抗；为一条工具栏引入框架级依赖违反 Simplicity |
| **自绘 chrome**（选定） | ✅ Chromium Views 同构：chrome 布局 → 绘制 → 命中测试全是本 crate 代码；复用已验证的 tiny-skia + cosmic-text 0.13 + swash 文本管线（renderer `draw_text` 同款）；零 C/C++ 依赖不变；无窗口环境（CI）可对 chrome 布局/绘制/输入做纯函数像素测试（W-4 价值平移） |

## 决策

1. **新建 `muskitty-chrome` crate** 作为浏览器窗口层：chrome 自绘（标签栏 + 工具栏 +
   地址栏）+ 页面视口合成 + winit/softbuffer 呈现。`muskitty-shell` 的窗口角色
   被**整体取代**，其可复用资产（`page.rs` 渲染管线 / `webview.rs` 标签状态 /
   快捷键 / headless 渲染）迁移进 chrome crate 后**删除 shell crate**（git 历史保留）。
2. **合成模型**（Chromium Views 式）：页面按 active 标签渲染为物理分辨率 RGBA；
   chrome 按 chrome 区域绘制；同帧合成进一张全窗口 Pixmap → softbuffer present。
   chrome 不与页面重叠（v1：chrome 条在上、页面视口在下）。
3. **纯函数分层**（可无窗口测试）：
   - `chrome::model` — `ChromeState`（地址栏文本/焦点、hover）+ `layout_chrome(...) -> ChromeRects`
   - `chrome::paint` — chrome → tiny-skia Pixmap（文本经 cosmic-text 整形 + swash
     outline → tiny-skia 路径，方案与 renderer `draw_text` 一致）
   - `chrome::input` — `hit_test(rects, x, y)` + `apply(state, event) -> 效果`（纯函数）
   - `compositor` — 页面 RGBA + chrome → 全窗口 RGBA
4. **feature gate 沿用 W-4 模式**：`winit-backend` 默认开；`--no-default-features`
   下仅模型/绘制/输入/合成（纯函数）+ `render_to_png` 可编译可测，CI 无窗口跑通。
5. **v1 范围裁剪**：保留系统标题栏（自定义窗口边框/decorations-off 延后）；
   地址栏光标恒在末尾（无光标移动/选区/IME，延后）；后退/前进按钮画出来但禁用
   （历史栈未建）；favicon 占位（network 未接轨，沿 W-5 裁剪）；标签拖拽重排延后。

## 后果

- `PlatformWindow` trait（shell）随 crate 退役；chrome crate 直接管理 winit 窗口
  （自绘 chrome 与 PlatformWindow 的"通用窗口"抽象目标冲突，保留两层抽象违反
  Simplicity——等第二个窗口形态出现再抽）。
- 渲染管线入口从 `muskitty_shell::page` 变为 `muskitty_chrome::page`；W-4 的
  无窗口渲染测试语义不变。
- chrome 视觉基线取 Chromium light 配色（#dee1e6 标签条 / 白工具栏 / #f1f3f4
  地址栏），仅为可辨识，非像素级复刻。
