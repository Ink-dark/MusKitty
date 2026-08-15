# 决策：外部依赖与本体 crate 解耦

> **日期**：2026-08-16
> **状态**：已实施（layout `bf52557` / renderer `98af15d` / network `8cfbdfd`）
> **影响 crate**：`muskitty-layout` / `muskitty-renderer` / `muskitty-network`

## 背景

三个本体 crate 的外部依赖都不同程度泄漏到了公共 API，违背"上层可抽离"的目标：

| crate | 泄漏点 | 后果 |
|-------|--------|------|
| layout | `LayoutTree.taffy/node_map/root` 三个 `pub` 字段（taffy 类型）、`map_style → taffy::Style`、`measure_text` 参数 `FontSystem`（cosmic-text） | 上层接触 taffy/cosmic-text 类型 |
| renderer | `TinySkiaBackend::pixmap()/take_pixmap()` 返回 tiny-skia `Pixmap` | 上层接触 tiny-skia 类型 |
| network | `NetworkError::Http(reqwest::Error)` | `NetworkError` 绑定 reqwest 类型，关闭 `reqwest-backend` feature 时不可用 |

若不处理，未来替换实现（换布局引擎 / 渲染后端 / HTTP 栈）就需要改动上层，违背模块独立与可插拔的目标。

## 决策

**本体 crate 的公共 API（`pub`）只暴露自身定义的抽象类型，外部依赖类型（taffy / tiny-skia / cosmic-text / reqwest）一律不出现在任何 `pub` 导出中。**

机制（沿用 network 已验证的"实现无关数据类型"模式）：

1. **私有化**：外部依赖类型从 `pub` 降到 `pub(crate)` / 私有字段，只作为内部实现。
2. **抽象输出**：公共 API 暴露本体 crate 自己的类型——`LayoutResult` / `NodeLayout` / `LayoutError`（layout）、`RenderOutput::Pixels { width, height, data }`（renderer）、`NetworkError`（network）。
3. **测试隔离**：integration test 需观察内部结构时，用 `#[doc(hidden)] pub` 辅助方法以 `usize`（DOM 地址）为对外 key 隐藏外部依赖类型；依赖私有实现的测试迁入 src 单元测试（`#[cfg(test)]`）。

## 实施

| crate | 处理 | commit |
|-------|------|--------|
| layout | `taffy`/`node_map`/`root` 字段降 `pub(crate)` + `has_root`/`node_count`/`contains_node`/`has_child` 测试辅助；`map_style`/`measure_text`/`resolve_font_size` 降 `pub(crate)`；`convert`/`style_map`/`text`/`tree` 模块降 `pub(crate)`；58 个 style_map 测试迁入 src 单元测试 | `bf52557` |
| renderer | 删 `pixmap()`/`take_pixmap()`，上层改用 `render()` 返回的 `RenderOutput::Pixels`；单元测试改用 RGBA 字节索引 | `98af15d` |
| network | `NetworkError::Http(reqwest::Error)` → `Http(String)`；`From<reqwest::Error>` feature-gate 化并转字符串 | `8cfbdfd` |

## 反决策（明确不做）

- **不引入 `LayoutEngine` trait 抽象**：当前无替换 taffy 的需求，引入 trait 属过早抽象（违反 Simplicity 原则）。私有化已足以让上层可抽离；将来确有第二个布局引擎时再抽象。
- **network 的 trait 抽象保留**：`NetworkFetcher` trait + `reqwest-backend` feature flag 已由 Phase 5 建立，本轮仅收尾 `NetworkError` 的残留泄漏，不重复抽象。

## 影响与验证

- 上层（renderer 之于 layout，未来浏览器外壳之于 renderer/network）只依赖抽象类型，替换实现零改动。
- 可验证：`grep` 各 crate 的 `pub` 导出无外部依赖类型（layout 无 taffy/cosmic-text、renderer 无 Pixmap、network `NetworkError` 无 reqwest）。
- 测试：layout 98 / renderer 68 / network 全绿，`cargo clippy -D warnings` 零警告。

## 后续

- **T-3 换行**（功能性，非解耦）：文本按容器宽度换行需 taffy measure function（`TaffyTree` context 化 + `compute_layout_with_measure`）。
- 该解耦约束已写入 CLAUDE.md / AGENTS.md 的 Hard Rules，未来新增 crate 时同样适用。
