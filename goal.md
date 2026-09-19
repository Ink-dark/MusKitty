# Goal — M-3 batch 3c + 图像管线 + @media 验证 + inline style 收口（2026-09-18）

> **状态**：✅ **已完成**（W-3c / BG-1 / MQ-V / IS-V 全部满足退出条件，见文末"完成记录"）。
> **依据**：[docs/plans/2026-09-12-css-completion.md](docs/plans/2026-09-12-css-completion.md)
> 批次 3 剩余项（3c）、批次 5 前置（background-image）、CS-1 后续验证、层叠闭环。
> 四项由架构师本轮指定，优先级即列出顺序。

## 背景证据（动手前实测）

- `white-space` 已注册（cascade registry.rs:252）但**零消费方**：layout `convert.rs:169`
  直接存原始 `text.data`（源码缩进/换行原样进测量），paint.rs:161 同样吃原始文本；
  纯空白节点一刀切跳过（`pre` 语义下错误）。
- `background-image` **未注册**（`lookup_property` 返回 None → 声明在 filter 阶段被丢）；
  `expand_background`（filter.rs:687）显式丢弃 image 分量；renderer 无任何图像命令；
  `png` crate 已在依赖图（tiny-skia 0.12 → png 0.18.1）。
- `@media` 求值器（cascade filter.rs:1134-1273）：`not` 只取反紧随项（规范：取反整个
  query）；无 `only`；未知 feature 直接 false（规范：Kleene unknown，`not (unknown)`
  必须仍为 false 而非 true）；无运算符相邻项被"以最新为准"吞掉（规范：malformed →
  `not all`）；feature 仅支持 px 单位。sheet 级 media（CS-1d）复用同一求值器。
- inline `style` 属性**已在 cascade 实现**（filter.rs:349-389 `collect_from_style_attr`，
  `from_style_attr` 标志 = §6.1 准则 4，specificity (0,0,0)，shorthand 展开、`--*`、
  important 均通）——本项为**验证收口**：补端到端像素断言 + 修注释错误
  （filter.rs:351 仍写 "(1,0,0,0)"；filter.rs:298 的"条件评估推迟"已过时）。

## 任务与退出条件

| # | 任务 | 退出条件 |
|---|------|---------|
| W-3c | **white-space + 空白折叠**：cascade `text_props` 新增单一来源（`white_space_keyword` + `apply_white_space` Phase I 折叠：space/tab 序列折叠、segment break 按 CJK 规则转空格或删除、`pre*`/`break-spaces` 保留、`pre-line` 保留换行）；layout `InheritedText` + `NodeContext::Text` 扩列（折叠后文本 + wrap 标志）；paint 同一折叠实现；`RenderCommand::Text` 增 wrap 字段；UA 表补 `pre`/`listing`/`plaintext`/`xmp { white-space: pre }` | cascade 单测覆盖 6 关键字 × 折叠矩阵（多空格/tab 合一、\n→空格、CJK 两侧删除、CJK+拉丁转空格、pre 保留、pre-line 保 \n 折空格）；layout 断言：`normal` 下含源码换行缩进的文本与手写单行文本测出相同排版、`nowrap` 单行不换、`pre` 保留换行（多行高）、纯空白节点在 normal 下不占位在 pre 下占位；renderer e2e 像素：normal 折叠前后墨迹一致、pre 多行、nowrap 溢出不换行；测量与绘制同一实现（两侧调 cascade 同一函数）；全量测试绿 |
| BG-1 | **background-image + 图像解码绘制管线**：cascade 注册 `background-image`（initial none）+ `background` 简写展开 image 分量；renderer 新增 `ImageBits`（RGBA+尺寸，ADR 合规）+ `decode_png`；`RenderCommand::Rect` 增 image 载荷；backend 贴图（top-left 起点、repeat 默认平铺、裁剪到盒）；chrome 新增图像子资源采集抓取（CSS `url()` 逐表按其 location 解析、data: 直接解码、复用 scheme 策略与上限模式）→ `PaintInput` 增 base_url + images 表 | cascade 单测：注册/initial/简写展开（`background: url(x) red` → background-color red + background-image url）；renderer 单测：1×1 PNG 解码、命令携带图像、贴图像素落点；chrome e2e：data: URL 背景图全链路像素正确、file:// 相对路径图生效、http 页拒 file:// 图（背景跳过页面照渲）；JPEG 等非 PNG 记录为不支持（解码失败跳过非致命） |
| MQ-V | **@media 求值修正 + 系统性验证**：按 MQ L4 §3 重写求值（Kleene 三值：unknown 经 not 保持 unknown、`false and unknown`=false、`true or unknown`=true；malformed → 整 query `not all`）；`not` 修饰整个 query；`only` 修饰忽略；feature 加 em/rem 单位 + `orientation`；sheet 级 media 与 @media 共用 | 求值矩阵单测全绿（not/only/and/or/逗号/unknown/malformed/单位换算/orientation）；sheet 级 `media="(min-width: …)"`/`not screen`/malformed 各有断言；chrome e2e 补 1 条（窄视口 max-width 表生效或等价）；既有 print/disabled 测试不回归 |
| IS-V | **inline style 收口**：修 filter.rs:351 specificity 注释（(0,0,0)，准则 3 不加权）与 filter.rs:298 过时注释；端到端像素断言补齐（内联胜同特异性样式表、样式表 !important 胜内联普通值、内联 shorthand 展开生效） | renderer end_to_end 3 条新像素测试全绿；既有 cascade 单测（style_attr_*）不回归 |
| Z-1 | **收尾**：文档 + 全量验证 | PROGRESS 行 + 本 goal 完成记录 + css-completion 总账勾掉 3c/background-image；`cargo test --workspace` + cascade/layout 独立仓库全绿；clippy `-D warnings` + fmt 干净；commit 落盘并 push（cascade/layout 提交到各自独立仓库） |

## 显式非目标（本轮不做）

- `white-space` 的 Phase II hang（行尾空格悬挂）、跨节点 IFC 空白合并（每文本节点仍
  独立成盒，节点内折叠 + 节点边界 trim 是本轮模型，偏差记录）
- `break-spaces` 的"每个空格后可断行"（cosmic-text 无此 API，按 pre-wrap 近似，记录）
- `letter-spacing`/`text-indent`/`direction`（batch 3b/后续）
- JPEG/GIF/WebP 解码、`background-repeat/position/size` 属性化（按初始值硬编码：
  repeat 平铺、0 0 起点、natural size）、渐变绘制（`linear-gradient` 记录为跳过，
  render_probe 的开关式断言保持）
- `@media` 的 range 语法（`width >= 600px`）、`prefers-*`、`resolution` 等 feature
- inline style 的 CSSOM `ElementStyle` 与 cascade 的代码级对接（属性字符串是唯一
  真源，双向已一致，无需引 CSSOM 依赖）

## 复跑命令

```bash
cd D:/Muskitty/crates/muskitty-cascade && cargo test
cd D:/Muskitty/crates/muskitty-layout && cargo test
cd D:/Muskitty && cargo test --workspace
cd D:/Muskitty && cargo clippy --workspace --all-targets -- -D warnings
cd D:/Muskitty && cargo fmt --all -- --check
```

## 完成记录（2026-09-18）

| # | 交付 | commit | 验证 |
|---|------|--------|------|
| W-3c | cascade `WhiteSpace` 模型（`collapse`/`preserve_newlines`/`wrap`，§4 表格逐值派生）+ `apply_white_space`（§4.1.2 Phase I：space/tab 序列折叠、§4.1.3 segment break 的 CJK 删除/拉丁转空格、`pre-line` 保换行仍折空格、`pre*`/`break-spaces` 全保留、首尾折叠空白移除） | cascade `031c7cd` | 10 条单测（6 关键字三列矩阵 + 折叠矩阵 + 借用语义）；cascade 247→262 全绿 |
| W-3c | layout 消费：`InheritedText.white_space` + `NodeContext::Text.wrap` + 折叠后文本入叶；measure 在 `wrap=false` 时传 `None`（单行）且缓存键归一化；纯空白节点仅"可折叠"时丢弃（`pre` 保留为内容）；UA 表补 `listing/plaintext/pre/xmp { white-space: pre }` | layout `bc78ec5` / 主仓库 `f248f36` | layout 5 条新测试（折叠与手写单行**等值**、nowrap 单行、pre 保换行、pre-line 保换行且折空格、纯空白节点 pre/normal 对照）；chrome `<pre>` 像素断言；renderer 3 条 e2e（折叠等值、pre 多行块、nowrap 溢出） |
| W-3c | renderer 消费：`paint` 同一 cascade 函数（transform → 折叠同序）、`RenderCommand::Text.wrap`、`draw_text` 按 `wrap` 决定是否传换行宽度 | 主仓库 `f248f36` | 与 layout 测量逐字节同源（两侧调 cascade 单一实现） |
| BG-1 | cascade：注册 `background-image`（initial `none`）+ `background` 简写展开 image 分量（`url()` 两种 token 形态识别、渐变函数透传） | cascade `ea3e4c6` | 6 条单测（注册/initial、无引号 Url、带引号 url()、none/未声明、简写双分量、全局关键字） |
| BG-1 | renderer：新 `image` 模块（`ImageBits` 自有类型 + `from_png`，ADR 合规）+ `RenderCommand::Rect.image` + tiny-skia Repeat pattern 平铺（color 之上、border 之下）+ `PaintInput.images`（绝对 URL 表）+ `no_images()` | 主仓库 `3cb8d00` | 4 条命令级测试 + 3 条 e2e 像素（平铺覆盖盒、三层绘制序、资源缺失页面照常）+ 2 条解码单测（2×2 PNG、非法字节） |
| BG-1 | chrome：新 `images` 模块（按各表 location 解析 `url()`、`absolutize_image_urls` 使声明与表 key 一致、`DocumentFetcher::fetch_bytes` 复用 scheme 策略、上限/失败非致命）+ `render_page_with_images` + `NavigationDoc/WebView/app/文件模式` 全链路携带 | 主仓库 `60d19f4` | 12 条模块测试（采集/去重/at-rule/渐变排除/抓取解码/上限）+ 3 条离线 e2e（**data: 图生效、file:// 相对路径图生效、http 页引用 file:// 图被拒且背景色照常**） |
| MQ-V | cascade：`Tristate`（Kleene 三值，§3 的 negate/and/or 语义 + unknown→false 收敛）；`not` 修饰整条 query、`only` 透明、malformed → `not all` 且**在逗号处恢复**；feature 支持 em/rem（基准 16px）与 `orientation`；sheet 级 media 共用同一求值器 | cascade `5882097` | 15 条新测试（12 条求值矩阵含 `not (unknown)` 必须为 false、`false and unknown`/`true or unknown`、malformed 恢复、em/rem、orientation、嵌套括号、and/or 混用非法；3 条 sheet 级：特性查询、取反与 unknown、malformed 剪枝）+ chrome 2 条 e2e 像素（`media` 属性特性查询、orientation） |
| IS-V | inline `style` **已存在**（`from_style_attr` = §6.1 准则 4，specificity (0,0,0)，简写/`--*`/`!important` 全通）——本轮为收口：修两处陈旧注释（"(1,0,0,0)" 与"条件评估推迟"）+ 文档化 §5.4.5 解析入口与完整排序语义 + 补端到端像素断言 | cascade `5882097` / 主仓库 `a5a929f` | 4 条 e2e 像素（内联胜更高特异性作者规则、作者 `!important` 胜内联普通、内联 `!important` 胜作者 `!important`、内联简写四边框全绘） |
| Z-1 | 文档：本 goal 完成记录 + `css-completion` 总账勾掉 3c 与 background-image + PROGRESS 行 | 主仓库（本轮文档 commit） | 全量复跑见下 |

**全量复跑**：`cargo test --workspace` **300** 全绿（chrome **148**：117 lib + 3 headless + 3 images_e2e + 6 probe + 11 stylesheets_e2e + 8 UA；renderer **126**：53 lib + 27 end_to_end + 46 paint；network **22** + 4 doc）；cascade **262**（111 lib + 52 filter + 79 integration + 19 style_tree + 1 doc）；layout **132**（72 lib + 3 absolute_coords + 10 build_tree + 13 compute + 3 grid + 10 integration + 4 position + 17 text_wrap）；`cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo fmt --all -- --check` 干净（cascade/layout 各自仓库同）。

**实测语义（验收点）**：`white-space` 六值全部生效（`normal` 折叠与手写单行排版**逐行相同**、`nowrap` 单行溢出、`pre` 保强制换行、`pre-line` 保换行且折空格、`pre-wrap`/`break-spaces` 全保留）；UA 表的 `<pre>` 默认等宽 + 保行；`background-image` 从 `url()` 采集→按表基准绝对化→`data:`/`file://` 抓取→PNG 解码→平铺绘制（背景色之上、边框之下）全通，资源缺失/格式不支持非致命；`@media` 三值逻辑（`not (unknown)` 恒 false）、`not`/`only` 修饰符、malformed 在逗号处恢复、em/rem 与 orientation，且 sheet 级 `media` 属性走同一实现；inline `style` 按 §6.1 准则 1→4 排序（`!important` 优先于内联）。

**已知偏差与债务（已记录，不修）**：`white-space` 的 Phase II 行尾空格悬挂与跨节点 IFC 折叠未做（每文本节点独立折叠，节点内为限）；`break-spaces` 的"每空格可断行"按 `pre-wrap` 近似（cosmic-text 无该 API）；`background-repeat/position/size` 按初始值硬编码（三属性未注册）；渐变与 JPEG/GIF/WebP 不支持（解码失败跳过，`linear-gradient` 探针保持"退化到画布"分支）；`@media` range 语法与 `resolution`/`prefers-*` 等特性为 unknown（三值语义下正确处理）；媒体查询 `em` 基准取初始 16px（不随文档字号，符合 MQ L4 §4.1）。