# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project: MusKitty

从零用 Rust 重写浏览器核心模块。独立实现，不 fork Chromium。Chromium 源码仅作参考，WHATWG 规范和 WPT 测试套件是行为 ground truth。

当前阶段：不做 UI/渲染/V8/Blink。只做纯逻辑模块。起步模块：HTML Parser。

## Build & Test Commands

```bash
cargo check                          # 检查整个 workspace（必须零 warning）
cargo check -p muskitty-html5-parser  # 只检查 html-parser crate
cargo test                           # 运行全部测试
cargo test -p muskitty-html5-parser   # 只跑 html-parser crate 测试
cargo test -p muskitty-html5-parser --lib  # 只跑 lib tests
cargo test --test html5lib_tokenizer -- --nocapture  # html5lib 套件
```

## Architecture

```
muskitty/                          (Cargo workspace root)
  Cargo.toml                       (workspace 定义)
  crates/
    muskitty-html5-parser/          # WHATWG HTML parser (tokenizer + tree construction)
      src/
        tokenizer/                 # §13.2.5 tokenization 状态机
          types.rs                 # Token, TagToken, DoctypeToken, State
          trait_def.rs             # Tokenizer trait 签名
        parser/                    # §13.2.6 tree construction（预留）
        dom/                       # DOM 节点类型（预留）
        error/                     # parse error 类型（预留）
    # 未来 crate 预留:
    # muskitty-dom/
    # muskitty-css/
    # muskitty-network/
    # muskitty-layout/
    # muskitty-renderer/
```

两阶段模型：**tokenizer** 消耗码点流，产出 token → **tree construction** 消耗 token，构建 DOM。

## Hard Rules

### Technical
- Rust stable，零 unsafe（FFI 边界需架构师批准）
- 零 C/C++ 依赖。标准库能搞定不引 crate
- 每个模块独立 crate，测试覆盖率 ≥ 80%
- 公共 API 必须有 doc comment，引用规范条款
- 参考优先级：**WHATWG > WPT > Chromium 源码**

### Behavior
1. **Read before write** — 动手前读规范对应章节 + Chromium 参考实现。不确定就问，不猜
2. **Think before code** — 先说清楚选择和取舍。真不懂就停
3. **Simplicity** — 最少代码解决问题。抵抗过早抽象。硬编码直到有真实理由需要配置
4. **Surgical changes** — diff 必须和任务一样小。不顺手改别的文件
5. **Verification** — 每个子任务先定义 success criterion。修 bug：先写 failing test → 看它 fail → 修 → 看它 pass
6. **Goal-driven** — ❌ "写个 tokenizer" ✅ "按 WHATWG §12.1 实现 Tokenizer trait，正确处理 data/rcdata/script-data 状态切换，附单元测试"
7. **Debugging** — 炸了先查，别猜。读完整报错。复现后再改，一次只改一处
8. **Self-check** — 提防：Kitchen Sink / Wrong Abstraction / Optimistic Path / Runaway Refactor

### Commit Discipline
- 每个子任务 + cargo check/test + cargo fmt 通过后立即 commit
- Message 格式：`[module] what + why`，例：`[tokenizer] add Data state, matches WHATWG §13.2.5.1`
- 必须 `git add <specific files>`，禁止 `git commit -a`
- 禁止 `git rebase -i` 压缩已完成的 commit
- WPT 语义比对通过后才允许 commit（架构师执行比对）

### Verification Flow
1. 你写完 → `cargo check` 零 warning
2. `cargo test` 全绿
3. 架构师跑语义比对（WPT 输出 vs 你的实现）
4. 比对通过 → `git add <files>` + commit
5. 比对不通过 → 根据差异修，回到步骤 1
6. 你不许自行宣布"完成"

## Style Conventions
- 别用 newtype 包裹，除非需要 orphan rule
- 别为未来需求加参数。真有需求时再加
- tokenizer 内部可多次 `clone()`，等 profiling 证明热路径以后再去掉
- 别自己写 interner——需要时用标准库类型
