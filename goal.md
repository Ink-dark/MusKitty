# Goal — W-3b：An+B 符号保真（WPT 选择器套件收尾，2026-09-13）

> **更新时间**：2026-09-13
> **状态**：✅ **已完成**（T-1~T-5 全部满足退出条件，见文末"完成记录"）。
> **实测结果**：**94.3%（479/508）→ 99.8%（507/508）**；剩余唯一 1 例为上一批
> 已记录的 tentative 夹具自相矛盾（`:has-slotted(div + div)`）。
> **上一批**：W-3 第一批（96 例，selectors `f8d2002` / css-tokenizer `23ec4ec`）。
> **本轮范围来源**：上一批 goal.md 明确留下的 28 例——全部是同一个根因：
> 解析器分不清 `<signed-integer>` 与 `<signless-integer>`。

## 根因（实测 + 规范核对）

CSS Syntax §7 L3377-3378 把两者定义为：

- `<signed-integer>`：type flag 为 "integer" 的 `<number-token>`，**带符号字符**；
- `<signless-integer>`：同类型但**不带符号字符**。

而 token 层 `5` 与 `+5` 完全同类：tokenizer 的 `consume_a_number` 按 §4.3.13 第 7 步
本应返回 `(value, type, sign)`，实现只返回了前两者（`impls.rs` 注释与 selectors
`an_plus_b.rs` 的"比规范更宽松"说明都记录了这一点）。于是：
`n + 5`（合法）与 `n 5`（非法）在 token 流上无法区分，`n- 5`（合法）与 `n- +5`
（非法）同样。

## 任务与退出条件

| # | 任务 | 退出条件 |
|---|------|---------|
| T-1 | **css-tokenizer**（独立仓库）：`consume_a_number` 返回 sign；`Numeric` 增 `has_sign: bool` + `Numeric::new()` 便捷构造；版本 **0.2.0 → 0.3.0**（加 pub 字段是破坏性变更） | 既有测试全绿 + 新增符号矩阵测试（±number/percentage/dimension、`1e+3` 指数符号不算）；fmt/clippy 干净 |
| T-2 | **css-parser / css**：依赖声明改 `0.3.0`；测试里的 `Numeric` 字面量补字段（合成的值 → `has_sign: false`） | 两仓库测试全绿；css-parser 100 例 |
| T-3 | **cascade / layout**：合成值构造点补 `has_sign: false`（共 28 处） | cascade 225 / layout 127 全绿，行为不变（无一处读该标志） |
| T-4 | **selectors**：`finish_after_n` 落实 §7 的文法——plain `n`/`-n`/`<n-dimension>` 的 B 只接受**带符号**整数；`n-`/`-n-`/`<ndash-dimension>` 与 `['+'\|'-']` 分隔符后的 B 只接受**无符号**整数；裸 `<integer>` 形式符号不限 | WPT `css/selectors/parsing` ≥99.8%；An+B 两夹具 100% 并纳入 `HARD_ASSERT_100` |
| T-5 | **验证与文档**：全链复跑（8 个 crate + workspace）、`goal.md`/`wpt-compliance`/`PROGRESS` 同步、各仓库 commit + push | 文档数字与实跑一致；commit 落盘（推送见下） |

## 规范依据（逐条核对本地 `docs/spec/css-syntax-3-Overview.bs`）

| 规则 | 规范行 |
|------|--------|
| `<signed-integer>` = integer 类型 + **有**符号字符 | §7 L3377 |
| `<signless-integer>` = integer 类型 + **无**符号字符 | §7 L3378 |
| `<n-dimension> <signed-integer>` / `'+'? n <signed-integer>` / `-n <signed-integer>` | §7 L3355-3357 |
| `<ndash-dimension> <signless-integer>` / `'+'? n- <signless>` / `-n- <signless>` | §7 L3359-3361 |
| `<n-dimension> ['+'\|'-'] <signless>` / `n` / `-n` 同样 | §7 L3363-3365 |
| `+` 与 `n` 之间不得有空白（其余 token 之间可有空白） | §7 L3371-3375（† 注） |
| `<integer>` 形式（A=0）不限制符号 | §7 L3347 + L3390-3392 |

## 显式非目标

- **`Display`/序列化带符号输出**：`Numeric::to_string()` 仍不打印 `+`（既有行为，
  改动会波及 css-parser/cssom 的序列化断言，与本轮目标无关）；`has_sign` 目前只服务
  文法匹配，已在 `types.rs` 注明。
- **发布**：本轮只做本地版本号与依赖声明（`css-tokenizer 0.3.0`）；实际发布与
  下游版本号（css-parser/css 的发布版本）由架构师的发布流程决定。
- `:has-slotted(div + div)` 那 1 例（tentative 夹具自相矛盾）**不改**——上一批已记录。

## 风险与既定裁决

- **公共 API 破坏**：`Numeric` 加字段使所有结构体字面量失效。已确认依赖声明只有
  `css-parser` / `css` 两处写死版本，其余 crate 经 css-parser/css 传递依赖，无需改
  `Cargo.toml`；字面量共 48 处，按文件机械补齐（`is_integer:` 是 `Numeric` 独有字段名，
  替换后由 rustfmt 统一缩进）。
- **不能过度收紧**：只按 §7 的三类 B 形态加约束；裸 `<integer>`、`odd`/`even`、
  `<ndashdigit-*>`（B 编码在 token 内）全部保持原样，并用 112 例 An+B 夹具回归验证。
- **推送受阻**：本轮期间 GitHub 大面积不可达（`Recv failure` / 连接超时），
  commit 全部本地落盘，推送由后台重试循环接管，恢复即推（见"完成记录"）。

## 完成记录（2026-09-13）

| # | 交付 | commit | 验证 |
|---|------|--------|------|
| T-1 | tokenizer：sign 暴露 + `Numeric::new` + 0.3.0 | css-tokenizer `aaeebd9` | 85 例（新增 `numeric_has_sign_flag`）+ 符号输入期望修正 |
| T-2 | css-parser / css 依赖声明与测试字面量 | css-parser `f0e7a56`、css `fc1621e` | 100 例 / 编译通过 |
| T-3 | cascade / layout 合成值补字段 | cascade `0f32951`、layout `a22bd84` | 225 / 127 例，行为不变 |
| T-4 | selectors：An+B 符号规则 | selectors `9371ab6` | **WPT 99.8%（507/508）**；`HARD_ASSERT_100` 增至 11 个夹具；新增 4 组符号矩阵测试（38 条断言） |
| T-5 | 文档 + 主仓库 `Cargo.lock` 版本联动 | 主仓库文档 commit | 全链复跑：css-tokenizer 85 / css-parser 100 / css-values 150 / cssom 104 / html5-parser 113 / selectors 186 / cascade 225 / layout 127 / workspace 218，全绿 |

**最终差距**：507/508。唯一失败项 `parse-has-slotted.tentative.json` 的
`:has-slotted(div + div)`——上一批已判定为 tentative 夹具自相矛盾（同文件把
`div + div` 判 valid、`div > span` 判 invalid，而 `+`/`>` 同为组合器），本实现取
一致的"参数为复合选择器"读法并保持该例失败，不做硬断言。

**推送状态**：见主仓库文档 commit 的说明与后续后台重试循环结果；
CSS 系共 6 个仓库在本轮有提交（tokenizer / css-parser / css / cascade / layout / selectors）。
