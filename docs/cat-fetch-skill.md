# cat_fetch — AI Agent 使用说明

此 skill 供 AI agent（Claude Code 等）在 MusKitty 仓库中执行多仓库管理操作时使用。

## 工具位置

```
二进制:  D:\cat_fetch\target\release\cat.exe
项目:   D:\cat_fetch\
配置:    D:\cat_fetch\repos.toml
启动器:  D:\cat_fetch\cat.ps1  (自动注入凭据)
```

## 前置条件：加载凭据

在调用任何 `cat` 命令之前，**必须先加载凭据**，否则 push/publish 会因缺 token 失败：

```powershell
. D:\cat_fetch\env.ps1
```

推荐使用启动器 `cat.ps1`，它会自动 dot-source `env.ps1`：
```powershell
D:\cat_fetch\cat.ps1 status MusKitty
```

## 命令参考

### 信息查询

```bash
# 查看所有仓库 git 状态（分支、脏工作区、ahead/behind）
cat status MusKitty --config D:\cat_fetch\repos.toml

# JSON 格式输出（便于解析）
cat status MusKitty --config D:\cat_fetch\repos.toml --json
```

### 源码获取

```bash
# 克隆所有 7 个 crate + 主仓库（幂等，已存在则跳过）
cat clone MusKitty --config D:\cat_fetch\repos.toml

# 拉取最新（fast-forward only）
cat pull MusKitty --config D:\cat_fetch\repos.toml

# 一键准备开发环境（clone + 验证 path 依赖 + pull）
cat setup MusKitty --config D:\cat_fetch\repos.toml
```

### 构建与测试

```bash
# 拓扑排序 cargo check（默认，最快）
cat build MusKitty --config D:\cat_fetch\repos.toml

# 发布模式编译
cat build MusKitty --release --config D:\cat_fetch\repos.toml

# 运行全部测试（lib + integration），输出汇总表
cat test MusKitty --config D:\cat_fetch\repos.toml

# 仅库测试
cat test MusKitty --lib-only --config D:\cat_fetch\repos.toml

# 仅某个 crate
cat build MusKitty --only muskitty-css --config D:\cat_fetch\repos.toml
cat test MusKitty --only muskitty-css --config D:\cat_fetch\repos.toml

# 格式化检查
cat fmt MusKitty --config D:\cat_fetch\repos.toml

# 自动修复格式
cat fmt MusKitty --fix --config D:\cat_fetch\repos.toml
```

### 提交与发布

```bash
# 查看哪些仓库有未推送 commit（确认前预览）
cat status MusKitty --config D:\cat_fetch\repos.toml

# 交互式推送（逐仓库确认）
cat push MusKitty --config D:\cat_fetch\repos.toml

# 跳过确认全部推送
cat push MusKitty --all --config D:\cat_fetch\repos.toml

# 发布预览（不实际发布）
cat publish MusKitty --dry-run --config D:\cat_fetch\repos.toml

# 实际发布到 crates.io（依赖拓扑顺序，幂等）
cat publish MusKitty --config D:\cat_fetch\repos.toml
```

### 脚手架

```bash
# 预览将生成的文件
cat new MusKitty css-values --deps muskitty-css-tokenizer --dry-run --config D:\cat_fetch\repos.toml

# 实际创建新 crate
cat new MusKitty css-values --deps muskitty-css-tokenizer --msrv 1.82 --config D:\cat_fetch\repos.toml
```

## AI Agent 工作流

### 场景 1：接手新任务，检查当前状态

```powershell
D:\cat_fetch\cat.ps1 status MusKitty
```

### 场景 2：改代码 → 验证 → 提交

```powershell
# 1. 先在对应的 crate 目录里改代码...
# 2. 编译检查
D:\cat_fetch\cat.ps1 build MusKitty --only muskitty-css

# 3. 格式化
cd D:\MusKitty\crates\muskitty-css && cargo fmt

# 4. 运行测试
D:\cat_fetch\cat.ps1 test MusKitty --only muskitty-css

# 5. 确认无意外改动
D:\cat_fetch\cat.ps1 status MusKitty

# 6. 提交（在对应的 crate repo 里 git add + git commit）
# 7. 推送
D:\cat_fetch\cat.ps1 push MusKitty
```

### 场景 3：新建 crate

```powershell
D:\cat_fetch\cat.ps1 new MusKitty css-values --deps muskitty-css-tokenizer --msrv 1.82
# 然后手动:
#   1. cd D:\MusKitty\crates\muskitty-css-values
#   2. git init && git remote add origin https://github.com/muskitty-dev/muskitty-css-values.git
#   3. 在 D:\cat_fetch\repos.toml 追加 [[repos]] 记录
#   4. cargo check
```

### 场景 4：发布全部 crate

```powershell
# 先预览
D:\cat_fetch\cat.ps1 publish MusKitty --dry-run

# 确认无误后正式发布
D:\cat_fetch\cat.ps1 publish MusKitty
```

## 依赖拓扑

`repos.toml` 中的 `deps` 字段定义了依赖图。`cat` 用 **Kahn 算法**自动推导构建层级：

```
Level 0 (并行):  muskitty-dom, muskitty-css-tokenizer, muskitty-html5-tokenizer
Level 1 (并行):  muskitty-css-parser, muskitty-html5-parser
Level 2:         muskitty-css
Level 3:         muskitty-selectors
```

同级可并行构建/测试；层级之间必须顺序执行。

## 凭据体系

| 环境变量 | 来源 | 用途 |
|----------|------|------|
| `CRATES_GIT_TOKEN` | Windows 凭据管理器 `git:https://github.com` | 7 个 muskitty-dev crate 仓库 |
| `MAIN_GIT_TOKEN` | Windows 凭据管理器 `gh:github.com:Ink-dark` | Ink-dark/MusKitty 主仓库 |
| `CARGO_REGISTRY_TOKEN` | `~/.cargo/credentials.toml` | crates.io 发布 |

Token **永不落磁盘**，仅存在当前进程的环境变量中。

## 约束

- 严禁在 `D:\MusKitty\` 根目录跑 `cargo check`/`cargo test`（`members = []`，零 crate）
- 每个 crate 是独立 git 仓库，commit/push 必须在 crate 目录内操作
- `cat build` 的 `--release` 会执行 `cargo build --release`，无此 flag 时仅 `cargo check`
- push 和 publish 前务必确认 token 已加载（运行 `. D:\cat_fetch\env.ps1`）
