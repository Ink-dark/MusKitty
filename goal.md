# Goal — CS-1：外链 CSS 与其他 CSS 来源接入（2026-09-14）

> **状态**：✅ **已完成**（CS-1a~CS-1f 全部满足退出条件，见文末"完成记录"）。
> **依据**：[docs/plans/2026-09-13-external-css-and-css-sources.md](docs/plans/2026-09-13-external-css-and-css-sources.md)
> （规划轮已交：现状证据表、决策 D1–D7、批次划分、规范本地源行号）。
> **上一轮**：W-3b（An+B 符号保真，WPT selectors 99.8%）——记录见 PROGRESS 头部与
> [docs/wpt-compliance-2026-09-06.md](docs/wpt-compliance-2026-09-06.md)。

## 任务与退出条件

| # | 任务 | 退出条件 |
|---|------|---------|
| CS-1a | **URL 基建**：`muskitty-network` 引 `url = "2"`（非可选，纯 Rust，已在 lock 经 reqwest 传递存在）；新增 `src/url.rs`：`resolve(base, reference)` / `file_url_from_path` / `path_from_file_url` / `is_fetchable_subresource`；pub 签名只出现 `&str`/`String`（对齐 decoupling ADR） | 解析用例表全绿（`../`、`./`、`//host/p`、`?q`、`#f`、百分号与非 ASCII、`file:///D:/x` ↔ `D:\x`、空串、绝对 URL）；scheme 策略表全绿（http→file 拒、file→http 允、`data:` 允、`about:`/`javascript:` 拒）；`cargo test -p muskitty-network` 全绿；fmt/clippy 干净 |
| CS-1b/c | **采集 + 抓取**：新 `chrome/src/stylesheets.rs`——`collect_sheet_sources`（DOM 先序 = 文档序；`<style>` 取 `text_content()`；`<link>` 按 rel 词表/`href`/`media`/`type`/`title`/`alternate`/`disabled` 语义；首个可用 `<base href>` 作文档 base）；`SheetLoader`（抓取 + 去重缓存 + 上限：单表 8 MiB / 每文档 64 表 / 深度 16 + 失败非致命）；`DocumentFetcher`（http(s) 走网络层 + Content-Type 必须 `text/css`；file 走本地读；`data:` 最小解码；http 页拒 `file://`）；删 `extract_inline_style` 与 4 个调用点 | 采集顺序/属性语义逐条有断言（注释内 `<style>` 不命中、`rel="next stylesheet"`、`type="text/plain"` 跳过、`disabled`、`alternate`、空 `href`）；抓取去重与上限有断言；`render_page` 增 sheets 入口且保留单表旧入口；chrome/workspace 编译与既有测试全绿 |
| CS-1d | **sheet 级字段**：cascade `prepare_sheets_with_context` 跳过 `disabled`/`alternate` 表、按 `sheet.media` 求值（复用 `eval_media_query_list`）；chrome 把 `media` 属性经 `parse_comma_separated_list_of_component_values` 填入 | cascade 单测：`print` 表被跳过、`disabled` 表被跳过、空/非法 media 语义；端到端：`media="print"` 表不生效 |
| CS-1e | **`@import`**：加载期就地展开——合法位置判定（其他规则之后出现的 import 无效）、以**导入表自身 URL** 为基准解析、条件导入包 `CssRule::Media`、循环/深度/失败跳过；`layer()`/`supports()` 前缀整条跳过（注释引规范） | 单测：顺序、相对基准、条件包裹、A→B→A 不挂死、深度上限、失败跳过、后置 import 无效；e2e：`@import url("a.css")` 与 `@import "b.css" screen` |
| CS-1g | **热重载**：`SourceFile` 监视集合扩到「HTML + 该页 file:// 外链表路径」，任一 mtime 变化 → 重新采集/加载 | 改外链 CSS 文件触发重载且像素变化；改 HTML 仍触发 |
| CS-1f | **UA 样式表（最后做，会改全仓像素预期）**：`chrome/src/ua.css` + `ua.rs`，`render_page` 系列入口把 UA 表置于表列表首位；内容按 HTML §15.3.1/§15.3.2/§15.3.3/§15.3.6/§15.3.7 最小集（逻辑属性按 horizontal-tb 等价物理属性写，偏差记录）；layout `is_non_rendered_tag` 保留为防御并注释指向 UA 表 | 像素断言：7 个此前会出盒的标签（`area/datalist/basefont/noembed/noframes/param/rp`）不再出盒、`<p>`/`<h1>` 默认边距与字号生效、`body` 默认 8px、`[hidden]` 不渲染；chrome/renderer/layout 全量测试绿（预期变化一次性校准并在 commit 说明） |
| CS-1e2e | **离线端到端**：`std::net::TcpListener` 迷你静态服务器（多文件 fixture） | 7 条断言全绿：相对路径生效、后出现的表胜出、404 表跳过且其余生效、`media="print"` 不生效、`@import` 链以导入表为基准（子目录 fixture）、成环限时返回、`data:` 表生效 |
| CS-1z | **收尾**：文档（PROGRESS 行、本 goal 完成记录、规划文档状态改"已实施"） | 全量 `cargo test --workspace` + cascade/layout 各自仓库测试全绿；`cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo fmt --all --check` 干净；commit 落盘并 push（cascade 改动提交到其独立仓库） |

## 显式非目标（本轮不做，理由见规划文档"非目标"表）

- `@import` 的 `layer()`/`supports()` 前缀（整条跳过，注释引规范）
- CORS / `integrity` / `crossorigin` / `referrerpolicy`
- alternate 样式表的切换 UI（本轮按 disabled 处理）
- `Link:` 响应头、`preload`/`prefetch` 等链接类型
- CSS 编码全解码（BOM 嗅探 + UTF-8 为准，其余 lossy；触发条件已记录）
- `url()` 相对解析（renderer 无图像管线）
- 首屏渐进渲染（外链本轮为 render-blocking）

## 风险与既定裁决

- **`CssStyleSheet: Send` 未验证**：第一步加静态断言测试；不成立则改为"线程回传文本+元数据，UI 线程建表"（规划 D2）。
- **签名涟漪**：`render_page` 新增 sheets 入口但保留单表旧入口，把测试改动降到最小；`NavigationDoc.css: String` → `sheets`，`WebView.css` → `sheets`，调用点逐个机械改。
- **UA 表注入**改变既有像素预期 → 放最后一批，集中一次校准，不保留旧口径兼容分支。
- **上限是本实现策略**（浏览器无硬限），值写在代码注释与规划文档，触发条件：真实页面命中。

## 复跑命令

```bash
cd D:/Muskitty && cargo test -p muskitty-network          # CS-1a
cd D:/Muskitty && cargo test -p muskitty-chrome           # CS-1b/c/e/g/e2e
cd D:/Muskitty/crates/muskitty-cascade && cargo test      # CS-1d
cd D:/Muskitty/crates/muskitty-layout && cargo test       # CS-1f 回归
cd D:/Muskitty && cargo test --workspace
cd D:/Muskitty && cargo clippy --workspace --all-targets -- -D warnings
cd D:/Muskitty && cargo fmt --all -- --check
```

## 完成记录（2026-09-14）

| # | 交付 | commit | 验证 |
|---|------|--------|------|
| CS-1a | `muskitty-network::url`：`resolve` / `file_url_from_path` / `path_from_file_url` / `scheme` / `is_fetchable_subresource` / `decode_data_url`；`url = "2"` 非可选依赖（原为 reqwest 传递依赖），pub 签名只出 `&str`/`Vec<u8>` | 主仓库 `30cfca6` | 12 模块测试（相对/协议相对/query/fragment/百分号与 CJK/file 往返/策略表/data URL 变体与错误）；network 10 + 4 doc-tests 全绿 |
| CS-1b/c | chrome `stylesheets` 模块：DOM 文档序采集（属性语义表 + `<base>`）、`DocumentFetcher`（scheme 分发 + MIME + BOM 编码）、`SheetLoader`（去重缓存 + 三项上限 + 失败非致命）；删 `extract_inline_style` 与 4 个调用点 | 主仓库 `6f25857` | 22 模块测试（顺序/注释与脚本免疫/属性语义表/base/上限/去重/`Send` 断言/fetcher 跨 scheme 拒绝） |
| CS-1c | 接线：`render_page_with_sheets`（旧单表入口保留）、`NavigationDoc{final_url,html,sheets,is_html,stats}`、导航线程内抓表、`WebView.sheets`、file 模式与 `file://` 导航走同一加载路径 | 同上 | chrome 108 lib + 3 headless + 6 probe 全绿；`--no-default-features` check 通过 |
| CS-1d | cascade：sheet 级 `disabled`/`alternate`/`media` 门控（复用 `eval_media_query`） | cascade `6ed08a0`（已 push） | 6 条新 filter 测试；cascade 231 全绿 |
| CS-1e | `@import` 加载期就地展开：合法位置、以导入表 URL 为基准、条件导入包 `@media`、循环/深度/失败跳过、`layer()`/`supports()` 前缀整条跳过 | 随 `6f25857` | 7 条模块测试 + 2 条 e2e（子目录相对基准、成环限时、内嵌表以文档为基准） |
| CS-1e2e | 离线多文件服务器（`TcpListener`）+ 9 条全链路像素断言 | 随 `6f25857` | 全绿（相对路径/后表胜出/404/media=print/disabled/@import 链/成环/data:/内嵌 import） |
| CS-1g | 热重载监视集合扩到 HTML + 其 `file://` 样式表 | 随 `6f25857` | `hot_reload_picks_up_external_css_change`（只改 CSS → 重载 → 声明值 red→green） |
| CS-1f | 最小 UA 样式表（`ua.css` + `ua.rs`，HTML §15 Rendering，只用有消费方的属性，未写规则与偏差逐条记录）+ `render_page_with_sheets` 统一注入（`OnceLock` 缓存） | 主仓库 `0f7df1e` | 7 条像素断言（15 标签清单中此前出盒的 7 个零墨迹 + 对照、body 8px、h1 2em/边距、p 1em、`[hidden]` 与作者覆盖、列表/引用缩进、hr） |
| CS-1f-doc | layout `is_non_rendered_tag` 注释化为防御并记录删除条件 | layout `ef63936`（已 push） | layout 127 全绿 |

**全量复跑**：`cargo test --workspace`（chrome 108+3+6+9+7、renderer 110、network 21+4）、cascade 231、layout 127 全绿；`cargo clippy --workspace --all-targets -- -D warnings` 与 `cargo fmt --all -- --check` 干净。

**实测语义**（与新行为对照的验收点）：外链 CSS（含同目录/子目录相对路径）端到端生效且像素正确；后出现的表在等特异性下胜出；404/抓取失败的表跳过而页面其余照渲；`media="print"` 与 `disabled` 表不生效；`@import` 以**导入表自身** URL 为基准解析、成环限时返回；`data:text/css,...` 表生效；UA 表使 7 个原出盒标签零墨迹、body 8px、h1 32px/边距 21.44px、`ul` 左缩进 40px、`[hidden]` 隐藏且作者 `display:block` 可覆盖。

**已知偏差与债务（已记录，不修）**：`@import` 的 `layer()`/`supports()` 前缀整条跳过（`supports` 不匹配时规范要求不得抓取，`layer()` 按 media 求值会误丢整表）；外链为 render-blocking（无首屏渐进渲染）；编码 BOM 嗅探 + UTF-8 为准，其余 lossy；`<base href>` 影响全部来源（规范只影响其后出现的 URL）；`url()` 相对解析随图像管线另排；layout 硬编码跳过表与 UA 表双轨（删除条件写在函数注释）。

**Mimosa 交互记录**：本轮 commit/push 与 layout/cascade 两个独立仓库的推送都收到"未取得完整扫描结论"的兼容放行警告；按仓库约定继续。另：曾试图用 Bash 追加测试文件被 hook 拦下，已改用 Edit 提交同一内容。
