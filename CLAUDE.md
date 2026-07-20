# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project: MusKitty

从零用 Rust 重写浏览器核心模块。独立实现，不 fork Chromium。Chromium 源码仅作参考，WHATWG 规范和 WPT 测试套件是行为 ground truth。

当前阶段：Phase 2（CSS 解析层）。HTML 解析层（tokenizer + tree construction + DOM）已完成并剥离为独立仓库；CSS Syntax tokenizer/parser/grammar hooks + Selectors Level 4 解析与匹配已完成并剥离为独立仓库；7 个 crate 全部发布到 crates.io。Layer 3 (Layout) / Layer 4 (Renderer) / Layer 5 (Network) 是远期工作。

本主仓库 (`Ink-dark/MusKitty`) 现仅作 workspace 协调中心：`members = []`，所有 7 个子 crate 列在 `exclude` 中并各自独立 git 仓库于 `muskitty-dev/` org 下。

## Build & Test Commands

主仓库 `members = []`，没有 crate 可直接 `cargo check`。每个独立 crate 在自己的目录里构建。

```bash
# 在某个独立 crate 目录下（例如 crates/muskitty-css-parser/）
cargo check                             # 检查该 crate（必须零 warning）
cargo test                              # 运行该 crate 全部测试
cargo test --lib                        # 只跑 lib tests
cargo test --tests                     # 只跑 integration tests
cargo fmt --all -- --check              # 格式检查
cargo clippy --all-targets -- -D warnings

# 跨 crate 联合构建（开发时本地 workspace 仍可解析 path 依赖）
cd D:\Muskitty\crates\muskitty-selectors && cargo test
```

依赖 path：每个 crate 的 `Cargo.toml` 用 `path = "../muskitty-xxx"` 引用同级 crate。本地开发时 `crates/` 目录下所有 crate 共存即可解析。CI 上每个仓库的 `scripts/setup-deps.sh` 负责克隆依赖到 `../` 相对路径。

## Architecture

```
MusKitty/                               # 主仓库 (Ink-dark/MusKitty)，workspace 协调中心
├── Cargo.toml                          # members = [], exclude = [7 个已剥离 crate]
├── PROGRESS.md                         # 项目进度面板
├── CLAUDE.md                           # 本文件（硬约束）
├── README.md                           # 项目 README
├── crates/                             # 每个子目录是独立 git 仓库
│   ├── muskitty-dom/                   # DOM Core (Node/Element/Text/Comment/...)
│   ├── muskitty-html5-tokenizer/        # WHATWG §13.2.5 tokenizer (80 states)
│   ├── muskitty-html5-parser/           # WHATWG §13.2.6 tree construction (23 modes)
│   ├── muskitty-css-tokenizer/          # CSS Syntax §4.3 tokenizer
│   ├── muskitty-css-parser/            # CSS Syntax §5 parser + §5.4.1/§5.4.2 grammar hooks
│   ├── muskitty-css/                    # Facade crate: 重导出 tokenizer + parser
│   ├── muskitty-selectors/             # Selectors Level 4 parser + matching engine
│   # 未来 crate 预留:
│   # muskitty-css-values/              # Phase 2 子阶段 3
│   # muskitty-cssom/                   # Phase 2 子阶段 4
│   # muskitty-cascade/                 # Phase 2 子阶段 5
│   # muskitty-network/                 # Layer 5
│   # muskitty-layout/                  # Layer 3
│   # muskitty-renderer/                # Layer 4
└── docs/
    ├── spec/                           # 规范源文件（CSS Syntax Overview.bs 等）
    └── archive/                        # 历史设计文档 / 审查报告
```

依赖拓扑（crates.io 发布顺序）：

```
muskitty-dom ───────────────────────────────────────────┐
                                                        ├─→ muskitty-selectors
muskitty-css-tokenizer ─→ muskitty-css-parser ─→ muskitty-css
                                                        ├─→ (远期) muskitty-cssom / muskitty-cascade
muskitty-html5-tokenizer ─→ muskitty-html5-parser
```

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

### Extraction Discipline (项目特有)
- 每个 crate 达到下一层入场门槛的 spec 覆盖后，剥离为独立 git 仓库（Hard extraction：crate 有自己的 `[workspace]` 块，从父 workspace `members` 移到 `exclude`）
- 主仓库 `.gitignore` 加入 `crates/<crate-name>/` 排除项
- 新仓库加 `LICENSE` (Apache-2.0) + `README.md` + `.github/workflows/ci.yml` + `.github/workflows/publish.yml` + `scripts/setup-deps.sh`
- `CARGO_REGISTRY_TOKEN` GitHub secret 通过 `gh secret set CARGO_REGISTRY_TOKEN --repo muskitty-dev/<crate>` 配置
- 发布顺序遵循依赖拓扑（先底层后上层）

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
