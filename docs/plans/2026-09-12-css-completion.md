# CSS 补全（M-3）总览与批次排期（2026-09-12）

> **用途**：M-3 轨道（"CSS 补全收尾"）的总账——已补什么、还缺什么、按什么顺序补。
> 缺陷判定一律以**实跑证据**为准（注册表 → 消费方 grep、端到端像素测试），
> 不采信文档声明。
> **上一批**：batch 1（border 简写统一展开 + media 视口接线，2026-08-29，PROGRESS 第 15 条）。
> **本批（已完成）**：batch 2（方向性边框 + outline 端到端，2026-09-12，见"批次 2"）
> 与 batch 3（`line-height` 精确解析 + `text-transform`，2026-09-13，见"批次 3"）。

## 一、判定方法

"已注册但无消费方"的扫描方式（可复跑）：

```bash
cd D:/Muskitty
for p in $(grep -o 'name: "[a-z-]*"' crates/muskitty-cascade/src/registry.rs | sed 's/name: "//;s/"//'); do
  lay=$(grep -rn "\"$p\"" crates/muskitty-layout/src/ | wc -l)
  ren=$(grep -rn "\"$p\"" crates/muskitty-renderer/src/ | wc -l)
  [ "$lay" -eq 0 ] && [ "$ren" -eq 0 ] && echo "$p"
done
```

注意该扫描只证明"没有字符串级消费者"，不区分"故意不消费"（如 `overflow` 只在
renderer 用于裁剪判定、`row-gap`/`column-gap` 走 taffy 字段名）与"真缺口"。
逐项定性见下文缺口表。

## 二、本批（batch 2，2026-09-12）已补

| # | 缺口（修复前实测） | 修复 | commit |
|---|------------------|------|--------|
| 1 | `border-top/right/bottom/left` 方向性简写、`border-width/style/color` 的 1–4 值形式：cascade 不展开 → registry 未命中 → **整条声明被丢弃** | cascade：`border` → 12 条方向性长属性；新增 `border-<side>` 简写 → 3 条；`border-width/style/color` 1–4 值拆分（分量类型校验，任一非法整条无效） | cascade `f6c05fa` |
| 2 | `border-<side>-style` 四向**未注册** | registry 补 4 条（initial `none`）；同时删除 `border-width/style/color` 三个**简写**的注册（简写不属长属性表，与 margin/padding 一致） | cascade `f6c05fa` |
| 3 | `thin`/`medium`/`thick` 宽度下游无法解析 → `border-top: solid red` 无边框 | computed value 阶段归一化为 1/3/5px（`normalize_line_width`，与 Chrome/Firefox 对齐、与 `normalize_font_size` 同款单一来源） | cascade `f6c05fa` |
| 4 | layout **完全不映射 border**（`style.border` 从未赋值）→ 边框不占空间、`box-sizing: border-box` 对边框无效（CSS Box Model L3 §2-§3 违背） | `map_style` 填 taffy border rect；§4.1 used width 语义（style none/hidden → 0） | layout `3e1c3a2` |
| 5 | renderer `Border` 单组宽/色/样式 + inset-rect stroke，结构上无法表达四边不同 | `Border` 四边独立（`Option<SideBorder>`）；backend 改为逐边矩形填充（方块拼角，文档化为 miter 近似） | renderer `e48cdff` |
| 6 | `border-style: hidden` 未识别；`double`/`groove`/`ridge`/`inset`/`outset` 解析为 `None` → 静默无边框 | `parse_border_style` 覆盖 §4.2 全集；`hidden` 等同不绘制；其余样式按 solid 近似绘制（可见占位优于静默丢弃） | renderer `e48cdff` |
| 7 | `outline-width/style/color` 已注册、**全 crate 零消费方** | `outline` 简写展开（cascade）+ `RenderCommand::Outline`（paint 在子节点之后发出，绘制在 border box 外围、后代之上）+ backend 四条外扩矩形条；`outline-style: auto` → solid 近似、`outline-color: auto` → 当前文字色 | cascade `f6c05fa` / renderer `e48cdff` |
| 8 | `currentcolor` 边框色被硬编码回退成黑色 | 逐边/轮廓色经 `resolve_color(&cv, current_color)` 用元素文字色解析 | renderer `e48cdff` |

验证：cascade 89 lib + 31 filter + 73 integration + 16 style_tree；layout 72 lib +
13 compute（新增 4 项盒模型端到端）；renderer 50 lib + 37 paint + 15 end_to_end
（新增 3 项全链路像素：仅左边框、border 撑大盒子、outline 在盒外 3px）；
chrome 85 + 3 + 6 全绿；fmt/clippy（`-D warnings`）干净。

## 二之二、本批（batch 3，2026-09-13）已补

| # | 缺口（修复前实测） | 修复 | commit |
|---|------------------|------|--------|
| 1 | `line-height` 是 `font_size * 1.2` 硬编码（T-3 近似）——`line-height: 40px` / 倍数 / 百分比对换行与盒高完全无效 | cascade 新增 `text_props::used_line_height_px`（px/数/百分比/normal/非法值语义 + 非有限钳制）；`line-height: <percentage>` 在计算值阶段按自身 font-size 转 px，数保持倍数形态随继承 | cascade `b66d0e4` |
| 2 | 测量与绘制各自算行高（两处 `font_size * 1.2`），**必须逐字节一致**否则行位置与盒高错位 | 两侧都调用 cascade 的同一函数：layout `measure_text` 接 `line_height` 参数（`NodeContext::Text` 新字段），renderer `RenderCommand::Text` 新字段 `line_height`，backend 不再自算 | layout `1730e94` / renderer `bdd1dae` |
| 3 | `text-transform` 零消费方（大写/小写/首字大写完全不生效） | `cascade::apply_text_transform`（Unicode 全尺寸映射、空白切词的 capitalize 近似）；layout 在**建树时**改写文本叶内容（测量即转换后文本），paint 在生成 Text 命令时改写内容——两侧同一实现 | cascade `b66d0e4` / layout `1730e94` / renderer `bdd1dae` |
| 4 | 继承文本上下文参数持续膨胀（T-3 加三个、batch 3 再加两个，撞 clippy 8 参数上限） | layout 收敛为 `InheritedText` 结构体（font-size/family/weight + line-height/text-transform），一次扩列不再改签名 | layout `1730e94` |

验证：cascade 101 lib + 31 filter + 73 integration + 19 style_tree + 1 doctest；
layout 72 lib + 12 text_wrap（新增 6 项测量用例，含"转换后文本与字面量大写测出完全相同
排版"的等式断言）；renderer 51 lib + 42 paint（新增 5 项命令级）+ 17 end_to_end
（新增 2 项全链路像素：60px 行高把末行墨迹下推 >40px 且总墨迹差 <10%；uppercase
对同串产生不同墨迹——不用字体相关的量值比较）；chrome 85 + 3 + 6 全绿；
fmt/clippy（`-D warnings`）干净。

**验证口径纪律（本轮踩到的坑）**：`text-transform` 的所有断言都用**等值**（转换后
文本 vs 字面量大写测出相同排版）或**不等**（墨迹不同），不用"大写更宽/更高"这类
字体相关量值比较——首版这么写就因 `l` 的 ascender 高于大写而误报。同理行高的像素
断言要求画布足够高，否则高行高的末行被裁掉会让墨迹计数失真。

## 三、仍开放的 CSS 缺口（按建议批次排序）

### 批次 3（2026-09-13 部分完成：line-height + text-transform ✅）

| 缺口 | 现状 | 规范 |
|------|------|------|
| ~~`line-height` 精确解析~~ ✅ **已完成**（batch 3）：`normal`/`<number>`/`<length>`/`<percentage>` 全支持；语义单一来源 `cascade::text_props::used_line_height_px`（layout 测量与 renderer 绘制共用），百分比在计算值阶段转 px、数保持倍数形态随继承（子元素按自身 font-size 折算） | 旧为 `font_size * 1.2` 硬编码近似 | CSS Inline L3 §4.2 |
| ~~`text-transform`~~ ✅ **已完成**（batch 3）：`none`/`uppercase`/`lowercase`/`capitalize`；`cascade::apply_text_transform` 在 layout 测量与 paint 内容两侧共用（布局前生效），Unicode 全尺寸映射（`ß`→`SS`） | 旧为零消费方 | CSS Text L3 §2.1 |
| `letter-spacing` / `word-spacing` | 注册但无消费方；**cosmic-text 0.13.2 无此 API**（`Attrs` 仅 family/stretch/style/weight，Buffer 仅 `set_monospace_width`/`set_tab_width`）→ 需在测量与字形定位两处自建 advance 契约（batch 3b） | CSS Text L3 §7 |
| `font-style: italic` | 注册但无消费方；管线侧只是 `Attrs::style` 一个字段，但**系统字体是否有 italic 面**决定像素结果（cosmic-text 不合成斜体）→ 需先定验证口径（batch 3b） | CSS Fonts L4 §2.3 |
| `white-space`（nowrap/pre/…） | 注册但无消费方；含空白折叠语义（当前测量直接吃原始文本，折叠整体缺失）→ batch 3c | CSS Text L3 §4 |
| `text-indent` | 注册但无消费方（首行缩进需按行测量，非当前单次测量结构） | CSS Text L3 §3 |
| `direction` | 注册但无消费方（RTL 基方向） | CSS Writing Modes L4 §2.3 |
| `tab-size` / `orphans` / `widows` | 注册但无消费方（低频，排 batch 3c 之后） | CSS Text L3 §3.3 / §5 |

### 批次 4（合成与层叠）

| 缺口 | 说明 |
|------|------|
| `opacity` | 需子树离屏合成（渲染到临时 Pixmap 后按 α 混合）；当前注册但零消费 |
| `z-index` + 层叠上下文 | paint 现为 DOM 先序；z-index 需建立层叠上下文与排序（`RenderTree` 中间结构曾因无消费者移除，届时重生） |
| `visibility: hidden` | 需在 paint 跳过自身绘制但保留布局空间，且允许后代 `visibility: visible` 覆盖（继承语义） |

### 批次 5（盒装饰余项）

| 缺口 | 说明 |
|------|------|
| `border-radius` | **未注册**；需路径圆角（tiny-skia 支持路径，但四角半径需裁剪/描边几何） |
| `outline-offset` | 未注册；本轮 outline 固定 offset 0 |
| corner miter 斜接 | 相邻边不同宽时浏览器用梯形斜接，当前方块拼接（已文档化近似） |
| dashed / dotted / double / 明暗类真实绘制 | 当前全部按 solid 近似；需 dash 模式与多线/明暗合成 |
| `background-image`（含 gradient） | renderer 无图像解码/绘制管线；`linear-gradient` 探针已确认为退化白底（chrome 回归测试留有开关式断言） |
| `box-shadow` / `text-shadow` | 未注册；需模糊核 |

### 批次 6（布局消费方缺口）

| 缺口 | 现状 |
|------|------|
| `order` | 注册但无消费方（taffy 支持 `order`，需 map_style 接线） |
| `justify-items` / `justify-self` | 注册但无消费方（grid 对齐） |
| `grid-auto-flow` / `grid-auto-columns` / `grid-auto-rows` | 注册但无消费方（隐式轨道） |
| `cursor` | 注册但无消费方（需 chrome 层 CSS 命中测试 + winit 光标设置） |
| `gap`（简写） | 展开为 row/column-gap 已就绪，但扫描显示无字面消费（实际经 taffy `gap` 字段生效——**判定为假缺口**，保留在此仅为对照） |

### 不排期（明确不做，理由记录）

| 项 | 理由 |
|----|------|
| `revert` / `revert-layer` 真语义 | 需低 origin/层回滚；零真实页面需求（当前按 unset 处理，见 cascade defaulting） |
| `@layer` 排序 | 已完整（audit B8），无需再做 |
| `@scope` / Shadow DOM 层叠准则（§6.1 准则 2/3） | 依赖未实现的 shadow/scope 模型 |

## 四、退出条件与复跑

- 每批的退出条件写进当轮 `goal.md`，并满足：相关 crate 全测试绿 + fmt/clippy 干净 +
  **端到端像素或值级断言**（不接受仅"命令生成"级断言）。
- 复跑本轮验证：

```bash
cd D:/Muskitty/crates/muskitty-cascade && cargo test          # 225 测试
cd D:/Muskitty/crates/muskitty-layout  && cargo test          # 121 测试
cd D:/Muskitty/crates/muskitty-renderer && cargo test         # 110 测试
cd D:/Muskitty && cargo test -p muskitty-chrome               # 94 测试（含导航/渲染）
```

> 环境注记：2026-09-12/13 期间本机 stable toolchain 被卡住的 `rustup update stable`
> 中断（`rustc.exe` 缺失），batches 2/3 的实现与验证用 `cargo +1.85.0` 完成
> （chrome/network 需 `--ignore-rust-version`，network 的 dev-dep wiremock 0.6.5
> 需 rustc ≥1.88 故当时编不过）。**2026-09-13 该 update 自行完成（stable = 1.98.1）**，
> 全部工作在默认工具链上复跑通过：`cargo test --workspace` 218、cascade 225、
> layout 127、`clippy --workspace --all-targets -- -D warnings` 与 `fmt --all --check`
> 干净（新版 clippy 的两处既有告警已在 `9522d32` 修掉）。
