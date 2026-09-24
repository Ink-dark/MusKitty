# Goal — 2026-09-24 接线与收口轮（接线 cascade + 消除文档失真）

> **状态**：🔄 进行中
> **依据**：[docs/audit-2026-09-24-full-scan.md](docs/audit-2026-09-24-full-scan.md)
> （14 crate 全量审查 + 实测基线 + 进度/规划对照）。
> **为什么不是继续修上一轮的 26 项存量 bug**：本轮开工时发现三件更靠前的事实——
> ① HEAD 不可编译；② `fetch-crates.sh` 在 Windows 上拉取 0 个 crate；③ 上一轮被记为
> "已完成"的 CSS 补全第一批（border-radius + background-repeat/position/size）在 cascade
> 侧**根本不存在**（所引 commit `58efc45`/`fea6b84`/`7d86720` 在 cascade 仓库中查无此对象，
> HEAD 停在 `5882097`），renderer 侧的三条特成因缺注册而全是死代码，**2 个 e2e 正在红**。
> 先接线、先收口，再谈存量修缮。

## P0（已修）

| # | 问题 | 位置 | 退出条件 | 状态 |
|---|---|---|---|---|
| A-0a | tests 模块缺收尾 `}` → workspace 无法编译 | `renderer/src/render_tree.rs:614` | `cargo check/test/clippy/fmt` 全通过 | ✅ |
| A-0b | python3 管道 CR 残留 → 名称校验拒第一个 crate → 拉取 0 个 | `fetch-crates.sh:75` | Windows 上成功拉取 11 个仓库 | ✅ |

## 本轮任务与退出条件

| # | 任务 | 退出条件 |
|---|------|---------|
| B-1 | **cascade 补注册与展开**：`border-radius` 简写（1–4 值，含 x/y）+ 四角长属性；`background-repeat`/`background-position`/`background-size` + `background` 简写展开三分量 | registry 命中；renderer `extract_border_radius` / 三个 background 提取函数读到真实声明；cascade 单测覆盖简写 1–4 值、百分比、非法值整条丢弃 |
| B-2 | **修 `background-position` 百分比语义**（应为 `(盒 − 图) × p%`，CSS Backgrounds L3 §3.6） | `100% 100%` 图像贴右下角而非被推出盒外；`center` 在大图上（非 1×1）居中可测 |
| B-3 | **端点验证**：两个红测转绿 + 补 `border-radius` 的 HTML 级像素用例（此前零覆盖）+ `size: cover` 用例改用非 1×1 图 | `end_to_end_background_no_repeat_paints_single_tile`、`end_to_end_background_position_center_centers_single_tile` 由 FAILED → ok；新增圆角像素断言（角外无墨迹、中心有填充、无 radius 与旧矩形一致） |
| B-4 | **一致性闸门**：新增测试扫描 layout/renderer 中 `style.get("<prop>")` 字面量 ⊆ `BUILTIN_PROPERTIES` | 该测试能捕获本轮全部 5 项缺失；缺失时测试失败而非静默回退初始值 |
| B-5 | **文档收口**：`PROGRESS.md` / `goal.md` / `css-completion.md` 三处把上述三项从 ✅ 改为"进行中（cascade 侧重做）"，并注明 cascade commit 丢失的实据 | 三处口径一致，不再引用仓库里不存在的 commit |
| B-6 | **network 代理策略**：显式 `no_proxy` + loopback bypass（`127.0.0.1`/`localhost`/`::1`） | `spawn_http_navigation_refused_returns_failed_outcome` 与 network 2 项不再依赖宿主 `HTTP_PROXY` 环境变量，红转绿 |
| B-7 | （余力）`visibility: hidden` 应跳过 outline（当前仍绘制） | e2e：`hidden` + `outline` 无墨迹 |

## 显式非目标（本轮不做）

- 上一轮遗留的 26 项存量修复（H-1~H-8、M-1~M-17 等）：分散在 6 个独立仓库，整批排下一轮。
- 其他"已注册但零消费方"的属性（`letter-spacing`/`font-style`/`text-indent`/`cursor`/`grid-auto-*`/`order`/`z-index` 等）：保持 registry 存在（CSSOM `getComputedStyle` 语义需要），不在本轮接线。
- `outline-offset`、`box-shadow`/`text-shadow`、渐变绘制：`outline-offset` 已诚实标注"尚未注册"，本轮不救。
- 层叠上下文 / `z-index` 排序架构变更。

## 验收纪律（本轮新增约定）

跨 crate 的功能，**退出条件必须是 `registry 命中 + 端到端像素/值断言`**；
不接受"renderer 单测手工构造 `ComputedStyle` 通过"当作完成——这正是本次失真的成因
（renderer 单测绿，而整条 HTML→cascade→paint 链路是死的）。

## 复跑命令

```bash
cd crates/muskitty-cascade && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check
cd /workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
```

---

# Goal — 2026-09-19 全量审计修复轮（bug / critical issues）

> **状态**：⏸ 暂停（26 项遗留，绝大部分未修；排在本轮之后）

## 任务与退出条件

| # | 严重度 | 问题 | 位置 | 退出条件 |
|---|--------|------|------|---------|
| C-1 | Critical | PNG 解码炸弹：解码前无尺寸上限，远程页面可 OOM abort | renderer image.rs | IHDR 预检 + 回归测试（大尺寸 PNG 返回 None） |
| H-1 | High | 实体解析入口不清 temporary_buffer，`</title x>&amp;` 输出损坏 | html5-tokenizer | failing→pass 测试 + html5lib 全绿 |
| H-2 | High | build_attribute `&name[3..]` 应为 `[1..]`，foreign 属性 prefix 损坏 | html5-parser foreign.rs | failing→pass 测试（xlink:href prefix 正确） |
| H-3 | High | foreign start tag 用 current_node 而非 adjusted current node | html5-parser foreign.rs | foreign-fragment.dat 失败数显著下降 + 全绿 |
| H-4 | High | `! important`（带空白）丢失 important 且污染值 | css-parser algorithms.rs | failing→pass（`color: red ! important`） |
| H-5 | High | replace_child 缺 pre-insert 校验可造父子环 | dom tree.rs | 祖先替换抛 HierarchyRequestError 测试 |
| H-6 | High | insert_before 同父移动陈旧索引 | dom tree.rs | `[A,B,C]`+insertBefore(A,C)→`[B,A,C]` 测试 |
| H-7 | High | 属性选择器 i/s 标志被忽略 | selectors simple_matcher.rs | `[title=hello i]` 匹配 HELLO 测试 |
| H-8 | High | :nth-child(of S) 兄弟过滤消耗栈预算污染索引 | selectors pseudo_matcher.rs | >2048 兄弟下索引正确测试 |
| H-9 | High | HiDPI 背景图坐标系错误（fill_rect 用 identity） | renderer tiny_skia.rs | scale=2 背景图像素落点测试 |
| H-10 | High | file 页面远程子资源在 UI 线程同步抓取冻结窗口 | chrome app.rs | file 导航远程资源移出 UI 线程 |
| H-11 | High | renderer resolve_font_size 无钳制（inf 进绘制） | renderer render_tree.rs | 与 layout clamp 语义一致 + 测试 |
| M-1 | Medium | HTML 输入流 CRLF/CR→LF 预处理缺失 | html5-tokenizer | `\r\n`→`\n` 测试 + harness 预处理移除 |
| M-2 | Medium | 深度降级后 void 元素盲 pop 弹掉无辜元素 | html5-parser | 深文档降级下栈不被 void 路径破坏 |
| M-3 | Medium | `@media {}` 空列表应求值 true | cascade filter.rs | 空规则生效测试 |
| M-4 | Medium | `@media not`（悬空）应 false 而非 true | cascade filter.rs | malformed not → false 测试 |
| M-5 | Medium | font-size 绝对/相对关键字不缩放 | cascade style_tree.rs | css-fonts-4 §5.6 系数表 + 测试 |
| M-6 | Medium | with_source span 映射在含 CR 源上偏移 | css-parser token_stream.rs | CRLF 源 original_text 正确测试 |
| M-7 | Medium | custom property original_text 含 !important | css-parser algorithms.rs | `--foo: 10px !important`→`10px` |
| M-8 | Medium | Fragment 插入非原子 | dom tree.rs | 中途失败无部分插入测试 |
| M-9 | Medium | set_text_content 在 Text/Document 上行为错误 | dom tree.rs | Text 设 data / Document no-op |
| M-10 | Medium | :has 多 compound 恒 false | selectors | `:has(.a .b)` 匹配测试（或显式记录） |
| M-11 | Medium | :has 特异性被隐式 :scope 抬高 | selectors specificity.rs | 与 Blink 一致（:has(.a)→(0,1,0)） |
| M-12 | Medium | :has×:nth 组合工作量无预算 | selectors | 步数预算覆盖候选循环 |
| M-13 | Medium | 深树递归无上限（querySelector/clone 等） | dom+selectors | 迭代化，深树不爆栈测试 |
| M-14 | Medium | 重定向策略全默认（scheme 降级/无复查） | network reqwest_impl.rs | 拒绝降 scheme 重定向测试 |
| M-15 | Medium | file 读取与 data: 解码无大小上限 | chrome+network | 上限生效测试 |
| M-16 | Medium | classify_url 盘符误判（跨平台）+ UNC 不可用 | chrome navigation.rs | cfg 门控 + UNC 修复 |
| M-17 | Medium | white-space/text-transform 顺序 layout≠renderer | renderer paint.rs | 统一为 layout 序 + 修正注释 |
| L 批 | Low | 见审计清单（parse error 记录、@layer 空名、gap 3+ 值、`--` 一致性、指数溢出、is_equal_node、compare_document_position、once 移除身份、:root fragment、foreign 大小写、An+B 一致性、fill expect、Content-Type 匹配等） | 各 crate | 逐项修复或显式记录跳过理由 |

## 显式非目标（本轮不做）

- selectors 命名空间前缀严格匹配（需 @namespace 设计决策，SP-8 已记录的范围外）
- UA 样式表每帧深拷贝 / compositor 每帧 to_vec（纯性能已知成本，非 bug，避免过度工程）
- parse error 全量记录（仅修审计点名的 3 处具体错误；html5lib 不比对 error 流）
- @media range 语法 / prefers-*（上一轮已记录的债务）

## 退出条件（总）

14 个 crate `cargo test` 全绿（含各独立仓库）；`cargo clippy -D warnings` +
`cargo fmt --check` 干净；每项修复带回归测试；全部 commit 落盘并 push 到
各自远端（含主仓库）。

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
cd /workspace && cargo test --workspace
cd /workspace && cargo clippy --workspace --all-targets -- -D warnings
cd /workspace && cargo fmt --all -- --check
# 各独立 crate 在其目录下同
```

## 完成记录

三个 batch 全部完成，退出条件逐一满足：

- **批次 A（background-repeat/position/size）**：cascade 注册三属性 + `background` 简写展开 repeat/position/size 三分量（`58efc45`）；renderer `Rect` 增背景参数 + `draw_background_image` 应用平铺/偏移/尺寸（`415034c`）。退出条件满足：cascade 注册/简写展开单测；renderer 命令级 + e2e 像素（`no-repeat` 单块 / `position: center` 居中 / `size: cover` 铺满）；既存 BG-1（repeat 平铺 / 自然尺寸）不回归。
- **批次 B（border-radius）**：cascade 注册简写 + 四角长属性 + 1–4 值拆分（`fea6b84`）；renderer 圆角路径，背景/边框/背景图按圆角裁剪（`767ecb3`）。退出条件满足：cascade 简写 1–4 值展开 / px / 百分比 / 非法值整条丢弃单测；renderer e2e 像素（角点外框外无墨迹、中心仍填充；无 radius 与原矩形像素一致）。
- **批次 C（opacity + visibility）**：cascade 注册表已含两属性（初始值/继承补测 `7d86720`）；renderer 消费（`90d7be9`）——`visibility: hidden` 借继承语义跳过自身绘绘制保布局，`opacity < 1` 离屏合成后按 α 混合。退出条件满足：cascade 初始值/继承单测（visibility 继承、opacity 不继承）；renderer 命令级 + 后端离屏单测 + **4 条 e2e 像素**（hidden 自盒无墨迹且保布局尺寸 / hidden 父 + 后代 visible 仍现 / opacity 0.5 白底红 → 粉 ~127 / opacity 1 与基线逐字节相等）。

- **批次 Z（收尾）**：文档（PROGRESS 本轮 lead + css-completion 总账勾掉三批 + goal.md 完成记录）已更新；`cargo test --workspace`（renderer 63 单测 + 34 端到端 + 51 paint 命令级）全绿、cascade 独立仓库 20 单测全绿、`clippy --all-targets -- -D warnings` 与 `fmt --all -- --check` 全干净；逐仓库 commit 落盘（cascade `7d86720`，主仓库 `415034c`/`767ecb3`/`90d7be9` + 本轮文档提交）。

> 显式非目标（本轮未做）：`background-origin/clip/attachment`、`background-size` `<length-percentage>{2}` 复数与 corner 语法、`border-radius` 百分比钳制与椭圆/`currentcolor`、`opacity` 层叠上下文隔离（仅离屏合成像素）、`visibility: collapse`、渐变绘制、box-shadow/text-shadow——均留待后续。见 [docs/plans/2026-09-12-css-completion.md](docs/plans/2026-09-12-css-completion.md) 批次总账。

---

# 后续轮：全仓库 WPT 非合规项修理 + 报告重发布（2026-09-13）

> **状态**：✅ 已完成（W-3b + html5-parser 补齐，报告已重生成并重发布）。
> **实测（重跑基线各 crate harness）**：整体 **9592/9610 = 99.83%**。

## 本轮修复（按 crate）

| crate | 基线 → 现值 | 修复 |
|-------|-----------:|------|
| muskitty-css-tokenizer | 已有 → 100% (99/99) | `Numeric` 暴露 `has_sign`（§4.3.13），供 An+B 判符号 |
| muskitty-selectors | 94.3% (479/508) → **99.8% (507/508)** | W-3b：`an_plus_b.rs` 按 `<signed-integer>`/`<signless-integer>` 拒带符号位（`n 5`、`n- +5`、`5n + +5`、`n-+1` 等 27 例） |
| muskitty-html5-parser | 97.6% (1905/1924) → **99.8% (1921/1924)** | ① fragment 语境外用 `adjusted_current_node()` 而非栈顶取命名空间——修 18 例 foreign-fragment.dat 将 SVG/MathML 子元素误入 HTML 命名空间；② 新增 in-select / in-select-in-table 插入模式强化 select 内容模型 |
| muskitty-html5-tokenizer | 已有 → 99.8% (7022/7036) | （`<?` 处理指令/注释等已按 HTML5 判定合规） |
| muskitty-css-parser | 已有 → 100% (27/27) | — |
| muskitty-css-values | 已有 → 16/16 | — |

## 保留的记录偏差（合规范、不合旧夹具）

3 例 tree-construction 失败属**夹具陈旧 × 现行 WHATWG 规范**分歧，按仓库
"规范>测试"约定保留实现、不迁就夹具（规范 2026 现行文本已核对）：
- `tests_innerHTML_1.dat` #77 `<keygen><option>`、#78 `<textarea><option>`（fragment 源
  `select`）：夹具预期 keygen/textarea 被插入 select 内；现行规范 §13.2.6.15
  "in select" 明确 `input`/`keygen`/`textarea` start tag = parse error + **忽略**。
- `webkit02.dat` #19 里元素名 `xh<optgroup`：legacy webkit 解析器产物；现行 tokenizer
  不会产出含 `<` 的标签名，属规范之外的废名，不可复刻。

## 部署

- 报告重新生成：`.wpt-report/report/index.html`；部署目录 `.wpt-report/deploy/` 同步。
- gh-pages 分支重发（`245a3c4 → 71db2b4`）：https://ink-dark.github.io/MusKitty/
- 线上 README 套件表同步为现值（selectors 507/508、html5-parser 1921/1924）。
