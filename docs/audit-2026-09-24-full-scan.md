# 2026-09-24 全量代码审计 + 进度/规划对照

> **日期**：2026-09-24
> **范围**：14 个 crate（3 个 workspace member + 11 个独立仓库按 `crates.json` 拉取到 HEAD）
> **方法**：拉取全部独立仓库 → 6 组并行深度源码审查 → 实测基线（`cargo test --workspace`、
> 各 crate 测试、`fmt`/`clippy`）→ 与 `goal.md` / `PROGRESS.md` / `docs/plans/*` 逐项对照
> **一句话结论**：**HEAD 不可编译、fetch 脚本在 Windows 全失效、claimed-done 的 CSS 补全批次
> 在 cascade 侧根本不存在**。前两项是阻断，第三项让 renderer 里已经写好的三条特性成了死代码，
> 并且有两个 e2e 像素测试正在红。

---

## 〇、结论速览（本轮决策）

| 优先级 | 事项 | 状态 |
|---|---|---|
| **P0** | renderer HEAD 编译失败（`render_tree.rs` tests 缺收尾 `}`） | ✅ 已修（待 commit） |
| **P0** | `fetch-crates.sh` 在 Windows 全盘失效 → 拉取 0 个 crate → checkout 不可构建 | ✅ 已修（待 commit） |
| **P1** | cascade 缺失 `border-radius` 家族 + `background-repeat/position/size` 注册 → renderer 三条特性死代码、**2 个 e2e 正在红** | 🔴 本轮主线 |
| **P1** | 三处文档（PROGRESS / goal / css-completion）把这些标为 ✅ 已完成，所引 cascade commit 在仓库中**不存在** | 🔴 本轮收口 |
| **P2** | network 无代理策略 → 环回请求穿代理 → 3 处测试依赖宿主环境变量失败 | ⚪ 本轮顺带 |
| **P3** | 上一轮（2026-09-19）遗留 C/H/M/L 共 26 项——**除 renderer 的 3 项外全部未修** | ⏭ 下轮 |

---

## 一、实测基线（本轮开工前）

| 项 | 结果 |
|---|---|
| `cargo check --workspace` | ❌ **编译失败**（修前）→ ✅ 通过（补 `}` 后） |
| `cargo test --workspace` | renderer 66 unit ✅ / 33 e2e ⚠️ **2 failed**；chrome 117 ⚠️ **1 failed**；network ⚠️ **2 failed** |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 干净 |
| `cargo fmt --all -- --check` | ✅ 干净 |
| 失败测试清单 | `end_to_end_background_no_repeat_paints_single_tile`、`end_to_end_background_position_center_centers_single_tile`、`navigation::tests::spawn_http_navigation_refused_returns_failed_outcome`、`network/tests/fetcher.rs` 2 项 |

---

## 二、P0 阻断（已修）

### A-0a · renderer HEAD 不可编译

`crates/muskitty-renderer/src/render_tree.rs:614` 的测试 `resolve_font_size_clamps_non_finite_and_huge_values`
缺收尾 `}`，导致其后的 `use Numeric;`、opacity 三测、`visibility` 测被吞进同一函数体，
`mod tests` 未闭合 → **整个 workspace 无法编译**（merge `fbbe59e` 带入）。

修法：补 1 行 `}`。验证：check / test / fmt / clippy 全部恢复。

> 副作用提示：这也是今日所有审查无法跑端到端像素证据的首要原因（先前的 e2e "全绿"记录无法复现）。

### A-0b · `fetch-crates.sh` 在 Windows 上完全失效

`json_array()` 走 python3 分支时，Windows 的 python3 写管道会把 `\n` 转成 `\r\n`，
每个 crate 名尾部残留 CR → 名称白名单校验 `^[A-Za-z0-9_.-]+$` 拒绝第一个条目并以错误退出：

```
[✗] crates.json 含非法仓库名（仅允许字母/数字/._- 且不以 - . 开头）: muskitty-cascade
```

结果：**拉取 0 个 crate**，`cargo check --workspace` 直接报找不到 `muskitty-cascade/Cargo.toml`。
即"新设备 clone 后一键初始化"这条README承诺在 Windows 上是不成立的（`fetch-crates.ps1` 无此问题）。

修法：在 python3 分支管道末尾加 `tr -d '\r'`。验证：`fetch-crates.sh` 成功拉取 11 个仓库。

---

## 三、P1 失真：文档标"已完成"，代码没有

### 事实链

1. `goal.md:81-84 / 110-114` 与 `PROGRESS.md:3-9` 记录：高频 CSS 补全第一批
   （`border-radius` + `background-repeat/position/size` + `opacity/visibility`）**已完成**，
   引用 cascade commit `58efc45` / `fea6b84` / `7d86720`。
2. `git -C crates/muskitty-cascade cat-file -t <hash>` → 三个哈希**均不存在**；
   `cascade` 仓库 HEAD = `5882097`（2026-09-19，`MQ-V` @media 重写），`unpushed=0`、`dirty=0`。
   **→ cascade 侧该批工作已丢失**（未推送且已被覆盖）。
3. 实证据：

| 属性 | cascade registry（80 条） | renderer 消费方 | 后果 |
|---|---|---|---|
| `border-radius` / `border-<corner>-radius` ×4 | ❌ 未注册 | `render_tree.rs:463-502 extract_border_radius`、`paint.rs:318` | 恒 0，圆角永不生效，**零 HTML 级 e2e 覆盖** |
| `background-repeat` | ❌ 未注册 | `render_tree.rs:107` | 恒初始值 → `end_to_end_background_no_repeat_...` **FAILED** |
| `background-position` | ❌ 未注册 | `render_tree.rs:131` | 恒初始值 → `end_to_end_background_position_center_...` **FAILED** |
| `background-size` | ❌ 未注册 | `render_tree.rs:218` | 恒 auto（`cover` 那条测试用 1×1 图，无鉴别力，侥幸绿） |
| `outline-offset` | ❌ 未注册 | `command.rs:232` | 显式记录"尚未注册"，**诚实** |

4. 未被丢弃的声明会在 cascade `normalize_property_name`（`filter.rs:404-412`）被静默移除 →
   renderer 的提取函数永远读到"键缺失" → 回退初始值。**功能等于未上线。**

### 为什么这是本轮最该修的

- 它是**唯一一处"文档与代码不一致"**：其余缺口都诚实地标注在各种批次表里，只有这三项被记成已完成。
- renderer 侧工作全部在位，只需补 cascade 注册 + 简写展开即可打通，**投入产出比最高**。
- 有**正在红的测试**做客观验收标准，不需要靠主观判断"做完了没"。

---

## 四、上一轮（2026-09-19）遗留项核实：**26 项几乎全部未修**

| 组 | 编号 | 结论 |
|---|---|---|
| renderer | C-1 PNG 解码炸弹 / H-9 HiDPI 背景图坐标 / H-11 font-size 钳制 | ✅ **已修**（`963b7e7`，有代码与回归测试佐证） |
| renderer+chrome | H-10 file 页面远端子资源在 UI 线程同步抓取 | ❌ 未修（见下 §5 新发现 C-7/N-1） |
| html5-tokenizer | H-1 temporary_buffer 不清；M-1 CRLF 预处理 | ❌ 未修（`impls.rs:664/672/585`；调用方各自 `preprocess_input`） |
| html5-parser | H-2 `&name[3..]`；H-3 current_node vs adjusted；M-2 void 盲 pop | ❌ 未修（`foreign.rs:373/602`；`dispatch.rs` 8 处 pop） |
| dom | H-5 replace_child 无 pre-insert 校验；H-6 insert_before 陈旧索引；M-8/M-9/M-13 | ❌ 未修（`tree.rs:54-97/125-157/17-36`；递归无上限） |
| css-parser | H-4 `! important`；M-6 CR 源 span 偏移；M-7 custom property original_text | ❌ 未修（`algorithms.rs:326-355` / `token_stream.rs:92` / `:238-273`） |
| selectors | H-7 i/s 标志忽略；H-8 `nth-child(of S)` 预算污染；M-10/11/12 `:has` 三项 | ❌ 未修（`simple_matcher.rs:107-136` / `pseudo_matcher.rs:90-98,196-203`；且 **M-11 的错误值被测试钉死**在 `tests/specificity.rs:153-160`） |
| cascade | M-3 `@media {}` 空列表；M-4 悬空 `not`；M-5 font-size 关键字不缩放；L 批 | ❌ 未修（`filter.rs:1247-1255/1277-1322`；`style_tree.rs:450-465`） |
| network | M-14 重定向策略全默认 | ❌ 未修（`reqwest_impl.rs:41-46` 无 `.redirect(...)`） |
| chrome | M-15 file/data 无上限；M-16 盘符/UNC；M-17 fold 顺序 | ❌ 未修（见下） |

**根因**：11 个独立仓库的 HEAD 停在 **2026-09-13 / 09-14**（仅 cascade 到 09-19、layout 到 09-18），
而审计日是 09-19——那一轮只在主仓库改了 `goal.md`/docs，**从未落到任何独立仓库**。
（唯一例外：renderer 属于 workspace member，三个修复跟着主仓库提交了。）

---

## 五、新发现精选（按严重度）

### P1/P2 —— 会立刻影响"能不能用"

| # | crate | 问题 | 位置 |
|---|---|---|---|
| N-1 | network | **无显式代理策略**：reqwest 默认继承 `HTTP_PROXY`，且不过 loopback → 本机 `http://127.0.0.1:1` 穿代理拿到 502 `text/plain`，被 `document_from_response`（`navigation.rs:193-202`，不读 status）当正文渲染成 `<pre>` | `reqwest_impl.rs:41-46`；`navigation.rs:176-214` |
| N-2 | renderer | 嵌套 opacity **每层分配整幅画布 Pixmap** 并递归持有，深度上限 512 → 4K 画布理论 ~17 GB | `tiny_skia.rs:517/525`；`paint.rs:225-232` |
| N-3 | renderer | `background-position` 百分比未按 `(盒 − 图) × p%`（缺减图像尺寸）：`100% 100%` 会把图推出盒外完全不绘制 | `tiny_skia.rs:886-891` |
| N-4 | renderer | `visibility: hidden` 跳过自身背景/边框/图，但**仍绘制 outline** | `paint.rs:389-407`（bg 分支有保护 `:294-296`） |
| N-5 | chrome | H-10 被热重载放大：`poll_source` 每次保存都在 UI 线程同步抓全部外链（单请求最长 30s） | `app.rs:271/455/619-627` → `stylesheets.rs:475/508` |
| N-6 | layout | **`position: static` 元素的 `top/right/bottom/left` 实际生效**（taffy 对 in-flow 也应用 inset），违背 Positioned Layout L3 | `layout/src/style_map.rs:87-92` |
| N-7 | layout | 文本测量不区分 `MinContent`/`MaxContent`：min-content 查询返回 max-content → flex/grid 收缩下界被高估，窄容器长文本溢出 | `layout/src/lib.rs:96-99` |
| N-8 | chrome | 热重载不更新图像表（`set_images` 缺失）——另两条加载路径都有 | `app.rs:283-288` vs `:229-232`/`:456-459` |
| N-9 | chrome | `source_tab` / `(tab, epoch)` 用**索引**定位标签 → 关标签后热重载写错页、过期导航结果错配 | `app.rs:181/280-288/493-527`；`webview.rs:187-216` |
| N-10 | chrome | `@import` 展开后，`url()` 按**顶层表**绝对化，而非声明所在被导入表 | `stylesheets.rs:421/425` + `images.rs:176-182` |

### P2/P3 —— 正确性与健壮性存量

- **cascade**：`font` 简写不 reset 其余 font 长属性（`filter.rs:754-781`）；`@supports` 只校验属性名不校验值（`filter.rs:1681-1694`，fail-open）；`@scope` 未做作用域限定（`filter.rs:269-272`，fail-open）；`gap`/`flex` 作为简写却注册为长属性（死键）。
- **selectors**：`:has` 的 Child 候选无上限（其余三种有 10k 封顶）；`:has` 候选求值绕过 `MAX_MATCH_STEPS`；栈预算耗尽后**不可复位**（永久假阴性）；命名空间前缀在匹配时被丢弃；calc 无类型系统/除零/`+ -` 空白规则。
- **css-values / cssom**：`Integer::from_cvs` 用 `as i32` 饱和（`99999999999` → `i32::MAX` 而非无效）；`setProperty` 缺 priority 且只替换最后一条同名声明；无限值序列化成 `inf` 非法字面量。
- **html5 / dom**：所有插入 `let _ =` 吞错且不记 ParseError；foreign breakout 循环在栈空时可能不终止；`serialize_node`/`clone_node`/`text_content` 递归无深度上限（DOM API 不受解析器 512 限制）；`normalize` O(n²)；跨文档 append 不做 adopt。
- **健壮性通则**：6 个 crate 未声明 `#![warn(missing_docs)]`（硬规则"公共 API 有 doc comment 引规范" 无编译期保障）；css-values 的 `numeric.rs`/`serialize.rs` doc 缺口最大。

> 完整条目（含 file:line 与修法）见本报告各组明细，此处仅收警示级以上的_TOP_。

---

## 六、进度 vs 规划对照

| 规划来源 | 声称 | 实测 | 判定 |
|---|---|---|---|
| `README.md` 状态表 | 14 crate 齐备，publish 版本对齐 | crates.io 版本与 README 一致 | ✅ |
| `PROGRESS.md:3-9` | CSS 补全第一批三条 batch 全完成 | 缺 cascade 侧，2 个 e2e 红 | ❌ **失真** |
| `goal.md:81-84` | 同上，含退出条件全满足 | 同上 | ❌ **失真** |
| `css-completion.md:112/117` | 批次 5 三项已划掉 | 同上 | ❌ **失真** |
| `css-completion.md` 批次 3b | letter-spacing/word-spacing/font-style 待做 | 确实未做（registry 有、零消费） | ✅ 诚实 |
| `AGENTS.md` 当前阶段 | M-3 batch 3c + BG-1 + MQ-V 已完成 | MQ-V 在（`filter.rs` 三值实现）；BG-1 半通（图像能画，但 repeat/position/size 未注册） | ⚠️ **BG-1 记"完成"偏乐观** |
| phase5-network | 自研 HTTP 栈远期路线 | trait 抽象完好，仍为 reqwest；M-14 重定向策略未做 | ✅ 路线未变 |
| JS 引擎评估 | 建议先做宿主层（执行时机/事件循环/DOM 绑定） | 未开工（`scripting flag` 仍恒关） | ✅ 一致（未声称开工） |

**总账**：WPT 仍在 99.83%（9592/9610，未重跑）；11 个 crate 的未推送工作 = **cascade 三批（已丢失）**；
其余仓库 `unpushed=0`、`dirty=0`，即本地没有"忘记推送"的在途改动。

---

## 七、本轮（2026-09-24）决策

> **本轮主题：接线与收口** —— 不是新功能，也不是继续修 20+ 存量 bug；而是让"声称已完成"的东西真的成立，
> 并把这类失真变成不可能复发。

| # | 任务 | 退出条件 |
|---|---|---|
| **B-1** | cascade 补注册 `border-radius` 简写 + 四角长属性 + 1–4 值展开；补注册 `background-repeat/position/size` + `background` 简写展开三分量 | registry 命中；`end_to_end_background_no_repeat_*` / `..._position_center_*` **由红转绿**；新增 `border-radius` 的 HTML 级像素用例（此前零覆盖）；`size: cover` 用例改用非 1×1 图以获得鉴别力 |
| **B-2** | 修 `background-position` 百分比语义为 `(盒 − 图) × p%`（§3.6），并补 `100% 100%` 回归 | `100% 100%` 图像贴右下角而非出盒不可见 |
| **B-3** | 加 **registry ↔ 下游消费方一致性测试**：扫描 layout/renderer 里 `style.get("<prop>")` 的字面量 ⊆ `BUILTIN_PROPERTIES` | 该测试能捕获本报告 §三 的全部 5 项，防再次出现"下游写了读取、上游没注册" |
| **B-4** | 文档收口：`PROGRESS.md` / `goal.md` / `css-completion.md` 把上述三项从 ✅ 改为"进行中（cascade 侧重做）"，注明 cascade 侧 commit 丢失的实据 | 三处口径一致，不再出现引用不存在的 commit |
| **B-5**（顺带） | network 显式代理策略 + loopback bypass（`no_proxy`），使 `127.0.0.1` 直连 | chrome `spawn_http_navigation_refused_*` 与 network 2 项转绿，**且不再依赖宿主是否有 `HTTP_PROXY`** |
| **B-6**（顺带，若余力） | `visibility: hidden` 跳过 outline | 一条 e2e：`hidden` + `outline` 应无墨迹 |

**显式非目标（本轮不做）**：上一轮 26 项存量修复（H-1~H-8 / M-1~M-17 等）——它们分散在 6 个独立仓库、
每项都要独立 regretion test，混入本轮会让"是否收口"这件事失去可判定性；整批排下一轮。
另外约定：任何跨 crate 的功能，**验收口径必须是 `registry 命中 + 端到端像素/值断言`**，
不接受"renderer 单测手工构造 `ComputedStyle` 通过"当作完成。
