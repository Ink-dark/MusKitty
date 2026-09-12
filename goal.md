# Goal — M-3 batch 2：方向性边框 + outline 端到端补全（2026-09-12）

> **更新时间**：2026-09-12
> **状态**：✅ **已完成**。B-1~B-5 全部满足退出条件（记录见文末"完成记录"）。
> **轨道**：M-3（CSS 补全）第二批。上一批（batch 1）见 PROGRESS.md 第 15 条
> （border 简写 1 组 + media 视口接线，2026-08-29）。总账（剩余缺口与批次排期）
> 见 [docs/plans/2026-09-12-css-completion.md](docs/plans/2026-09-12-css-completion.md)。
> **依据**：PROGRESS.md "M-3 余项" 明列的四项——`@layer` 排序（已完整，无需再做）、
> background-image（renderer 无 image 消费方）、revert 真语义（需低 origin/层回滚，
> 零真实页面需求）、**方向性 border**、**outline**。本轮取后两项：它们有现成消费方
> （paint/layout），是真实页面高频用法，且当前是**静默错误**而非"未实现"——
> `border-left: 2px solid red` 与 `border: 1px solid red` 渲染结果相同（四边等宽），
> `outline-*` 注册了但零消费方。

## 背景：已确认的缺陷（动手前实测）

| # | 现状 | 影响 |
|---|------|------|
| 1 | cascade `border` 简写只展开为 `border-width`/`border-style`/`border-color` 三个**统一**长属性（filter.rs `expand_border`）；`border-top: …` 等方向性简写、`border-width: 1px 2px 3px 4px` 多值形式**完全不展开**（registry 未命中 → 整条声明丢弃） | `border-left`/`border-bottom` 等真实页面高频写法**整条声明被丢弃**；四边多值同理 |
| 2 | 方向性长属性 `border-*-width`/`-color` 已注册（`-style` 四向**未注册**），但 renderer `extract_border` 只读统一三属性 | 即使写出方向性长属性也不生效 |
| 3 | layout `style_map.rs` **完全不映射 border**（`.border` 字段从未赋值，grep 零命中） | 盒模型残缺：`border: 10px solid` 不占空间，`box-sizing: border-box` 对边框无效——CSS Box Model L3 §2-§3 违背 |
| 4 | renderer `draw_border` 用 inset rect **stroke**，`Border { width, color, style }` 单组值 | 结构上无法表达四边不同宽/色/样式 |
| 5 | `outline-width/style/color` 已注册，repo 内**零消费方**（全 crate grep 为 0） | `outline: 2px solid red` 静默丢弃 |
| 6 | `border-style` 支持 `none/solid/dashed/dotted`；`hidden` 未识别；`double`/`groove`/`ridge`/`inset`/`outset` 在 renderer 解析为 `None` | `border: 5px double red` → 无边框（比"近似绘制"更差） |
| 7 | `thin`/`medium`/`thick` 宽度在 renderer 解析失败 → 无边框 | `border-top: solid red`（省略宽度）本应 3px 实线，实际无边框 |

## 规范依据

- CSS Backgrounds & Borders Level 3 §4.1（border-width 计算/used 值：style 为
  `none`/`hidden` 时 used width = 0）、§4.2（border-style 关键字全集）、§4.3
  （`<line-width>`：`thin`/`medium`/`thick` UA 相关，取 1px/3px/5px 与
  Chrome/Firefox 对齐）、§4.4（`border` 与 `border-<side>` 简写、
  `<line-width> || <line-style> || <color>`）
- CSS Box Model Level 3 §2-§3：content/padding/border 盒模型，border 参与
  box-sizing 计算
- CSS UI Level 4 §4：`outline` 简写与 `outline-<width|style|color>` 长属性；
  outline **不参与布局**（绘制在 border box 之外，不改变元素尺寸）
- CSS Cascade Level 5 §3.2：简写中的 CSS-wide 关键字分配到所有长属性

## 任务与退出条件

| # | 任务 | 退出条件 |
|---|------|---------|
| B-1 | **cascade**（独立仓库）：注册 `border-<side>-style` 四向长属性；`border` 简写展开为 **12 条**方向性长属性；新增 `border-<side>` 四向简写展开（各 3 条）；`border-width`/`border-style`/`border-color` 按 1–4 值展开为四向长属性（分量类型校验，任一非法则整条无效）；新增 `outline` 简写 → 3 条长属性；`thin`/`medium`/`thick` 宽度在 computed value 阶段归一化为 px（1/3/5，同 `normalize_font_size` 做法，单一来源）；删除 `border-width`/`border-style`/`border-color` 三个**简写**的 registry 条目（简写不属长属性注册表，与 margin/padding 一致） | `cargo test` 全绿（含新增简写/多值/方向性用例）；`cargo fmt --check` + `clippy -D warnings` 干净；简写展开后 `border-width` 等统一属性**不再存在**（断言） |
| B-2 | **layout**（独立仓库）：`map_style` 把四向 `border-<side>-width` 映射到 taffy `Style.border`；`border-<side>-style` 为 `none`/`hidden` 时 used width 取 0（§4.1） | 新增盒模型测试：`border: 10px solid` + `width: 100px`（content-box）→ border box 120px；`box-sizing: border-box` → 100px 含 10px 边框；单边边框（`border-bottom`）只增对应边；`border-style: none` 不占空间；全绿 + fmt/clippy 干净 |
| B-3 | **renderer**（主仓库）：`extract_border` 改为逐边提取（`Border` → 四向 per-side width/color/style）；`border-style: hidden` 等同 `none`（不绘制）；`double`/`groove`/`ridge`/`inset`/`outset` 按 solid 近似（显式文档化）；backend 由 inset-stroke 改为**四边矩形填充**（corners 采用 top/bottom 全长、left/right 纵向内缩的方块拼接，非 miter 斜接——文档化近似） | 端到端像素测试：四边各自颜色/宽度分别断言；`border-left: 4px solid` 仅左边着色；`border-bottom` 简写生效；`border: 1px solid` 四边一致；`border-style: none` 无边框；全绿 + fmt/clippy 干净 |
| B-4 | **renderer**：`outline-*` 绘制——新增 `RenderCommand::Outline`，在**子节点之后**发出（outline 绘制在 border box 外侧、后代之上）；`outline-color: auto` → currentColor；`outline-style: none`/宽度 0 → 不发指令 | 像素测试：outline 在 border box 外（元素外 2px 处着色、元素内不受影响）；`outline: 2px solid` 简写端到端；不影响布局尺寸（layout 结果与无 outline 时逐字段相等） |
| B-5 | **文档/记录**：`docs/plans/2026-09-12-css-completion.md`（M-3 全量缺口表 + 本轮批次 + 后续批次排期）、PROGRESS.md 第 15 条追加 batch 2 记录、goal.md 收尾 | 文档与实跑一致；各仓库分别 commit（cascade/layout 独立仓库，renderer 主仓库） |

## 显式非目标（本轮不做，写入缺口表排后续）

- `background-image`（需 renderer 图像解码/绘制管线，无消费方）
- `revert`/`revert-layer` 真语义（需 origin/层回滚）
- 文本属性缺口：`line-height` 精确解析（当前 `font_size * 1.2` 近似）、
  `font-style`（italic）、`letter-spacing`/`word-spacing`/`text-transform`/
  `text-indent`/`white-space`/`tab-size`（均已注册、零消费方）
- `opacity`（需子树离屏合成）、`z-index`（需层叠上下文）、`visibility`
- `border-radius`（未注册，需路径圆角）
- `outline-offset`（未注册，本轮 outline 固定 offset 0）
- 布局未消费：`order`/`justify-items`/`justify-self`/`grid-auto-*`
- border corner miter 斜接（当前方块拼接近似）

## 风险与既定裁决

- **API 破坏**：renderer 公共 `Border` 改为四向结构 + `RenderCommand` 加变体
  （枚举已 `#[non_exhaustive]`），chrome 不构造 `Border`（已 grep 确认），
  影响面仅 renderer 自身 + 测试。
- **渲染结果变化**：布局开始计入 border 宽度后，带边框的既有页面尺寸变大
  （正确行为）。demo（`chrome/src/main.rs`）与既有测试若断言旧尺寸，按新
  语义更新断言并在 commit message 记录。
- **cascade 属主**：cascade/layout 是独立仓库（muskitty-dev/*），动手前已
  `git fetch`——本地与 `origin/main` 同步（0/0），无架构师并行撞车。

## 完成记录（2026-09-12）

| # | 交付 | commit | 验证 |
|---|------|--------|------|
| B-1 | cascade：12 条方向性长属性展开、`border-<side>` 简写、`border-width/style/color` 1–4 值、`outline` 简写、`thin/medium/thick`→px 归一化、`border-<side>-style` 注册 + 删除三个简写注册 | muskitty-cascade `f6c05fa`（已推送） | 89 lib + 31 filter + 73 integration + 16 style_tree + 1 doctest 全绿；fmt/clippy 干净 |
| B-2 | layout：taffy border rect + §4.1 used width（none/hidden → 0） | muskitty-layout `3e1c3a2`（已推送） | 72 lib + 13 compute（新增 4 项盒模型）全绿；fmt/clippy 干净 |
| B-3 | renderer：四边独立 `Border`/`SideBorder`、逐边提取与矩形填充、§4.2 全集样式、`currentcolor` 解析 | 主仓库 `e48cdff` | 50 lib + 37 paint + 15 end_to_end 全绿 |
| B-4 | renderer：`RenderCommand::Outline` + paint 子节点后发出 + backend 盒外四条矩形条 | 同上 `e48cdff` | 含像素测试（盒外 3px）与"不影响布局"断言 |
| B-5 | 文档：本 goal + PROGRESS 第 15b 条与总览行 + `docs/plans/2026-09-12-css-completion.md`（缺口总账与批次 3–6 排期） | 主仓库文档 commit | 与实跑一致 |

**验证口径**：新增断言均为**端到端像素级**（cascade 值级 → layout 几何级 → renderer 像素级），
不接受仅"命令生成"级断言。全链路像素测试三项：仅左边框只染左侧 6px；`border: 5px`
使 border box 从 20px 增至 30px；`outline: 3px` 落在 10px margin 盒外。

**本机环境注记（影响复跑，非本轮改动）**：stable toolchain 的 `rustup update stable`
（PID 8556，22:38 启动）中断，`rustc.exe` 缺失 → 本轮全部构建用 `cargo +1.85.0`；
chrome/network 依赖 rust-version ≥1.86 的 icu 包需 `--ignore-rust-version`；
network 的 dev-dep wiremock 0.6.5 需 rustc ≥1.88（let-chains），故 network 测试
本机暂无法编译（该 crate 本轮未改动）。toolchain 修复后应按默认 stable 复跑一次。

**Mimosa 交互记录**：layout commit 曾被 medium 级发现拦截，命中的是
`crates/muskitty-layout/target/doc/static.files/search-*.js`（rustdoc 生成物，
Aug 2 的陈旧产物）——已按既有结论判定为误报，并用 `cargo clean --doc` 删除该
生成物（可 `cargo doc` 重新生成），随后 commit 放行。其余 commit/push 均为
"未取得完整扫描结论"的兼容放行警告（不宣称项目安全）。
