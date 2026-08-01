# GPUI 集成调研与渲染后端决策

> **创建时间**：2026-08-01
> **所属阶段**：Phase 4 Renderer → B-0
> **调研人**：Claude（GLM-5.2）
> **结论**：**采用 tiny-skia 作为主渲染后端**，GPUI 与 vello 列为未来可选后端

---

## TL;DR

GPUI 虽已发布到 crates.io（v0.2.2），但 **官方 README 明确声明仅支持 macOS / Linux**，发布的 crate 在 Windows 上没有可用的渲染后端（`Cargo.toml` 中 `target_os = "windows"` 仅依赖 `Win32_Foundation` + `Win32_System_Power`，无 Direct3D/渲染相关 crate）。Zed 编辑器自身的 Windows 端使用 GPUI 的 **git 主干依赖**（拉取整个 zed 仓库 + 多个未发布的 workspace 依赖 crate），对独立项目不可行。

**决策**：Phase 4 Renderer 采用 **tiny-skia** 作为主后端（纯 Rust、CPU 渲染、跨平台、39.5M 下载、BSD-3-Clause 兼容、可直接输出 PNG 用于视觉验证）。GPUI / vello 留作未来 GPU 加速后端的可选项（feature flag）。

---

## 调研内容

### 1. GPUI 的 crate 结构

#### 1.1 crates.io 发布情况

| 字段 | 值 |
|------|-----|
| crate 名 | `gpui` |
| 最新版本 | `0.2.2`（9 个月前发布，约 2025-11） |
| 总版本数 | 7 |
| 总下载量 | 179,931 |
| License | Apache-2.0（兼容 muskitty） |
| SLoC | 66K |
| 仓库 | github.com/zed-industries/zed |
| 首页 | gpui.rs |

**注意**：crates.io 上另有一个名为 `gpui` 的 `0.1.0` 旧记录（Ilya Maximov，2022，MIT，1KB），是早期抢注的空壳 crate，与 Zed 的 GPUI 无关。

#### 1.2 workspace 依赖问题

GPUI 的 `Cargo.toml` 中以下依赖标记为 `workspace = true`，意味着它们解析到 Zed 仓库根 `Cargo.toml` 的 `[workspace.dependencies]`：

- `collections` / `gpui_macros` / `gpui_shared_string` / `gpui_util`
- `http_client` / `scheduler` / `sum_tree` / `util_macros`
- `gpui_platform`（dev-dependency）
- `reqwest_client` / `gpui_web`（target-specific dev-deps）

发布到 crates.io 时，这些 workspace 引用会被解析为具体版本号。但实际可用性需验证：从 crates.io 主页显示的 README 仍写「macOS or Linux」，说明即便依赖可解析，**Windows 仍不可用**。

#### 1.3 Windows 支持现状

`Cargo.toml` 中 Windows 相关配置：

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { workspace = true, features = ["Win32_Foundation", "Win32_System_Power"] }

[target.'cfg(target_os = "windows")'.build-dependencies]
# (无)
```

仅 `Win32_Foundation`（基础类型）+ `Win32_System_Power`（电源管理），**完全没有 Direct3D / DXGI / Direct2D 等渲染后端依赖**。Zed 编辑器自身的 Windows 端（v0.224.11，2026-02-26 发布）使用 GPUI 的 git 主干版本，该主干包含未发布的 DirectX 适配代码和大量 workspace 内部 crate。

**结论**：发布的 `gpui = "0.2.2"` 在 Windows 上**无法实际渲染**。要用 Windows 必须走 git 依赖：

```toml
gpui = { git = "https://github.com/zed-industries/zed", branch = "main" }
```

这会拉取整个 Zed 仓库（数百 MB + 数十个 workspace crate），对 muskitty 这类独立项目不可接受（编译时间、依赖膨胀、稳定性全失控）。

### 2. GPUI 渲染原语

从 `examples/` 目录与源码可推断的渲染原语：

| 原语 | API | 备注 |
|------|-----|------|
| 矩形 | `div()` + tailwind 风格样式 | 高层声明式 API |
| 文本 | `StyledText` / `Label` | 内置文本测量（依赖 font-kit） |
| 图片 | `img()` | 支持 SVG/PNG |
| 路径 | `Path` element + lyon | 复杂形状 |
| 变换 | `Transform` + element | 仿射变换 |
| 裁剪 | `clip()` element | 矩形/路径裁剪 |

**与 taffy 的协作**：GPUI 内部使用 `taffy = "=0.12.2"`（与 muskitty-layout 完全同版本，**版本匹配完美**），但其 taffy 用法是给 GPUI 自己的 element 树做布局，**不暴露给外部使用者**。muskitty 要复用 GPUI 的 taffy 输出，需要把自己的 LayoutResult 重新映射成 GPUI 的 element 树——这是两层布局系统，不直接复用。

### 3. GPUI 窗口系统集成

- macOS：Metal（cocoa + core-graphics + core-text）
- Linux：Wayland + X11（自带，无 winit）
- Windows：发布版**无渲染后端**；git 主干有 DirectX 适配但需整包依赖

### 4. GPUI 文本测量能力

- macOS：`font-kit`（zed fork）+ core-text
- Linux：fontconfig + freetype
- Windows：未在发布版中提供

muskitty-layout 当前跳过 text 节点的布局（layout 阶段不测量文本），即便用 GPUI 也无法直接复用其文本测量。文本测量推迟到独立阶段（可能用 cosmic-text 或 swash）。

### 5. GPUI 与 taffy 协作模式

**理想模式**（不可行）：muskitty-layout 的 LayoutResult → 喂给 GPUI 渲染。
**实际模式**：GPUI 内部已自带 taffy 布局，不接受外部 LayoutResult。muskitty 必须把 LayoutResult 转换成 GPUI 的 element 树（带绝对位置/尺寸的 div），让 GPUI 重新布局一次——这是冗余且语义错位的工作。

---

## 备选方案对比

| 维度 | GPUI (crates.io 0.2.2) | GPUI (git main) | tiny-skia 0.12 | vello 0.9 |
|------|----------------------|-----------------|----------------|-----------|
| Windows 支持 | ❌ README 明说不支持 | ⚠️ 有但需整包依赖 | ✅ 纯 Rust 跨平台 | ✅ wgpu/DirectX |
| macOS/Linux | ✅ | ✅ | ✅ | ✅ |
| 渲染方式 | GPU（Metal/Vulkan/DX） | GPU | CPU 软件 | GPU compute |
| 下载量 | 180K | — | 39.5M | 较少 |
| 成熟度 | pre-1.0，频繁破坏 | 不稳定 | 稳定（resvg 在用） | alpha（官方警告） |
| 依赖体积 | 66K SLoC + 大量 workspace crate | 整个 zed 仓库 | 11K SLoC，262 KiB | 中等（wgpu） |
| 文本渲染 | ✅（macOS/Linux） | ✅ | ❌（明确不做） | ⚠️ glyph caching 未完成 |
| PNG 输出 | ❌（需窗口） | ❌ | ✅（内置） | ⚠️ 需自行接 wgpu 转储 |
| License | Apache-2.0 | Apache-2.0 | BSD-3-Clause ✅ | Apache-2.0/MIT |
| 集成成本 | 高（完整 UI 框架） | 极高 | 低 | 中 |
| 与 taffy 关系 | 内部用 0.12.2 | 同左 | 无关 | 无关 |

### License 兼容性

- muskitty 用 Apache-2.0
- tiny-skia 用 BSD-3-Clause（与 Apache-2.0 兼容，可动态/静态链接）
- vello 用 Apache-2.0/MIT（兼容）
- GPUI 用 Apache-2.0（兼容，但不可用）

---

## 决策

### 主后端：tiny-skia

**理由**：
1. **Windows 原生可用**（用户主开发环境是 Windows）
2. **纯 Rust**（无 C 依赖，无 build.rs 复杂配置，符合 muskitty 「业务逻辑全 Rust」原则）
3. **39.5M 下载量**，被 resvg 长期使用，稳定可靠
4. **11K SLoC + 262 KiB**，依赖极轻
5. **PNG 内置**（`PngEncoder`），无需窗口即可视觉验证
6. **API 精准匹配 paint 需求**：`Path` + `Fill` + `Stroke` + `Clip` + `Transform` 正好对应 `RenderCommand::{Rect, Border, Clip}`
7. **无文本渲染**是已知限制，但 text 推迟到 Phase 4 后期，不影响 B-2/B-3
8. **BSD-3-Clause** 与 Apache-2.0 兼容

### 未来可选后端（feature flag）

- **vello**：当 muskitty 进入性能优化阶段且需要 GPU 加速时，可作为 `backend_vello` feature 加入（wgpu 已支持 Windows DirectX）
- **GPUI**：仅当 muskitty 决定整体迁移到 Zed 生态、且 GPUI 发布版支持 Windows 后再考虑

### 不采用 GPUI 的关键原因

1. **crates.io 发布版明确仅 macOS/Linux**（README 原文：*"be on macOS or Linux"*）
2. **Windows 渲染后端代码未发布**（Cargo.toml 中 Windows target 只有 `Win32_Foundation`/`Win32_System_Power`，无 Direct3D）
3. **git 依赖不可行**（拉取整个 zed 仓库 + 未发布的 workspace crate，编译/稳定性灾难）
4. **架构错位**：GPUI 是完整 UI 框架（含状态管理、事件循环、element 树），muskitty 只需渲染层；用 GPUI 等于把浏览器塞进另一个 UI 框架
5. **布局重复**：GPUI 内部已用 taffy 0.12.2 自布局，muskitty-layout 的 LayoutResult 无法直接喂入，需二次转换

---

## 对后续计划的影响

### B-3 调整

原计划 B-3 写「如果 GPUI 不可用，改用 tiny-skia」。**现在确认 GPUI 不可用**，B-3 直接采用 tiny-skia：

```
src/backend/tiny_skia.rs  ← 主后端实现
examples/render_demo.rs   ← 输出 PNG 验证
```

### B-4 端到端 demo

demo 输出改为 **PNG 文件**（不是窗口）：
- 输入：HTML + CSS 字符串
- 流程：parse HTML → parse CSS → cascade → compute → layout → paint → tiny-skia → PNG
- 输出：`examples/output.png`，可用图片查看器打开

### 文本渲染推迟

tiny-skia 明确不做文本。Phase 4 后期需要文本时，方案：
1. 集成 `cosmic-text`（纯 Rust 文本 shaping + 测量）→ 输出 glyph 路径 → tiny-skia 描边/填充
2. 或集成 `swash`（font shaping）+ 自绘 glyph

### 渲染层架构（修订）

```
muskitty-layout 的 LayoutResult (DOM + ComputedStyle + 位置/尺寸)
        │
        ▼  build_render_tree
RenderTree (带样式信息的渲染节点树)
        │  paint
        ▼
RenderCommand[] (Rect/Border/Clip/Transform)
        │  backend (tiny-skia)
        ▼
PngImage / 像素 buffer
```

后端抽象为 `Backend` trait，未来可加 vello/GPUI 后端。

---

## 参考链接

- GPUI on crates.io: https://crates.io/crates/gpui
- GPUI 源码 Cargo.toml: https://docs.rs/crate/gpui/0.2.2/source/Cargo.toml
- GPUI README: https://docs.rs/crate/gpui/0.2.2/source/README.md
- Zed Windows 移植博客: https://blog.hotdry.top/posts/2025/10/16/porting-zed-editor-to-windows-with-rust-and-gpui/
- tiny-skia on crates.io: https://crates.io/crates/tiny-skia
- vello on crates.io: https://crates.io/crates/vello
- taffy 0.12 (muskitty-layout 已用): https://crates.io/crates/taffy
