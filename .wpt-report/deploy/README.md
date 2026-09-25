# MusKitty WPT 合规度实测报告

全仓库 WPT 测试实测结果的静态报告（GitHub Pages）。

- 在线报告：https://ink-dark.github.io/MusKitty/
- 生成：`python3 gen_report.py`（解析 `logs/*.log` → `index.html`）
- 数据源：各 crate 本地 `cargo test --test <harness> -- --nocapture` 的 harness 实跑输出，原始日志见 `logs/`

## 套件

| 套件 | crate | harness | 通过/总 | 通过率 |
|------|-------|---------|--------|-------|
| html5 tree-construction | muskitty-html5-parser | `html5lib_tree_construction` | 1923/1924 | 99.9% |
| html5lib tokenizer | muskitty-html5-tokenizer | `html5lib_tokenizer` | 7022/7036 | 99.8% |
| css/selectors/parsing | muskitty-selectors | `wpt_parsing` | 507/508 | 99.8% |
| css/css-syntax (tokenizer) | muskitty-css-tokenizer | `wpt_css_syntax` | 99/99 | 100% |
| css/css-syntax (parser) | muskitty-css-parser | `wpt_css_syntax` | 27/27 | 100% |
| css/css-syntax (numeric) | muskitty-css-values | `wpt_css_syntax`（硬断言夹具） | 16/16 | 100% |
| **合计** | — | — | **9594/9610** | **99.83%** |

## 复跑

```powershell
# 在各自 crate 目录下
cargo test --test html5lib_tree_construction -- --nocapture   # crates/muskitty-html5-parser
cargo test --test html5lib_tokenizer          -- --nocapture   # crates/muskitty-html5-tokenizer
cargo test --test wpt_parsing                 -- --nocapture   # crates/muskitty-selectors
cargo test --test wpt_css_syntax              -- --nocapture   # crates/muskitty-css-tokenizer / -css-parser / -css-values

# 生成报告（脚本按自身所在目录找 logs/ 与 report/）
python .wpt-report/gen_report.py
```

## 已知未达标项（16 例，均为"规范 > 夹具"保留偏差）

- **muskitty-html5-parser 1 例**：`tests_innerHTML_1.dat` #76 `<input><option>`（context
  `select`）——夹具早于 2016 年 reset 删除 select 分支，现行 WHATWG §13.2.6.15 "in select"
  规定 `input` start tag = parse error + 忽略，故 `input` 按 InBody 插入。
- **muskitty-html5-tokenizer 14 例**：`test2.test` 2 / `test3.test` 9 / `xmlViolation.test` 3，
  均为 `<?` 处理指令与 XML 违例流，按 HTML5 判定合规。
- **muskitty-selectors 1 例**：`parse-has-slotted.tentative.json` 的 `:has-slotted(div + div)`
  自相矛盾（`+` 判 valid、`>` 判 invalid），tentative 夹具。

## 变更记录

- 2026-09-25：`muskitty-html5-parser` 0.2.2 修 H-3——foreign start tag 改用 §13.2.4 的
  **adjusted current node**（原用栈顶，fragment 场景下栈顶是合成 `<html>` 根，导致
  `svg`/`math` 上下文里插入的元素落到 HTML 命名空间）。`foreign-fragment.dat` 48/66 → **66/66**，
  套件 1905/1924 → **1923/1924**，整体 99.65% → **99.83%**。
