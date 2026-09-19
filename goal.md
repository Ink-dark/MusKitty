# Goal — 2026-09-19 全量审计修复轮（bug / critical issues）

> **状态**：🔄 进行中
> **依据**：2026-09-19 全量代码分析（拉取 11 个独立 crate 后 4 组并行深度审查 +
> 人工复核），问题清单见下表。按严重度排序修复，每项走 Verification Flow
> （先写 failing test → 修 → 全绿 → commit），全部完成后统一 push。

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

## 复跑命令

```bash
cd /workspace && cargo test --workspace
cd /workspace && cargo clippy --workspace --all-targets -- -D warnings
cd /workspace && cargo fmt --all -- --check
# 各独立 crate 在其目录下同
```
