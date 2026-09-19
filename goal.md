# Goal — 高频 CSS 补全第一批（2026-09-19）

> **状态**：✅ 完成（批次 A/B/C + 收尾 Z，退出条件全满足）。
> **依据**：[docs/plans/2026-09-12-css-completion.md](docs/plans/2026-09-12-css-completion.md)
> 三、仍开放缺口表——按网页真实使用频率选定本批：`background-repeat/position/size`
> （批次 5，直接复用 BG-1 图像管线）、`border-radius`（批次 5，当前完全未注册）、
> `opacity` + `visibility`（批次 4，合成与隐藏）。
> 用户点选：border-radius、opacity + visibility、background-repeat/position/size。

## 背景证据（动手前实测）

- `background-repeat`/`background-position`/`background-size` **未注册**
  （cascade registry.rs 只到 `background-image`：L142-151）；`background` 简写在
  filter.rs:702 `expand_background` 只展开 color + image 两个分量，其余跳过；
  renderer command.rs:43-49 记录"三属性按初始值硬编码"（repeat 平铺、起点 0 0、
  natural size），`draw_background_image`（tiny_skia.rs:470）用 pattern Repeat +
  natural size 一次 `fill_rect` 铺满。
- `border-radius` **未注册**；shorthand/token 解析无对应。
- `visibility`（registry L152-157，"visible"，inherited）与 `opacity`
  （L164-169，"1"，非 inherited）**已注册但全 crate 零消费方**。

## 任务与退出条件

| # | 任务 | 退出条件 |
|---|------|---------|
| A | **background-repeat/position/size**：cascade 注册三属性 + `background` 简写展开；renderer `Rect` 增背景参数（repeat/position/size），`draw_background_image` 应用平铺模式/起点偏移/尺寸缩放（auto/百分比/px/cover/contain 子集）；e2e 像素 | cascade 单测：注册/initial/简写展开三分量；renderer 命令级 + e2e 像素：`no-repeat` 单块、`position` 偏移（center）、`size: cover` 铺满 vs 原图；既有 BG-1（repeat 平铺/自然尺寸）不回归 |
| B | **border-radius**：cascade 注册 `border-radius` 简写 + 四角长属性（`border-<corner>-radius`）+ 1-4 值拆分；renderer 圆角路径（背景/边框/背景图按圆角裁剪） | cascade 单测：简写 1-4 值展开、px/百分比、非法值整条丢弃；renderer e2e 像素：圆角矩形的角在外框外无墨迹、中心仍填充；无 radius 时与原矩形像素一致 |
| C | **opacity + visibility**：layout 映射（布局不受影响）；renderer 消费——`visibility: hidden` 跳过自身绘制（后代 visible 覆盖）；`opacity < 1` 子树离屏合成后按 α 混合 | cascade 单测：初始值/继承（visibility 继承、opacity 不继承）；renderer e2e 像素：hidden 自盒无墨迹但布局尺寸不变、后代 visible 仍现；opacity 0.5 的半透明混合（白底上红色 → 粉）；opacity 发散时 1.0 与基线像素相等 |
| Z | 收尾：文档 + 全量验证 | PROGRESS 行 + css-completion 总账勾掉三批 + `cargo test --workspace` 全绿 + cascade/layout 独立仓库全绿 + clippy `-D warnings` + fmt 干净 + 逐仓库 commit 落盘 |

## 显式非目标（本轮不做）

- `background-origin`/`background-clip`/`background-attachment`；`background-size` 的
  `<length-percentage>{2}` 复数（单值 + `auto` 组合）、`background-position` 的 4 值
  corner 语法与百分比 length 混合（初始子集：关键字/px/百分比）
- `border-radius` 的百分比计算（按盒宽高的一半钳制）、椭圆（`x / y`）半径、`currentcolor`
- `opacity` 的层叠上下文隔离（不造 RenderTree，仅离屏合成子树像素）；`visibility: collapse`
- 渐变绘制、box-shadow/text-shadow

## 复跑命令

```bash
cd crates/muskitty-cascade && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check
cd crates/muskitty-layout   && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check
cd /workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
```

## 完成记录

三个 batch 全部完成，退出条件逐一满足：

- **批次 A（background-repeat/position/size）**：cascade 注册三属性 + `background` 简写展开 repeat/position/size 三分量（`58efc45`）；renderer `Rect` 增背景参数 + `draw_background_image` 应用平铺/偏移/尺寸（`415034c`）。退出条件满足：cascade 注册/简写展开单测；renderer 命令级 + e2e 像素（`no-repeat` 单块 / `position: center` 居中 / `size: cover` 铺满）；既存 BG-1（repeat 平铺 / 自然尺寸）不回归。
- **批次 B（border-radius）**：cascade 注册简写 + 四角长属性 + 1–4 值拆分（`fea6b84`）；renderer 圆角路径，背景/边框/背景图按圆角裁剪（`767ecb3`）。退出条件满足：cascade 简写 1–4 值展开 / px / 百分比 / 非法值整条丢弃单测；renderer e2e 像素（角点外框外无墨迹、中心仍填充；无 radius 与原矩形像素一致）。
- **批次 C（opacity + visibility）**：cascade 注册表已含两属性（初始值/继承补测 `7d86720`）；renderer 消费（`90d7be9`）——`visibility: hidden` 借继承语义跳过自身绘绘制保布局，`opacity < 1` 离屏合成后按 α 混合。退出条件满足：cascade 初始值/继承单测（visibility 继承、opacity 不继承）；renderer 命令级 + 后端离屏单测 + **4 条 e2e 像素**（hidden 自盒无墨迹且保布局尺寸 / hidden 父 + 后代 visible 仍现 / opacity 0.5 白底红 → 粉 ~127 / opacity 1 与基线逐字节相等）。

- **批次 Z（收尾）**：文档（PROGRESS 本轮 lead + css-completion 总账勾掉三批 + goal.md 完成记录）已更新；`cargo test --workspace`（renderer 63 单测 + 34 端到端 + 51 paint 命令级）全绿、cascade 独立仓库 20 单测全绿、`clippy --all-targets -- -D warnings` 与 `fmt --all -- --check` 全干净；逐仓库 commit 落盘（cascade `7d86720`，主仓库 `415034c`/`767ecb3`/`90d7be9` + 本轮文档提交）。

> 显式非目标（本轮未做）：`background-origin/clip/attachment`、`background-size` `<length-percentage>{2}` 复数与 corner 语法、`border-radius` 百分比钳制与椭圆/`currentcolor`、`opacity` 层叠上下文隔离（仅离屏合成像素）、`visibility: collapse`、渐变绘制、box-shadow/text-shadow——均留待后续。见 [docs/plans/2026-09-12-css-completion.md](docs/plans/2026-09-12-css-completion.md) 批次总账。