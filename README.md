# MusKitty WPT 合规度实测报告

全仓库 WPT 测试实测结果的静态报告（GitHub Pages）。

- 在线报告：https://ink-dark.github.io/MusKitty/
- 生成：`python3 gen_report.py`（解析 `logs/*.log` → `index.html`）
- 数据源：各 crate 本地 `cargo test -- --nocapture` 的 harness 实跑输出

## 套件

| 套件 | crate | 通过/总 | 通过率 |
|------|-------|--------|-------|
| html5 tree-construction | muskitty-html5-parser | 1905/1924 | 99.0% |
| html5lib tokenizer | muskitty-html5-tokenizer | 7022/7036 | 99.8% |
| css/selectors/parsing | muskitty-selectors | 479/508 | 94.3% |
| css/css-syntax (tokenizer) | muskitty-css-tokenizer | 99/99 | 100% |
| css/css-syntax (parser) | muskitty-css-parser | 27/27 | 100% |
| css/css-syntax (numeric) | muskitty-css-values | 16/16 | 100% |
