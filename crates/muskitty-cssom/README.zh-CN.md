# muskitty-cssom

[English](README.md) | [简体中文](README.zh-CN.md)

纯 Rust 实现的 CSS Object Model (CSSOM) 数据结构与序列化，遵循 [CSSOM Level 1](https://drafts.csswg.org/cssom-1/)。

[MusKitty](https://github.com/muskitty-dev) 浏览器引擎项目的一部分。

## 状态

| 组件 | 规范 | 测试 |
|------|------|------|
| CssStyleDeclaration + CssDeclaration | §8.5 / §8.6 | 10 |
| CssRule 枚举（9 变体） | §8.4 | 13 |
| CssStyleSheet 容器 | §8.1 | 5 |
| Parser → CSSOM 转换 | §8.4 / §8.6 | 20 |
| 序列化 (ToCss trait) | §3 / §8.4-§8.6 | 19+ |
| **总计** | | **81** |

- 零 `unsafe` 代码
- 零 C/C++ 依赖
- 单向转换：parser `Stylesheet` → CSSOM `CssStyleSheet`
- Rust stable，MSRV 1.82

## 安装

```toml
[dependencies]
muskitty-cssom = "0.1.0"
```

## 设计原则

1. **单向转换** — Parser 输出流入 CSSOM；CSSOM 独立存在，不引用 parser 类型。
2. **枚举优于 trait object** — CSSOM rule 类型使用枚举，值语义、pattern matching 清晰。
3. **CSSWG 是 ground truth**
4. **零 unsafe**

## 许可

Apache License, Version 2.0。详见 [LICENSE](LICENSE)。

Copyright 2026 MusCat / MusKitty Bit-Torch Community
