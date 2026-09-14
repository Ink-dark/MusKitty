# 外链 CSS 与其他 CSS 来源接入规划（CS-1，2026-09-13）

> **用途**：把 CSS 的**来源**补齐到浏览器语义——文档里的 `<style>`/`<link>` 按文档序采集、
> 相对 URL 解析、外链抓取、`@import` 递归、每张表自身的 media/disabled 生效，外加一份最小
> UA 样式表。判定纪律沿用 M-3：**缺陷与收益一律以实跑证据为准**（代码位置 + 端到端像素），
> 不采信文档声明。
> **状态**：📋 **规划完成，未实施**（本文件只定义批次与退出条件；实施时另开 `goal.md` 轮次）。
> **相关文档**：
> [docs/plans/2026-09-12-css-completion.md](2026-09-12-css-completion.md)（M-3 特性缺口总账）、
> [docs/plans/2026-08-09-phase5-network.md](2026-08-09-phase5-network.md)（N-6 才是规范指定的
> DOM/子资源接驳阶段）、
> [docs/decisions/2026-08-16-external-dependency-decoupling.md](../decisions/2026-08-16-external-dependency-decoupling.md)
> （外部依赖类型不得进 pub 签名）。

## 一、现状实测基线（证据）

| # | 环节 | 实测结论 | 证据位置 |
|---|------|---------|---------|
| 1 | `<style>` 采集 | 字符串扫描（全文小写化 + `find("<style")`），**不识别任何属性**（`media`/`type` 全忽略），且会命中注释/脚本字符串里的 `<style`；4 个调用点 | `crates/muskitty-chrome/src/page.rs:99-121`；调用点 `app.rs:153`、`app.rs:198`、`app.rs:380`、`navigation.rs:163` |
| 2 | `<link rel=stylesheet>` | **完全不支持**（注释明示"页外 CSS `<link>` 不在本轮范围"）→ 真实网页几乎全部依赖外链 CSS，这是"渲染真实页面"的最大单一缺口 | `navigation.rs:45` |
| 3 | base URL | 无任何承载：`WebView` 只有 `html/css/title`；DOM 的 `DocumentData.url` 恒为 `about:blank` 且解析器从不赋值；`<base href>` 不解析（按 void 元素插入即弹栈） | `webview.rs:25-47`、`document.rs:8,19`、`html5-parser/src/parser/dispatch.rs:318-326` |
| 4 | URL 解析 | 全 workspace 无相对解析能力（只有 `classify_url`/`leading_scheme`/`is_local_host` 三个字符串级函数）；`url 2.5.8` 已在 `Cargo.lock`（经 reqwest 传递）但**零代码使用** | `navigation.rs:69-148`、`Cargo.lock:2799` |
| 5 | 多表层叠 | **已就绪**：`prepare_sheets_with_context` 按 slice 顺序展平，`order` 为全局递增计数 → 等 origin/层/特异性时后一张表胜出；`@media`/`@supports` 已求值 | `cascade/src/filter.rs:106-122`、`filter.rs:300-320`、`filter.rs:219-235` |
| 6 | sheet 级字段 | `CssStyleSheet` 有 `location/media/title/alternate/disabled`，但 `from_stylesheet*` 全部硬编码为空，且**没有任何消费方**（连 `disabled`/`media` 都不读） | `cssom/src/stylesheet.rs:28-43`、`cssom/src/convert.rs:28-43`、`cascade/src/filter.rs:106-122` |
| 7 | `@import` | 解析到 `CssRule::Import`（有 `href`/`media`），cascade 把它与 `@font-face` 等一起**直接跳过** → 外链样式的嵌套导入零加载 | `cssom/src/rule.rs:122-128`、`convert.rs:112`、`cascade/src/filter.rs:258-265` |
| 8 | `style` 属性 | **已通**（cascade 直接读属性重解析，`from_style_attr` 参与排序） | `cascade/src/filter.rs:332-368`、`cascade/src/cascade.rs:55-65` |
| 9 | UA 样式表 | **不存在**。`Origin::UserAgent` 只有排序权重；layout 用硬编码 8 标签跳过表顶替 UA 表职责，其余元素一律映射 `Display::Block`（taffy 无 inline）→ 规范 §15.3.1 列出的 15 个非渲染标签中 `area/datalist/basefont/noembed/noframes/param/rp` 会生成盒；`p/h1/body` 无默认边距与字号 | `cascade/src/cascade.rs:50-61`、`layout/src/convert.rs:359-364`、`layout/src/style_map.rs:45-66` |
| 10 | 编码 | 响应体一律 UTF-8 lossy，无 BOM/`@charset`/Content-Type charset 处理（HTML 与 CSS 两侧同） | `network/src/response.rs:42` |
| 11 | 渲染时机 | 每帧 `render_page(&html, &css, …)` 重新解析 CSS（表文本进、逐帧 parse）；导航线程抓取顶级文档，子资源无路径 | `app.rs:257-264`、`page.rs:42-75` |

## 二、目标与非目标

**本轨道目标（4 条）**

1. 页面里的样式表按 **HTML 文档序** 采集：`<style>` + `<link rel=stylesheet>` 交错保序，
   元素属性（`media`/`type`/`disabled`/`title`/`alternate`）按规范语义处理。
2. **外链抓取**：http(s) 在导航线程内抓（不阻塞 UI），file:// 走同步读；失败非致命；
   单表/整页/深度有上限；同一 URL 只抓一次。
3. **`@import` 递归展开**：按 CSS Cascade L5 §"Importing Style Sheets" 的 in-place 语义展开
   到加载期，条件导入按 `@media` 包裹复用既有求值路径；含循环/深度/失败处理。
4. **sheet 级字段生效**：cascade 消费 `media`/`disabled`/`alternate`（`location` 仅记录），
   外加一份**最小 UA 样式表**（HTML §15.3.1/§15.3.2 起步），让页面默认外观接近浏览器。

**显式非目标（本轮不做，理由记录）**

| 项 | 理由 |
|----|------|
| `@import` 的 `layer()` / `layer` 关键字与 `supports()` 前缀 | 需 `@layer` 与 `@supports` 的导入期语义（cascade 层序 + 条件组），且规范要求 supports 不匹配时**不得抓取**。保守裁决：含这两种前缀的 `@import` 整条跳过并注释，宁缺勿错（见 D5） |
| CORS / `integrity` / `crossorigin` / `referrerpolicy` | 无同源模型与 SRI；当前请求也不带凭据语义 |
| alternate style sheet 的用户切换 UI | 无 UI 面；`alternate` 表本轮一律不生效（按 disabled 处理），字段保留 |
| `Link:` HTTP 响应头（process a link header）、`preload`/`prefetch`/`modulepreload` | 独立特性，与"样式来源"正交 |
| CSS 编码完整解码（BOM/`@charset`/Content-Type charset） | 需 encoding_rs 级解码表；本轮只做最小策略（见 D6），触发条件记录在 §八 |
| `url()` 相对解析（`background-image` 等） | renderer 无图像管线（M-3 batch 5），解析出来也无消费方 |
| 首屏渐进渲染（FOUC：先画无样式页再补样式） | 本轮外链是 render-blocking（表到齐才应用），与浏览器最终态一致；渐进渲染单列优化项 |
| 历史栈 / 刷新 / 二次导航复用 | 沿用 navigation.rs 现状范围 |

## 三、关键决策

### D1 URL 解析：直接引 `url` crate，落点 `muskitty-network::url`

- **决策**：`muskitty-network` 增加非可选依赖 `url = "2"`（纯 Rust；`Cargo.lock:2799` 已有
  2.5.8，经 reqwest 传递存在，不是新增供应链），新增 `src/url.rs`，pub 签名只出现
  `&str`/`String`（对齐 decoupling ADR，`url::Url` 不出现在任何 pub 类型里）：
  ```rust
  pub fn resolve(base: &str, reference: &str) -> Option<String>;      // WHATWG URL 参考解析
  pub fn file_url_from_path(path: &str) -> Option<String>;            // D:\x → file:///D:/x
  pub fn path_from_file_url(url: &str) -> Option<String>;
  pub fn is_fetchable_subresource(base: &str, target: &str) -> bool;  // scheme 策略（见 D4）
  ```
- **理由**：URL 是 WHATWG 规范算法（状态机 + IDNA + 百分号编码规范化），标准库无法替代
  （AGENTS.md 的"标准库能搞定不引 crate"在此不适用）；规范优先级 WHATWG > WPT > Chromium，
  而 rust-url 正是 WHATWG URL 的参考实现，比手写 RFC 3986 §5 子集更贴近 ground truth。
- **备选（不采用，记录）**：手写 RFC 3986 §5 解析（~150 行）——只在"必须零新依赖"时启用；
  代价是 IDNA/百分号编码/点段边界要自测，且与 WHATWG 有已知差异。
- **落点理由**：URL 语义属网络层（自研 HTTP 栈 N-1~N-7 与 N-6 Fetch 都要），chrome 已依赖
  network；file:// ↔ 路径转换是纯字符串运算，不引入 I/O。
- **许可证核对**：新增直接依赖会把 `url`/`idna_adapter`/`icu_normalizer`/`icu_properties`
  等从"传递"变成"直接可见"；`deny.toml` 的 licenses allow 列表当前是空模板（未入 CI），
  实施时把 MIT/Apache-2.0/Unicode-3.0 等按实际补进 allow。

### D2 传递单元用 `cssom::CssStyleSheet`，`render_page` 改收已解析表

- **决策**：把"CSS 文本 + 序号"升级为 `Vec<CssStyleSheet>`：loader 填 `origin`/`location`/
  `media`/`title`/`alternate`/`disabled`，`render_page(html, sheets, w, h, scale)` 直接消费；
  `WebView` 存 `sheets`（不再存 `css: String`）。
- **理由**：CSSOM §8.1 的这些字段就是为"一张来自某 URL、带 media/title 的表"设计的，现状是
  "字段齐备但零填充"；每帧少一次 CSS parse（正面副作用）；未来 `document.styleSheets` 类
  API 也有落点。
- **备选**：保留 `render_page(html, css: &str, …)` 并在内部加 `base_url` 参数 —— 会被
  "多表 + 每表 media/location"的需求立刻撑破，不取。
- **跨线程前提**：导航线程→UI 线程的 channel 需要 `CssStyleSheet: Send`（未验证）。实施第一步
  加 `fn assert_send<T: Send>()` 静态断言；若不成立，改为线程只回传
  `Vec<LoadedSheet{ location, media, title, alternate, disabled, css: String }>`，UI 线程在
  drain 时构造 sheet（解析成本一次性、与帧无关）。

### D3 采集走 DOM，不走字符串扫描

- **决策**：解析 HTML 一次 → 用 `Node::descendants()`（先序 DFS = 文档序，`dom/src/node.rs:398-406`）
  采集 `style`/`link`；`<style>` 内容取 `text_content()`（RAWTEXT 子文本节点即源码）；
  `get_attribute` 大小写不敏感语义直接用（`dom/src/element.rs:81-89`）。删除
  `extract_inline_style` 及其 4 个调用点。
- **理由**：正确性（注释/脚本里的 `<style` 不误命中、属性大小写、`<style>` 出现在 body、
  畸形 HTML 由解析器兜底）+ 属性语义本来就要按元素读 + 文档序免费获得。
- **代价与缓解**：加载期多一次 HTML 解析（渲染每帧本来就要解析一次；先不引入"Document 长驻
  对象"，把它列为后续优化项，触发条件：加载路径实测明显变慢）。
- **`<base href>`**：取树序中**第一个** `href` 能解析成功的 `base` 元素作为文档 base URL
  （HTML §4.2.3），在采集阶段完成；无 base 时用文档 URL。

### D4 抓取：导航线程内顺序抓，失败非致命，URL 去重缓存

- **时机**：http(s) 页面的全部外链（含 `@import` 递归）在**导航线程**内抓完再回传
  （沿用 `navigation.rs:228-260` 的线程 + mpsc 模式，UI 线程永不阻塞）；file 模式同步读
  （文件小，且热重载路径本就在 UI 线程读文件）。
- **scheme 策略**（`is_fetchable_subresource`）：http(s) base → 只 http(s)；file base →
  file + http(s)；`data:` 任何 base 都允许（URL Standard §data URL 的最小解码：
  `data:text/css,...` / `;base64`）；其余 scheme（`about`/`javascript`/`blob`…）拒。
  **http(s) 页不得读 `file://`**（与浏览器一致的安全边界，也是本次唯一涉及本地文件读取的
  新增面）。
- **上限**（防 DoS，且与"不挂死"的既有约定一致）：单表 8 MiB、每文档 64 张表、`@import`
  深度 16。超限按"该表跳过 + 注释说明"处理，不 abort 页面。
- **去重**：按解析后 URL 的 `HashMap<String,String>` 抓取缓存（同 URL 只发一次请求），
  但**每个引用点各建一张表**——规范明说两个 `rel=stylesheet` 的 link 是两个独立资源
  （`06-4the-elements-of-html.md:627`）。
- **失败**：单个表抓取失败（DNS/404/超时/Content-Type 非 `text/css`）→ 跳过该表，页面其余
  照渲（浏览器同语义）；只 `eprintln!` 一次，不生成错误页。
- **Content-Type 规则**：非 `text/css` → 表不生效（规范 `L11695`）；缺 Content-Type 时按
  默认类型 `text/css` 处理（`L11657`）。

### D5 `@import`：加载期就地展开，条件导入包一层 `@media`

- **位置规则**（CSS Cascade L5 `css-cascade-5/Overview.md:115-118`）：`@import` 必须位于其他
  有效 at-rule 与 style rule **之前**（`@charset`、`@layer` 语句除外），且不能与之前的
  `@import` 之间有其他规则；违反者整条无效 → 采集/展开阶段只处理"合法位置"的 import。
- **展开语义**（`L95-113`）：内容按"字面写在 `@import` 处"处理 → 加载期把
  `CssRule::Import` **就地替换**为导入表的规则；条件导入用 `CssRule::Media(condition)` 包住，
  复用 cascade 既有 `@media` 求值（`filter.rs:1127`），不引入新语义。
- **相对解析基准**：imported 表内的相对 URL（含嵌套 import）以**该表自身**的 URL 为基准，
  非文档 URL（`location` 字段的用途）。
- **循环与深度**：按 URL 维护"当前展开栈"，URL 已在栈中 → 跳过（循环）；深度 > 16 → 跳过。
- **`layer()`/`supports()` 前缀**：整条跳过（见非目标表）；代码注释引规范行号，文档与本
  计划同步记录。
- **抓取顺序**：import 先于宿主表其余规则生效，但**抓取**顺序按发现顺序（宿主表先抓、再抓
  其 import），失败互不影响。

### D6 编码：最小策略（UTF-8 为准），完整解码单列

- **本轮**：BOM 嗅探（UTF-8/UTF-16LE/BE → 按 BOM 去标记后按 UTF-8 处理，非 UTF-8 记一条
  `eprintln!`）；`link` 的 `charset` 属性与 `@charset`（css-parser 已整体丢弃）**只在
  utf-8 时生效**，其余按 UTF-8 lossy 解码并在注释/文档登记偏差。
- **理由**：网络层当前就是 UTF-8 lossy（`response.rs:42`），HTML 侧同缺口；CSS 单独做全解码
  会把 encoding_rs 拖进来，收益（GBK/Shift_JIS 样式表）在当前页面样本里为零。
- **触发条件**（转正式项）：第一个非 UTF-8 的真实页面/用户指令；届时连同 HTML 侧一起做。

### D7 UA 样式表：最小集放 chrome，layout 硬编码表降级为"防御保留"

- **内容 v1**（只用当前**有消费方**的属性；规范里的逻辑属性按 horizontal-tb 等价物理属性写，
  偏差记录在注释）：
  - 非渲染元素（HTML §15.3.1 `17-15rendering.md:57-81`）：
    `area, base, basefont, datalist, head, link, meta, noembed, noframes, param, rp, script, style, template, title { display: none }`
    → 顺带修掉现状中 7 个会出盒的标签。
  - `html, body { display: block }`；`body { margin: 8px }`（§15.3.2 `83-100`）。
  - 标题（§15.3.6 `385-393`）：`h1..h6` 的 `font-size`/`font-weight: bold`/`margin` 按
    `:heading(n)` 表展开成六条类型选择器（不用 `:heading()` 伪类，避免绑定 L5 匹配语义）。
  - 流内容（§15.3.3）：`p, blockquote, figure, dl, ol, ul, pre, form` 的默认 `margin`；
    列表（§15.3.7 `413`）：`ol, ul { padding-left: 40px }`。
  - `pre { font-family: monospace }`（`white-space: pre` 待 batch 3c 的 white-space 落地后加，
    注释写明原因）。
  - `b, strong { font-weight: bold }`（规范写 `bolder`，当前 `resolve_font_weight` 不消费
    `bolder` → 用 `bold` 近似并注明）。
  - 明确不加：`:link{color:#0000EE}`（需 `:link` 匹配 + 文档 URL 语义）、`::marker`、
    表单控件外观（§15.5）。
- **落点**：`crates/muskitty-chrome/src/ua.rs`（`include_str!("ua.css")` + 构造
  `Origin::UserAgent` 表）；`render_page` 永远把它放在 sheet 列表**首位**（cascade 的 origin
  权重已保证 UA 低于 Author，位置只为表达"先加载"）。不放 cascade：cascade 保持语义中立。
- **与 layout `is_non_rendered_tag`（`convert.rs:359-364`）的关系**：UA 表生效后该表冗余
  （同一效果由 `display:none` 产生），但 layout/renderer 的既有测试不注入 UA 表，直接删除会
  波及大量测试 → 本轮**保留**，在函数注释写明"UA 表已覆盖，保留为防御；删除条件：layout/
  renderer 测试统一走含 UA 表的入口"。
- **行为变化**：注入 UA 表会改变既有像素预期（如 `body` 默认 8px 边距、`h1/p` 默认边距）——
  这是**向浏览器对齐**的预期变化，一次性校准并登记在当轮 goal/commit。

## 四、批次与退出条件

| # | 批次 | 内容 | 落点 | 规模 | 退出条件 |
|---|------|------|------|------|---------|
| CS-1a | URL 基建 | `url` 依赖 + `network::url` 四个函数 + scheme 策略 | `network/src/url.rs`、`Cargo.toml` | S | 解析用例表全绿（`../`、`./`、`//host/p`、`?q`、`#f`、百分号/非 ASCII、`file:///D:/x` ↔ `D:\x`）；`cargo test -p muskitty-network` 全绿；clippy/fmt 干净 |
| CS-1b | DOM 采集 | `<style>`/`<link>` 文档序采集 + 元素属性语义（rel 词表/`media`/`type`/`disabled`/`title`/`alternate`）+ `<base href>`；删 `extract_inline_style` 与 4 个调用点 | 新 `chrome/src/stylesheets.rs`；`page.rs`、`app.rs`、`navigation.rs` | M | 采集顺序（style/link 交错、注释内 `<style>` 不命中、`<style>` 在 body）、属性语义表（大小写、`rel="next stylesheet"`、`href` 空、`type=text/plain`、`disabled`、`alternate`）逐条有断言 |
| CS-1c | 外链抓取 | 导航线程内顺序抓 + 去重缓存 + 上限 + 失败非致命 + `data:` 表 + file 同步读 + http 页拒 `file://` | `navigation.rs`、`app.rs`、`stylesheets.rs` | M | 离线 e2e（`TcpListener` 多文件）：相对路径生效、后表胜出、404 表跳过、`data:` 表生效；file 模式 temp-dir fixture 生效 |
| CS-1d | sheet 级字段 | cascade 跳过 `disabled`/`alternate`、按 `sheet.media` 求值（`<link media>` / `<style media>` 经 `parse_comma_separated_list_of_component_values` 填入） | `cascade/src/filter.rs:106-122`；`stylesheets.rs` | S | cascade 单测：`print` 表被跳过、`disabled` 表被跳过、空/非法 media 值语义；端到端：`media="print"` 不生效 |
| CS-1e | `@import` | 合法位置判定 + in-place 展开 + `@media` 包裹 + 以表自身 URL 解析 + 循环/深度/失败 | `stylesheets.rs`；必要时 `cssom` 小改 | M | 单测：顺序、相对基准、条件包裹、A→B→A 不挂死、深度上限、失败跳过、后置 import 无效；e2e：`@import url("a.css")` 与 `@import "b.css" screen` |
| CS-1f | UA 样式表 | `ua.css` 最小集 + `render_page` 首位注入 + layout 硬编码表注释化 | `chrome/src/ua.rs`、`page.rs`、`layout/src/convert.rs`（注释） | M | 像素断言：7 个非渲染标签不再出盒、`<p>`/`<h1>` 默认边距与字号生效、`body` 默认 8px、`[hidden]` 不渲染；chrome/renderer/layout 全量测试绿（预期变化一次性校准） |
| CS-1g | 热重载 + 收尾 | 文件模式监视集合扩到「HTML + file:// 表」；文档（本文件落成、PROGRESS 行、当轮 goal 收尾） | `app.rs:181-213` | S | 改外部 CSS 文件触发重载且像素变化；`cargo test --workspace` + 三 crate 全绿；fmt/clippy 干净 |

**建议实施顺序**：CS-1a → CS-1b → CS-1c → CS-1d → CS-1e → CS-1g → CS-1f
（CS-1f 放最后：它会改变全仓像素预期，最后做可避免边改边校准）。

## 五、规范依据（本地源，行号为本次核对）

| 特性 | 条款 | 本地源 |
|------|------|--------|
| `link` 元素 `media` 属性对外链**是规定性的**（不匹配则不应用） | §4.2.4 | `D:\whatwg\html\06-4the-elements-of-html.md:841` |
| `rel` 是词表（空格分隔、大小写不敏感）；`next stylesheet` 各自独立成链 | §4.2.4 | 同上 `:613`、`:627` |
| `stylesheet` 链接类型：默认类型 `text/css`、`body` 内合法 | §4.6.8.23 | 同上 `:11629`、`:11657`、`:10484`/`:10515` |
| `link disabled` 属性 → disable 关联样式表；移除 → 重新抓取并应用 | §4.2.4 | 同上 `:754-760`、`:11661`、`:11671` |
| 抓取前置：`disabled` 置位则**不抓** | 链接资源抓取步骤 | 同上 `:11684-11691` |
| 处理已抓资源：Content-Type 非 `text/css` → 失败；建表填 `location`/`media`/`title`/`alternate`/`disabled` | 同上 | `:11693-11790` |
| quirks 模式特例：同源且 Content-Type 非受支持样式类型 → 仍按 `text/css` | 同上 | `:11680` |
| 环境编码：`el` 的 `charset` 属性 → 否则**文档编码** | 同上 | `:11774-11782` |
| `alternate stylesheet`：需显式启用才生效；同 `title` 为一组 | §4.6.8.20 | 同上 `:10545-10561` |
| `style` 元素 `media` 属性；`title` 为样式表集名 | §4.2.6 | 同上 `:1960-2000` 前后 |
| `@import` 必须位于其他规则之前（`@charset`/`@layer` 语句除外），违反即无效 | Importing Style Sheets | `D:\CSSWG\css-cascade-5\Overview.md:115-118` |
| `@import` 内容按"字面写在 import 处"参与层叠 | 同上 | `:95-113` |
| 条件导入：不匹配时等同被包在 `@media`/`@supports` 里；`supports` 不匹配时 UA **不得抓取** | Conditional `@import` Rules | `:178-200` |
| UA 级默认表：以下各小节的规则"expected to be used as part of the user-agent level style sheet defaults" | §15.2 | `D:\whatwg\html\17-15rendering.md:35-53` |
| 非渲染元素清单（15 个标签）与 `[hidden]` | §15.3.1 | 同上 `:57-81` |
| `html, body { display: block }`；`body` 默认 8px 边距（presentational hint） | §15.3.2 | 同上 `:83-100` |
| `:heading(n)` 的字号/边距表；`ol, ul { padding-inline-start: 40px }` | §15.3.6/§15.3.7 | 同上 `:385-393`、`:413` |
| 样式表字段（location/media/title/alternate/disabled）与 CSSOM 侧"disable/移除"算法 | CSSOM §8.1 及关联算法 | `D:\CSSWG\cssom-1\Overview.md`（行号实施时按本地源核对） |

## 六、验证方案

**单测（快、无网）**

- `network::url`：相对解析表（`../`、`./`、`//`、`?`、`#`、空串、绝对 URL、`file:` 基准）、
  scheme 策略表（http→file 拒、file→http 允、`data:` 允、`about:`/`javascript:` 拒）。
- `chrome::stylesheets`：采集顺序与属性语义表；`@import` 展开表（位置规则/基准/条件包裹/
  循环/深度/失败）；上限行为（超表数、超体积）。
- `cascade`：sheet 级 `media`/`disabled`/`alternate` 跳过。

**离线端到端（像素级，CI 可跑）**

在 `chrome` 测试内起 `std::net::TcpListener` 迷你静态服务器（按路径返回 fixture 字节，沿用
`navigation.rs:471-504` 的手法），覆盖：

1. `<link rel=stylesheet href="a.css">` 相对路径生效（目标像素为 a.css 指定的颜色）；
2. 两个 link 都匹配时**后出现的胜出**（等特异性，验证 order 语义）；
3. `404` 的表被跳过，其余表仍生效；
4. `media="print"` 的表不生效、`media="screen"` 生效；
5. `@import` 链 `a.css → b.css`，且 b.css 的相对解析以 **a.css** 为基准（放子目录 fixture）；
6. `@import` 成环时限时返回、不挂死；
7. `data:text/css,…` 表生效。

**文件模式与热重载**

temp-dir fixture（`index.html` + `css/style.css`）：`render_html_file` 与 `App::with_source_file`
两条路径都生效；改 `style.css` → `poll_source` 触发重载 → 像素变化（CS-1g）。

**手工冒烟（非 CI）**

本地多文件页面（含外链 + `@import` + `media` 分支）在真窗口里肉眼确认；外加"外链 404 时页面
仍可读"的断网/断链检查。

## 七、风险与既定裁决

| 风险 | 处置 |
|------|------|
| `render_page`/`NavigationDoc`/`WebView` 签名变更的涟漪 | 调用点清单：`page.rs:90`、`app.rs:264`、`headless.rs:23,58`、`compositor.rs:68`、`tests/render_probe.rs:17`、`navigation.rs:163`、`app.rs:153/198/380`。逐点机械改；每步 `cargo check --workspace` |
| `CssStyleSheet` 是否 `Send` | 实施第一步静态断言；不成立则回退为"线程回传文本+元数据，UI 线程建表"（D2） |
| UA 表注入改变既有像素预期 | 预期变化，集中一次校准；所有断言改成"与浏览器一致"的口径（如 `body` 8px），不保留旧口径的兼容分支 |
| 顺序抓取把首次渲染推迟 | 接受（render-blocking 与浏览器最终态一致）；渐进渲染列优化项；断言里用 `recv_timeout` 防挂死 |
| 新增本地文件读取面（file:// 表） | scheme 策略函数强制：http(s) 页拒 `file://`；file 页只读同目录树外的任何本地文件都是用户自觉（与浏览器一致） |
| 上限选择被质疑过严/过松 | 上限是**本实现策略**（浏览器无此类硬限，靠内存与超时）；记录在代码注释与本文档，触发条件：真实页面命中上限 |
| 与"每帧重解析 CSS"的现状冲突 | 不再冲突：sheets 存已解析对象，渲染路径少一次 parse |

## 八、"其他 CSS 支持"的边界与后续排期

本规划覆盖的"其他来源"：`<style>` 元素属性、`<link>` 全属性、`@import`、`<base>`、UA 表、
`data:` 表、编码最小策略。**不在**本规划的 CSS 特性缺口仍按
[docs/plans/2026-09-12-css-completion.md](2026-09-12-css-completion.md) 的批次 3b/3c/4/5/6 走。

**推荐顺序（收益/成本比）**

1. **CS-1（本规划）**——外链 CSS 是"能否渲染真实页面"的第一道门；
2. **white-space 与空白折叠**（M-3 batch 3c 的核心项）——空白折叠影响几乎所有文本页面的
   观感，且 UA 表的 `pre{white-space:pre}` 等它落地才能加；成本集中在"测量与绘制两侧同一
   折叠实现"（与 batch 3 的 `text_props` 单一来源同款手法）；
3. **`background-image` + `border-radius`**（M-3 batch 5 主体）——观感提升最大的一批装饰；
   需 renderer 图像解码/绘制管线（可先只做 `linear-gradient` 与纯色，图片解码另议）；
4. **`opacity`/`z-index`/`visibility`**（M-3 batch 4）——需离屏合成与层叠上下文排序，
   `RenderTree` 中间结构届时重生；
5. **`letter-spacing`/`word-spacing`/`font-style: italic`**（M-3 batch 3b）——受 cosmic-text
   能力与系统字体面制约，需先定验证口径；
6. **布局消费方缺口**（M-3 batch 6：`order`/`justify-items`/`justify-self`/`grid-auto-*`/
   `cursor`）——低频或需 chrome 命中测试；
7. **编码面（BOM/`@charset`/Content-Type charset 全解码）**——触发条件见 D6；
8. **首屏渐进渲染 / 子资源并行抓取 / 抓取缓存持久化**——触发条件：真实页面实测变慢。

## 九、复跑命令（预留，实施后回填）

```bash
cd D:/Muskitty && cargo test -p muskitty-network          # CS-1a：URL 解析表
cd D:/Muskitty && cargo test -p muskitty-chrome           # CS-1b/c/d/g：采集、抓取、热重载
cd D:/Muskitty/crates/muskitty-cascade && cargo test      # CS-1d：sheet 级 media/disabled
cd D:/Muskitty && cargo test --workspace                  # 全量 + 集成
cd D:/Muskitty && cargo clippy --workspace --all-targets -- -D warnings
cd D:/Muskitty && cargo fmt --all -- --check
```

## 十、明确留给后续的债务（本轮记录，不修）

1. **UA 表与 layout 硬编码跳过表双轨**：CS-1f 后 `is_non_rendered_tag` 冗余但保留，删除条件
   写在函数注释里。
2. **逻辑属性未注册**：UA 表用物理属性等价替代（规范 `margin-block-*`/`padding-inline-start`）；
   逻辑属性（含 `direction`/writing-mode 交互）单列后续项。
3. **`@import` 的 `layer()`/`supports()`**：整条跳过，代码注释与文档同源。
4. **编码面**：UTF-8 为准 + BOM 嗅探，其余 lossy（HTML 侧同缺口）。
5. **DOM 二次解析**：加载期采集与渲染各解析一次 HTML；"Document 长驻对象"（DOM + sheets +
   URL 一体）列为后续重构，触发条件：加载路径实测或需要二次导航复用。
