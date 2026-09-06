# WPT 合规度实测报告（2026-09-06）

> 本轮原则：**合规度以 harness 实测为准，不采信文档声明**。所有数字来自
> 本地 `cargo test` 实跑（WPT 上游 revision
> `b89af32bc8f42d678f444eb0703bca015ddcf240`，2026-09-05 同步）。
> 复跑方式见各节 "复跑" 命令。

## 一、套件覆盖总览

| 套件 | crate | 用例 | 实测通过 | 通过率 |
|------|-------|-----:|---------:|-------:|
| WPT html/syntax/parsing（html5lib tree-construction） | muskitty-html5-parser | 1924（另 14 script-on 跳过） | 1905 | **99.0%** |
| html5lib tokenizer（WPT 镜像同源） | muskitty-html5-tokenizer | 7036 | 7022 | **99.8%** |
| WPT css/selectors/parsing（22 文件） | muskitty-selectors | 508 | 380 | **74.8%** |
| WPT css/css-syntax（tokenizer 层，6 文件） | muskitty-css-tokenizer | 99 | 99 | **100%** |
| WPT css/css-syntax（parser 层，6 文件） | muskitty-css-parser | 27 | 27 | **100%** |
| WPT css/css-syntax（数值语法，2 文件） | muskitty-css-values | 16 | 16 | **100%** |

本轮之前 CSS 系（tokenizer/parser/values/selectors/cssom/cascade）**没有任何
WPT 派生测试**，合计新增 **650 个**数据驱动用例。

## 二、本轮套件补全内容

- **selectors**：`tests/wpt_parsing.rs` + `tests/data/wpt/`（26 个 JSON）。
  移植 `css/selectors/parsing/parse-*.html` + `invalid-pseudos.html` 的
  validity 语义（`test_valid_selector` / `test_valid_forgiving_selector` /
  `test_invalid_selector` → 严格解析成功/失败），另收
  `css-syntax-*.json` 4 份（an+b 表、ident-three-code-points、escaped-eof
  选择器用例、inclusive-ranges）。序列化断言记录在 JSON 中但**不断言**
  （crate 尚无 serializer）。
- **css-tokenizer**：`tests/wpt_css_syntax.rs`，6 份夹具
  （escaped-eof、unclosed-url-at-eof、whitespace、non-ascii-codepoints、
  input-preprocessing、cdc-vs-ident-tokens），断言 token 身份/值。
- **css-parser**：`tests/wpt_css_syntax.rs`，6 份夹具（charset、
  custom-property-rule-ambiguity、at-rule-in-declaration-list、
  trailing-braces、var-with-blocks、url-whitespace-consumption），断言
  规则/声明的存活性。
- **css-values**：`tests/wpt_css_syntax.rs`，decimal-points-in-numbers +
  inclusive-ranges 的数值语法有效性。
- **html5-parser**：tree-construction 夹具与 WPT 上游同步（7 个文件更新 +
  `scripted_foster01.dat` 新增，1920 → 1938 用例）。
- harness 均为"信息性报告"模式（同 html5lib harness 约定）：逐夹具报表 +
  失败样本，`assert!` 仅保证夹具已加载，不设硬通过率门槛。

## 三、套件当场抓出并已修复的规范缺陷（5 项）

| crate | 缺陷 | 修复 |
|-------|------|------|
| html5-parser | §13.2.4 fragment 场景 adjusted current node 未实现：栈中只有合成 `<html>` 根时 dispatcher/CDATA 判定不使用 fragment context，foreign 语境下 U+0000 应插 U+FFFD 却被 in-body 规则丢弃（plain-text-unsafe 2 例） | dispatcher + `current_node_in_foreign_content` 采用 fragment 规则；breakout reprocess 加一次性跳过标志防回环 |
| css-tokenizer | §4.3.8 `\`+EOF 是合法 escape（现行为误判 Delim），经 §4.3.7 应产出 U+FFFD | `is_valid_escape_next` / `is_valid_escape_at` EOF → true |
| css-tokenizer | §5.3 预处理缺失 NULL → U+FFFD | `preprocess_input` 补映射 |
| css-tokenizer | §4.2 non-ASCII ident code point 用旧 "≥U+0080" 规则；现行规范是显式白名单（对齐 custom element 命名，U+0080–U+00B6/U+00B8–U+00BF 排除） | 新增 `is_non_ascii_ident_code_point` 白名单 |
| css-parser | §5.5.1 `@charset` 应整体丢弃，实现保留为 at-rule | stylesheet contents 丢弃 `@charset` |

连带更新 3 个固化旧行为的既有单测（css-tokenizer `backslash_at_eof`、
cssom `charset_at_rule_is_dropped` / `roundtrip_other_at_rule_statement`）。

## 四、实测差距（未修复，按 crate 归档）

### muskitty-selectors — 128 失败（74.8%）

| 类别 | 数量 | 说明 |
|------|-----:|------|
| `::part()` / `::slotted()` 功能性伪元素 | ~35 | 解析层未实现（`::part(foo)` 报 UnexpectedToken） |
| `:state()` / `:heading` / `:has-slotted()` 新伪类 | ~40 | 未实现（UnknownPseudoClass） |
| `:host()` 带参形式 | ~14 | `:host(:is(div))` 等把参数当 trailing tokens |
| An+B 语法保真（§An+B microgrammar） | ~32 | 双符号/悬空整数该拒未拒（`5n + +5`）；`+ n`/`( N- 123 )` 空白形态该收未收 |
| `:has` 无参应非法、`:has(:has)` 嵌套非法、`:not(::before)` 非法 | ~5 | 过度接受 |
| 其余 | ~2 | 零散 |

### muskitty-html5-parser — 19 失败（99.0%，另 14 script-on 跳过）

- foreign-fragment.dat 18 例：fragment foreign 语境的 end-tag 处理
  （§13.2.6.5 与 fragment 栈的交互；本轮 adjusted-current-node 修复后无回退，
  历史遗留）。
- tests_innerHTML_1 #76：select 语境夹具早于 2016 reset 删除 select 分支，
  现行 reset 无 select 分支（既有结论，非缺陷）。

### muskitty-html5-tokenizer — 14 失败（99.8%）

- xmlViolation.test 3 例（XML-only 变换，WHATWG 不要求）；
- test2/test3 共 11 例 `<?` → PI：夹具按旧 html5lib 期望 Comment，
  现行 WHATWG 定义 PI states，实现按规范正确（夹具过时）。

## 五、未移植的 WPT 套件及理由

| 套件 | 理由 |
|------|------|
| css/css-values 主体（~700 文件） | 断言经 computed value / 渲染，需 layout 引擎 |
| css/cssom / css/css-cascade | 断言经 CSSOM JS API（cssText、insertRule、computed）；本地无对应 API 面。serialize-escape-identifiers / serialize-consecutive-tokens 需 cssText 与 var() 替换，cssom 的 `serialize_component_values` 具备雏形，后续可接 |
| dom/ 主体 | JS DOM API 面（EventTarget 语义可后续按 dom/events 子集移植） |
| css/css-syntax 其余文件 | anb-serialization（无序列化器）、urange-parsing（属性级语法，无属性 DB）、missing-semicolon（reftest）、charset/（编码面） |
| custom-property-rule-ambiguity 嵌套两例 | 需 CSSNestedDeclarations 物化 |

## 六、复跑命令

```bash
cd crates/muskitty-html5-parser && cargo test --test html5lib_tree_construction -- --nocapture
cd crates/muskitty-html5-tokenizer && cargo test --test html5lib_tokenizer -- --nocapture
cd crates/muskitty-selectors && cargo test --test wpt_parsing -- --nocapture
cd crates/muskitty-css-tokenizer && cargo test --test wpt_css_syntax -- --nocapture
cd crates/muskitty-css-parser && cargo test --test wpt_css_syntax -- --nocapture
cd crates/muskitty-css-values && cargo test --test wpt_css_syntax -- --nocapture
```

夹具出处与提取方式：`crates/muskitty-selectors/tests/data/wpt/README.md`；
各 JSON 内 `source`/`note` 字段记录逐文件映射与未移植断言。
