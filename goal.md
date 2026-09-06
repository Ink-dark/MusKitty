# Goal — WPT 测试套件补全 + 规范合规度实测轮（2026-09-06）

> **更新时间**：2026-09-06
> **当前状态**：已完成。合规度全部以 harness 实测为准（不采信文档声明），
> 数字与差距归档于 [docs/wpt-compliance-2026-09-06.md](docs/wpt-compliance-2026-09-06.md)。
> 上一轮（审计修复 F-0~F-15）已收尾推送。

---

## 当前阶段定位

- **本轮主线**：把 WPT 测试套件补全到此前零覆盖的 CSS 系 crate，刷新 html5
  夹具到 WPT 上游，并以实跑数字出具合规度报告。
- **方法**：每 crate 一个数据驱动 harness（信息性报告模式，同 html5lib 约定）；
  套件当场抓出的规范缺陷按 failing-test → 修 → 全绿 流程独立 commit。

## 任务清单（每项 = 1 个 commit，均已完成）

- [x] T-1 `[html5-parser]` tree-construction 夹具同步 WPT 上游（7 更新 +
      scripted_foster01 新增）。**退出**：62/62 文件与上游逐字节一致；套件实跑
      出新基线（1903/1924，98.9%）。
- [x] T-2 `[html5-parser]` §13.2.4 adjusted-current-node fragment 规则
      （dispatcher + CDATA 判定 + breakout reprocess 防回环）。**退出**：
      plain-text-unsafe 2 个 NUL-in-foreign 用例转绿且无回退（1905/1924，99.0%）。
- [x] T-3 `[selectors]` WPT css/selectors/parsing 套件（26 夹具 / 508 用例，
      含 css-syntax 派生 4 份）。**退出**：harness 报表 + 实测 74.8%；
      `cargo test` 全绿；fmt/clippy 零警告。
- [x] T-4 `[css-tokenizer]` WPT css/css-syntax tokenizer 套件（6 夹具 / 99 用例）
      + 三项规范修复（escaped-EOF→U+FFFD、§5.3 NULL 预处理、§4.2 ident 白名单）。
      **退出**：实测 100%；既有单测更新后全绿。
- [x] T-5 `[css-parser]` WPT css/css-syntax parser 套件（6 夹具 / 27 用例）
      + §5.5.1 @charset 丢弃修复。**退出**：实测 100%；下游 cssom 两个固化
      旧行为的单测同步更新，cssom 104/104 绿。
- [x] T-6 `[css-values]` 数值语法 WPT 用例（decimal-points + inclusive-ranges，
      16 用例）。**退出**：全绿。
- [x] T-7 `[docs]` 合规度实测报告 + goal/PROGRESS 同步。**退出**：
      docs/wpt-compliance-2026-09-06.md 落盘，数字全部来自实跑。

## 实测合规度基线（2026-09-06）

| 套件 | 通过率 |
|------|-------:|
| html5lib tree-construction | 99.0% (1905/1924) |
| html5lib tokenizer | 99.8% (7022/7036) |
| WPT css/selectors/parsing | 74.8% (380/508) |
| WPT css/css-syntax（tokenizer/parser/数值三层） | 100% (142/142) |

## 不在本轮范围（显式排除）

- selectors 128 个失败用例的修复（::part/::state/:heading/:host()/An+B 语法
  保真等）——已在报告归档，进后续轮。
- css/cssom、css/css-cascade、dom/ 主体（需 JS API 面 / layout）。
- 序列化层（selectorText / cssText / 数值序列化）落地后回填 JSON 中已记录的
  序列化断言。
