# JS 引擎接入评估：Boa（嵌入式）与 rusty_v8（crate 打包为动态库 + IPC 挂载插件）

> **日期**：2026-09-18（2026-09-19 修订：修正方案 B 的表述与结论）
> **状态**：调研完成（评估文档，未实施；本文件不含代码改动）
> **问题**：MusKitty 是否应接入 JS 引擎？两条候选路线的实现可能性、性能代价与最终受益各是什么？
> **方案 B 的精确定义（2026-09-19 修订）**：**把 `v8`（rusty_v8）这一个 Rust crate 整体编译为动态库产物**，宿主对其做**动态库链接**，用于**挂载插件**；**内部实现走 Unix IPC**（不是"把 V8 上游改造成 component build 再 dlopen 单体 v8.dll"——初版文档误按后者评估，本版已纠正）。
> **前提（用户给定）**：① 不在本机从源码编译 V8（机器扛不住），需要编译就走 GitHub 仓库的 CI；② 许可无特别约束，只要不引入传染类许可证（GPL/AGPL/LGPL 等）；③ 网络层 C ABI 草案**作为"网络层的接口方案"已作废**（网络层现在直接用 reqwest），故动态库 FFI 在本仓库**没有在跑的实例**——但该草案的 **ABI 范式**对本方案的插件边界仍然适用，见 §3.8。
> **方法**：外部一手资料（crates.io API / docs.rs / 上游 GitHub issue、PR、README / V8 与 Chromium 官方文档）+ 本机实测（Boa 实际编译运行、**Rust 动态库四种形态逐一编译加载实验**、IPC 往返实测、工具链与网络探测）。凡未经实测的数字均标注来源；凡推断均标注"推断"。
>
> **本版修订要点**：方案 B 补入 §3.1（Rust ABI 与 C ABI 两条路的**本机实测对比**）、§3.2（V8 在 Windows/Linux 的动态库支持差异，含"必须走 CI 构建"的确切原因）、§3.3（多副本 V8 问题与挂载插件的正确形态）；新增 §3.6（Windows 上 Unix IPC 的平台现实）；**重写 §3.8**——澄清 `network-c-abi-draft.md` 是**跨动态库边界的 ABI 范式规范**（网络层只是其首个候选消费者），其设计原则在方案 B 中直接适用，并列出 V8 场景需增补的五项契约。

---

## 0. 结论速览

| | 方案 A：Boa 嵌入式（进程内） | 方案 B：`v8` crate → 动态库 + IPC 挂载插件 |
|---|---|---|
| **实现可能性** | ✅ 技术可行，本机已实证构建成功 | ✅ **可行**，但**必须走 C ABI（`cdylib`）**；Rust ABI（`dylib`）实测不可取（§3.1）。**Windows 上需要自建 CI 编译 V8**（§3.2 / §3.5） |
| **性能代价** | 与**禁用 JIT 的** V8 差 **11.9 倍**（Boa 官方基准；对生产级 V8 的差距更大） | 动态库**调用**本身开销可忽略；代价在**装载/初始化**与**跨进程 IPC**。实测往返 **66 µs**（文献 11–31 µs）→ 粗粒度可行，**细粒度 DOM 访问过 IPC 不可行**（§3.4） |
| **最终受益** | 保住"纯 Rust / 零 C-C++ 依赖"的项目定位；无 FFI、无进程边界 | 唯一能拿到真实 web 兼容性的路线（V8 97.6% test262）；**一份 V8 供多方复用**（§3.3 是正确形态），并可换取插件崩溃隔离 |
| **硬约束冲突** | 无冲突（零 C/C++ 依赖 ✅）；但 MSRV 从 1.82 抬到 **1.91** | **同时打破两条项目硬规则**：零 C/C++ 依赖 ❌、README "no V8" 定位 ❌；FFI 需架构师批准 |
| **当前建议** | 作为纯 Rust 路线的**长期锚点**，先做 spike；**不作为今天的页面 JS 引擎** | **架构正确、但今天不必开工**。若做，只用于**插件/扩展**，且必须是"一份 V8 + 插件走 ABI/IPC"，**绝不能让每个插件各带一份 V8**（§3.3） |

**一句话**：Boa 现在能跑但没有 DOM（引擎层面锁死，issue #5513 明言"无法在 Boa 之上构建 DOM"），性能还差一个数量级；**把 `v8` crate 打包成动态库这条思路本身是成立的，而且 Deno 已在 Windows 上生产验证（`libdenort`）**——但要点在于：必须用 **C ABI 而非 Rust ABI**、Windows 上**必须自己编一次 V8**（这正是"开 GitHub 仓库编译"的用处）、以及**只能加载一份 V8 供所有插件共享**。**两个方案今天都不该直接开工；真正该先做的是与引擎无关的宿主层（脚本执行时机 / 事件循环 / DOM 绑定接口）。**

---

## 1. 现状基线：MusKitty 今天离 JS 有多远

先把"接入 JS 引擎"这件事在 MusKitty 里的实际含义量出来，否则评估会失焦。

| 现状 | 证据 |
|---|---|
| **完全没有 JS 执行** | 解析器 `scripting flag` 恒为关闭（`crates/muskitty-html5-parser/src/lib.rs:239-240` 注释明示 "our parser runs with scripting disabled，所以 noscript 留在 Data"） |
| `javascript:` URL 被显式拒绝 | `crates/muskitty-chrome/src/navigation.rs:124`（scheme 黑名单）+ `navigation.rs:396-397` 单测断言其归入 `Unsupported` |
| `<script>` 只被当作文本 | tokenizer 有 ScriptData 状态（`lib.rs:237`）——那是**分词正确性**需要，不是执行；`<script>` 内容不产生任何副作用 |
| 项目公开定位 | `README.md:35` 明确写着 "JavaScript engine (no V8, no Blink)" 属**有意不做**范围 |
| 渲染管线形态 | HTML→DOM→CSS→Layout→Render **同步、单线程、一次性**（`docs/decisions/2026-08-29-chrome-window-layer.md`）；没有事件循环，也没有"页面可变"的概念 |

**关键含义**：接入 JS 不是"加一个引擎依赖"，而是新增一整个**宿主层**：

1. **脚本执行时机**（HTML §4.12：`<script>` 同步阻塞解析 / `defer` / `async` / 动态插入）
2. **事件循环 + 微任务队列**（否则 Promise 永不结算、`setTimeout` 无宿主）
3. **DOM 绑定层**（JS 对象 ↔ DOM 节点，属性/方法/事件；这是工程量最大的部分，通常需要代码生成）
4. **渲染与脚本的交互**（脚本改 DOM → 重排重绘的失效传播；当前管线是一次性快照）

这四件事**与选哪个引擎无关**。先做它们，引擎可以后换——这是本评估最重要的结论（见 §8）。

---

## 2. 方案 A：Boa 嵌入式

### 2.1 事实核查

| 项 | 值 | 来源 |
|---|---|---|
| 最新版本 | `boa_engine` **0.22.0**（2026-08-28 发布，上一个 0.21.1 是 2026-03-29） | crates.io API |
| 维护活跃度 | 活跃。`main` 最后提交 2026-09-16，近 90 天 74 次提交；有每日基准与 test262 数据仓库 | GitHub API / `boa-dev/data` |
| 许可 | **Unlicense OR MIT**（全部第一方子 crate 同） | crates.io / docs.rs |
| MSRV | **rust-version = 1.91.0**，edition 2024 | 已发布 Cargo.toml |
| test262 | 发布标签实测 **51,225 / 53,578 = 95.60%**；`main` 分支 **95.99%** | `boa-dev/data` 原始 JSON；CI bot 注释 |
| 第三方横比（同日同套件） | Boa **95.59%** / V8 97.59% / SpiderMonkey 98.45% / JavaScriptCore 98.79% / QuickJS 82.12% | test262.fyi 数据 |
| 性能 | 官方基准（V8 v7 套件，对照引擎**均为禁用 JIT 配置**）：Boa 综合分 **213**，v8-jitless **2,532（11.9 倍）**，sm-jitless 784（3.7 倍），QuickJS 1,221（5.7 倍） | `boa-dev/data` bench 原始 JSON |
| 单基准极端值 | RegExp：Boa 43 vs v8-jitless 3,973 → **92 倍**；EarleyBoyer 18.9 倍 | 同上 |
| 架构 | 字节码解释器 + 内联缓存，**无 JIT**（上游路线图把性能工作放在 VM/数据结构，不是 JIT） | 上游 roadmap / issue #5522 |
| GC | 自研 mark-sweep（`boa_gc`），官方路线图列出"Garbage Collector Redesign"，自述为 **prototype**、需重大重构 | 上游 roadmap |
| 线程模型 | `Context`、`JsValue`、`Gc<T>` **全部 `!Send + !Sync`**（基于 Rc/RefCell）；每 context 单线程 | docs.rs 自动 trait 列表 |
| 许可传染性 | 抽样 ~50 个传递依赖（共约 159 个非 dev），**全为宽松许可**（MIT / Apache-2.0 / Unicode-3.0 / Zlib / BSD-3 等），**未见 GPL/AGPL/LGPL/MPL** | 逐个 crates.io 元数据（抽样，非全量审计） |

### 2.2 本机实测（本次评估的硬数据）

探针工程：`D:\tmp\boa-probe`（`boa_engine = "0.22"`，默认 features，release 构建）。环境：Windows 10 x64、8 核、rustc 1.98.1。

| 指标 | 实测值 | 说明 |
|---|---|---|
| **是否引入 C/C++ 编译** | **零**——`cargo tree` 中**不存在 `cc` 包**，无任何 `-sys` crate、无 `bindgen`/`cmake`；构建日志中 C/C++ 编译器调用次数 **0** | 直接印证 §2.1 的"纯 Rust"结论 |
| 传递依赖规模 | 构建日志实测 **144** 个第三方 crate（`cargo tree -e normal` 去重 140 个） | |
| 完整构建耗时 | **11 分 05 秒**（冷启动，含下载，8 核 release） | 对增量开发的日常影响可接受 |
| 构建产物占用 | `target/` **459 MB** | |
| 单文件二进制 | **13.7 MB**（release，含 temporal/ICU 相关默认 feature） | 对照：Boa 官方 CLI 分发版 32–33 MB——那是含 intl+fetch+reqwest+rustls 的超集 |
| `Context::default()` | **2.06 ms** | 引擎初始化（内建编译） |
| 首次 `eval` | **1.69 ms** | |
| 热态 `eval("1+1")` | **6.97 µs** | 同 context 重复执行 |
| 第二个 `Context` | **745 µs** | 多 realm / 多插件场景的边际成本 |
| 进程冷启动 | **196–297 ms**（8 次采样） | 含进程创建 + ICU 数据初始化 |

功能验证：`const xs=[1,2,3]; xs.map(x=>x*2).join(',')` → `"2,4,6"` ✅。

### 2.3 阻断性问题

**① DOM 无法构建（决定性）**

上游 issue **#5513**（2026-09-06 开启）原文：

> "The Boa engine doesn't allow external crates to create 'exotic' objects. This mechanism is locked down (`pub(crate)`)... **Without it, it's impossible to build a DOM on top of Boa**: DOM collections like NodeList or HTMLCollection require indexed and named properties. Proxy isn't a good fit."

这不是"还没做"，而是**引擎的公开 API 从设计上不允许宿主构造 DOM 所需的奇异对象**。对浏览器而言，一个不能挂 DOM 的 JS 引擎只能做与页面无关的脚本执行（配置、测试、纯计算），**无法承载页面 JS**。

**② 性能差一个数量级**

11.9 倍是与**禁用 JIT 的** V8 比。生产 V8 开着 JIT，差距会进一步放大（**推断**：按此类基准的常见比例，真实差距可能落在数十倍量级——此数为推断，非实测）。后果很具体：一个中量级 SPA 的脚本时间会从数十毫秒变成数百毫秒到秒级，页面在用户感知上"卡死"。

**③ 0.x 语义化版本频繁破坏**

13 个已发布版本全部 0.x，横跨 9 条 minor 线；MSRV 在最近一个小版本内从 1.88 跳到 1.91。上游有 `cargo-semver-checks` 约束"意外破坏"，但**不阻止 0.x 的刻意破坏**；1.0 尚未发布（公开 API 审计 issue #4524 仍开启）。例：0.21 把 `JobQueue` 改名为 `JobExecutor` 且方法签名改为接收 `Rc<Self>`。升级需要跟随改动。

**④ MSRV 与项目冲突**

MusKitty 各 crate 声明的 `rust-version` 为 1.82（多数）/ 1.70（dom、两个 tokenizer）/ 1.85（layout），CI 有专门的 MSRV 1.82 任务。Boa 要求 **1.91**。引入 Boa 意味着承载它的 crate（如 chrome）MSRV 抬到 1.91，或把 Boa 隔离在单独的、不参与 MSRV 门禁的 crate 里。

**⑤ `unsafe` 面（次要，但需记录）**

Boa 自身：`boa_engine` 409 行含 `unsafe`（0.27%），`boa_gc` 183 行（4.25%）；有一个**未关闭的安全性抱怨 issue #5392**。对 MusKitty 的"零 unsafe"规则而言，**依赖内部的 unsafe 不算违规**（规则约束的是本仓库代码），但使用者要留意：注册带捕获的原生函数时 `NativeFunction::from_closure` 是 `unsafe fn`，安全替代品要求捕获为 `Copy`/`Trace`——这会约束 DOM 对象的持状态方式。

### 2.4 方案 A 小结

- **可行性**：技术可行，本机 11 分钟建成，零 C/C++ 依赖，"纯 Rust"定位完好。
- **今天不能用它跑页面 JS**：DOM 锁死（#5513）+ 性能差一个数量级。
- **可成立的用途**：与页面无关的脚本能力（例如：内置自动化/测试脚本、`--js` 计算、未来的扩展脚本"轻模式"），且必须 feature-gate，避免污染核心。
- **观察触发条件**：#5513 关闭并落地"允许外部构造奇异对象"的 API + GC 重设计完成 + 综合基准相对 v8-jitless 差距收窄到 3 倍以内。

---

## 3. 方案 B：把 `v8` crate 打包成动态库 + IPC 挂载插件

### 3.1 两条"动态库"路线：本机实测对比

"把 `v8` 这个 Rust crate 打包成动态库"在 Rust 里有**两种完全不同的产物**，必须分清——它们的可行性差别是决定性的。本机为此建了三 crate 探针（`D:\tmp\dylib-probe`：`engine` 出动态库、`host` 消费、`plug` 插件），逐条实测：

| 实验 | 命令/配置 | 结果 |
|---|---|---|
| **① Rust ABI 动态库**（`crate-type = ["dylib"]`）被普通宿主链接 | `cargo build -p host` | ❌ **链接失败**：`cannot satisfy dependencies so 'std' only shows up once`（连带 `core`/`alloc`/`compiler_builtins`/`hashbrown` 等 13 处）——Rust 要求**整条依赖图**统一为动态格式 |
| **② 同上 + 全图动态** | `RUSTFLAGS="-C prefer-dynamic" cargo build` | ✅ 编译通过；`host.exe` 仅 **14 KB**、`engine.dll` **22 KB**（真正动态） |
| **③ 验证它确实是运行时动态链接** | 删除 `engine.dll` 后运行 | ✅ **exit=127**（加载失败）；放回后正常输出 `engine(host) = 42` |
| **④ 运行时到底需要什么**（PE 导入表） | 解析 `host.exe`/`engine.dll` | ⚠️ `host.exe` 依赖 **`engine.dll` + `std-44a584f44bc3dd65.dll`**——那个 `std` 库名**带工具链哈希** |
| **⑤ C ABI 动态库**（`crate-type = ["cdylib"]`） | `cargo build`（无特殊 flag） | ✅ 正常构建，产出 `plug.dll` + **`plug.dll.lib`**（导入库） |
| **⑥ cdylib 的导入表** | 解析 `plug.dll` | ✅ 只有 `KERNEL32` / `VCRUNTIME140` / `api-ms-win-crt-*`——**不含 `std-*.dll`**，自包含 |
| **⑦ 运行时加载（挂载插件的真实形态）** | 裸 FFI 调 `LoadLibraryW` + `GetProcAddress` | ✅ 成功：`plug_answer() = 7`；无需导入库 |

**结论（Rust 层，与引擎无关）**：

> **Rust ABI（`dylib`）这条路工程上不可取**——它要求整个依赖图（含 `std`）都以动态格式提供，产物对外依赖**哈希命名的 `std-<hash>.dll`**。宿主与插件**必须用完全相同的 rustc + 完全相同的 crate 版本**才能对上；对"插件由第三方构建"的生态模型，这是致命约束。Rust 官方对 `dylib` 的定位本就是"编译器内部使用"，不是稳定的插件 ABI。
>
> **C ABI（`cdylib`）是唯一现实形态**——自包含（不依赖 Rust std 动态库）、可用 `LoadLibrary`/`dlopen` 运行时加载、跨语言可调用。**"挂载插件"必须走这条**，而且要在边界上只传 C 兼容类型（不透明指针 / `uint8_t*` + `size_t`）——**这正是 `docs/network-c-abi-draft.md` 所规范的那套范式**：作废的是"网络层拿它当接口"，不是这套 ABI 本身。详见 §3.8。

### 3.2 V8 特有的支持差异：Linux 已支持，Windows 需要自己编

把 `v8` crate 做成 cdylib，**技术上就是"把预编译静态库 `rusty_v8*.lib` 链进一个 `cdylib`"**。但 V8 有一个平台差异必须先解决：

| 项 | 值 | 来源 |
|---|---|---|
| 最新版本 | `v8` crate **152.2.0**（2026-08-20） | crates.io API |
| 许可 | crate：**MIT**；V8 本体：**BSD-3-Clause** | 上游 LICENSE |
| 预编译产物形态 | **静态库**：非 Windows 为 `librusty_v8*.a`，Windows 为 `rusty_v8*.lib`。README："We publish **static libs** for every version" | 上游 README / `build.rs` |
| 预编译体积 | Windows x86_64-msvc release **38.1 MB**（`.lib.gz`）；Linux/mac 约 37–40 MB | release 资产清单（本次实测拉取） |
| **link 进 cdylib 的关键 GN 开关** | `v8_monolithic_for_shared_library` → 定义 `V8_TLS_USED_IN_LIBRARY` → 把 V8 的线程局部存储从 `initial-exec`/`local-exec` 切到 **`local-dynamic` 且改为 noinline 访问器** | V8 `BUILD.gn`（`v8_monolithic && v8_monolithic_for_shared_library`）+ `src/common/thread-local-storage.h` |
| **Linux** | ✅ **已支持**：rusty_v8 `build.rs` 在 `target_os == "linux"` 时**自动注入**该 GN 参数；README 明示 Linux 预编译 release 归档已按 shared-library-safe TLS 模式构建（PR #2008/#2009，2026-06-10 合并，v149.4.0 起） | 上游 `build.rs` / README / PR #2008 |
| **Windows** | ⚠️ **无对应支持**：`build.rs` 的注入条件是 `target_os == "linux"`，**不含 Windows**；Windows 预编译 `.lib` 用 V8 默认 TLS 模式。上游**没有任何**关于 Windows cdylib/DLL 的 issue 或 PR | 上游 `build.rs` / 资产清单 / issue 检索 |
| Windows 为何不会撞 Linux 那个链接错误 | Linux 的失败是 ELF 重定位 `R_X86_64_TPOFF32 ... cannot be used with -shared`（lld 拒绝）；**Windows/COFF 没有这个错误**，MSVC 下 DLL 内的 initial-exec TLS 自 Vista 起合法。所以"Windows 能行"**合理但未证实** | V8 TLS 头注释（含 Windows 行）+ MS 文档 |
| 上游对卸载的立场 | **不支持卸载**。维护者答（issue #1589）："unloading dynamic libraries is generally not supported. you can simply exit the process after deinitializing V8." → 引擎库应**常驻到进程结束** | 上游 issue #1589 |
| 允许链接外部 libv8？ | ❌ 维护者明确劝阻（issue #411）：除非版本**精确一致**，否则"prone to crash, behave erratically" | 上游 issue #411 |

**这正好解释了"为什么要开一个 GitHub 仓库编译"**：

- 若走 **Linux**：直接用官方预编译库，`build.rs` 已帮你注入正确 GN 参数，**不必自己编**。
- 若走 **Windows**：官方预编译 `.lib` **不是** shared-library-safe 模式建的，上游也不提供该配置的制品 → **要稳妥，就得自己在 CI 里 `V8_FROM_SOURCE=1 GN_ARGS='v8_monolithic=true v8_monolithic_for_shared_library=true'` 编一次**。这就是"开 GitHub 仓库编译"的准确用途与必要性。

**一个有利的实测发现**：`v8` crate 的 `.crate` 包**自带完整 V8 源码**——本次解包实测：`v8/` 目录 **79 MB / 3780 个文件**（含 `src`、`include`、`gni`、`BUILD.gn`），外层 `third_party/` **109 MB**（abseil-cpp、libc++、libc++abi、libunwind、icu、simdutf、partition_alloc、highway 等），另有 `buildtools/`。**因此 CI 不需要先 `gclient sync` 拉取完整的 Chromium/V8 仓库**（Chromium 文档的"≥100 GB 可用空间"是针对完整 Chromium 检出 + 构建的），构建脚本会按需自动下载 `gn`/`ninja`/`clang`。这显著降低了"自建编译仓库"的门槛——但仍需一个足够大的 runner 构建目录（见 §3.5）。

**旁证：这条路在生产环境跑通过**。Deno 的桌面运行时把**整个 deno_core + deno_runtime + v8** 编成 `crate-type = ["cdylib"]` 的 `libdenort`，由宿主 `dlopen` 加载并通过 C ABI 调用，**支持的目标包含 `x86_64-pc-windows-msvc`**（Deno `doc/desktop-architecture.md` / `cli/rt_desktop/`）。另有 s2script（Metamod 插件）、pg_typescript（Postgres 扩展）、aardvark 等项目在 Linux 上以 `v8_monolithic_for_shared_library=true` 把 rusty_v8 链进 `.so`。**但"发布一个可复用的独立 `v8.dll` 供插件各自链接"这样的项目，本次调研未找到先例。**

### 3.3 挂载插件的正确形态：一份 V8，不是每插件一份

这是方案 B 最容易踩的坑。若"挂载插件"被实现成**每个插件动态库各自静态包含一份 V8**，一个进程里就会有 N 份 V8 副本。V8 源码对此的态度很明确：

| 冲突点 | 事实 | 来源 |
|---|---|---|
| **Sandbox 唯一性** | V8 源码明写：sandbox"**of which there can currently be only one per process**"，因为它需要大片虚拟地址空间（典型 **1 TB**） | `src/init/isolate-group.h`、`docs/sandbox/architecture.md` |
| **指针压缩 cage** | 每个 isolate 有自己的 **4 GB** cage；共享 cage 模式下一个进程共用一个 4 GB。N 份副本 = N 套 cage | `include/v8-internal.h`、`docs/heap/pointer-compression.md` |
| **Platform / 线程池** | 每个副本各有自己的 `Platform` 与 worker 线程池（`NewDefaultPlatform`）；副本间不协调 | `include/libplatform/libplatform.h` |
| **单例检查是"每副本"而非"每进程"** | `V8::InitializePlatform` 的 `CHECK(!platform_)` 是**文件级静态变量**——两份副本**各自通过检查**，谁也拦不住谁。这是陷阱：加载器不会阻止你，但每个副本都以为自己是本进程唯一的引擎 | `src/init/v8.cc` |
| **进程级资源争用** | Wasm trap handler 走进程级信号/VEH 链（各副本各装一个 handler）；PKU 保护键是进程级分配 + 每线程继承 | V8 trap-handler / rusty_v8 `V8.rs` 文档 |
| **符号冲突** | ELF 上有符号介入风险（Deno 需要 `-Wl,--exclude-libs,ALL` 隐藏静态链入的 libc++，PR #35424）；rusty_v8 有未合并的 PR #1844 讨论 Abseil 符号冲突导致"runtime crashes"。Windows 的 DLL 按模块解析符号，此类风险较低 | Deno PR #35424 / rusty_v8 PR #1844 |
| **卸载** | 即使只有一份也不能安全卸载（#1589）→ 插件宿主里副本只会越积越多 | 上游 #1589 |

**V8 官方支持的多实例模型是"一份引擎拷贝里的多个 isolate（或 IsolateGroup）"，不是"多份引擎拷贝"**：

> `include/v8-isolate.h`：**"Isolate represents an isolated instance of the V8 engine... The embedder can create multiple isolates and use them in parallel in multiple threads."**
> `src/init/isolate-group.h`：Isolate group 是"用户声明哪些 isolate 应共处同一指针 cage"的机制。

**因此挂载插件的架构只有两种正确形态**：

| 正确形态 | 描述 | 代价 |
|---|---|---|
| **① 一份 V8 动态库 + 插件走 C ABI** | 宿主加载**唯一的** V8 cdylib；插件（`cdylib`）通过该库暴露的稳定 C ABI 使用 JS 能力；插件本身**不含 V8**。这是 **Node-API 的模型**（"ABI stable across versions of Node.js"，一个引擎、多个 addon） | 需要自建一套稳定的 JS 宿主 ABI；插件与 JS 交互的每次调用都过 ABI（同进程，开销可忽略） |
| **② 每个插件一个进程 + 进程内一份 V8** | 插件的 JS 在**独立 OS 进程**里跑，该进程内静态链入 V8；宿主 ↔ 插件进程走 IPC | 进程启动/初始化成本；跨边界调用必须是**粗粒度**的（§3.4）；换来真正的崩溃隔离与沙箱位 |

**明确不可取的组合**：同一进程里加载 N 份 V8 副本（N 个插件各带一份）。它同时踩中 sandbox 唯一性、cage 复制、线程池复制、卸载不可行，且**上游从未声明支持**（本次调研未找到任何"两份 V8 共存"的官方说明，也未找到明确因此崩溃的公开报告——属"无文档、无管理、只在全隔离且永不卸载的窄条件下可能侥幸可用"）。

同理，"用 IPC 把 V8 与 DOM 分离"也不可取——DOM 访问是细粒度的，见 §3.4。

### 3.4 IPC 代价（本机实测 + 文献）

实测环境同 §2.2，探针 `D:\tmp\boa-probe\src\bin\ipcbench.rs`，TCP loopback + `nodelay`，ping-pong 往返 20,000 次取均值：

| 载荷 | 往返延迟（本机实测） |
|---|---|
| 64 B | **66.1 µs** |
| 256 B | 76.0 µs |
| 1 KB | 79.2 µs |
| 16 KB | 100.5 µs |

按下（**60 fps = 16,667 µs/帧**）折算：

| 每帧跨边界往返次数 | 占帧预算 |
|---|---|
| 1 次 | 0.40% |
| 10 次 | 3.96% |
| 100 次 | **39.6%** |
| 1000 次 | **396%**（≈66 ms/帧 → 约 15 fps 上限） |

文献交叉验证（同为小载荷 ping-pong，中位数 RTT）：

| 机制 | Windows | 来源 |
|---|---|---|
| 共享内存（非内核中介） | 0.10–0.30 µs | ipc-shootout / ipc-bench（社区基准） |
| 命名管道 | **11–27 µs** | smithtrenton/ipc-bench（Win11，绑核）；ipc-shootout CI |
| AF_UNIX / Unix socket | **18–31 µs** | 同上 |
| TCP loopback | 40 µs | ipc-shootout CI |
| Mojo（Chromium） | 无公开绝对值；官方仅称"比旧 IPC 快约 1/3、上下文切换少 1/3" | Chromium Mojo README |

**架构含义（本评估最关键的一条）**：

浏览器里 JS 对 DOM 的访问是**极细粒度**的——一次 `document.getElementById(...)`、一次 `.textContent = ...`、一次 `addEventListener` 都是独立调用。真实页面里每帧成百上千次 DOM 触碰是常态。**若把 V8 与 DOM 分置两个进程，这些触碰全部变成 IPC 往返 → 1000 次往返 = 66 ms/帧 → 页面彻底不可用。**

这正是 Chromium 的做法**不是**分离 JS 与 DOM，而是把 **V8 + Blink（DOM）+ 排版**放在同一个渲染进程里，**只在浏览器进程 ↔ 渲染进程之间**做 IPC（边界粗、消息少）。所以：

> **IPC 边界只能画在"粗粒度"处**（一次导航、一次脚本整段执行、一批 DOM 变更、一次插件调用），**不能画在"每个 DOM 操作"处**。
> 因此："用 IPC 把 V8 与页面分离"是不可行的架构；"用 IPC 把**插件/扩展**与浏览器分离"是可行且有益的架构。

顺带说明：共享内存虽快 2–3 个数量级，但代价是要么忙等（实测占用约 2 个核，且无空闲核时退化到毫秒级），要么自建同步协议——复杂度高，且不能把细粒度 DOM 访问变成可行，只能把"批量传输"做得更快。

### 3.5 构建与分发代价（对应"开 GitHub 仓库编译"的既定前提）

**先说结论（已据实测修订）**：标准免费 runner **做不了完整 Chromium 检出**（那需要 ≥100 GB），但 **`v8` crate 自带 V8 源码**（§3.2 实测：`v8/` 79 MB + `third_party/` 109 MB），**不需要 `gclient sync` 拉完整仓库**。因此"在 GitHub Actions 里自建 V8"的门槛比初版评估低得多——真正需要的是**足够的构建磁盘与时间**，而非 100 GB 检出。

| 事实 | 值 | 来源 |
|---|---|---|
| **完整** Chromium/V8 检出的官方磁盘要求 | **≥100 GB 可用空间**（Linux 文档另需在 HDD 留 50–80 GB 构建产物）；≥8 GB RAM（建议 >16 GB） | Chromium `windows_build_instructions.md` / `linux/build_instructions.md` |
| **`v8` crate 自带的源码体积** | `v8/` **79 MB / 3780 文件** + `third_party/` **109 MB** + `buildtools/` 179 KB（**本次解包实测**） | 本机解包 `v8-152.2.0.crate` |
| 源码构建开关与工具 | `V8_FROM_SOURCE=1`；需 Python3（`python3`）、curl、**libclang 21.1+**（bindgen）；`gn`/`ninja`/`clang` 缺失时自动下载；可用 `GN_ARGS` 传自定义参数 | 上游 README |
| 标准 GitHub 托管 runner（公开仓库，免费） | 4 vCPU / 16 GB RAM / **14 GB SSD** | GitHub 官方文档 |
| 标准 runner（私有仓库） | 2 vCPU / 8 GB RAM / 14 GB SSD | 同上 |
| 大 runner（75 GB – 2040 GB SSD 档位） | **仅限 GitHub Team / Enterprise Cloud 的组织与企业**；**始终按分钟计费** | GitHub 官方文档原文 |
| 上游自己怎么构建 | rusty_v8 的 CI 用 **`windows-2022-xxl` / `ubuntu-22.04-xl`** 等大 runner，`V8_FROM_SOURCE: true` | 上游 `.github/workflows/ci.yml` |

**Windows 上自建 V8 的三条出路**：

1. **付费大 runner**（Team/Enterprise + 按分钟计费）：选 ≥150 GB SSD 档位，跑
   `V8_FROM_SOURCE=1 GN_ARGS='v8_monolithic=true v8_monolithic_for_shared_library=true' cargo build`，
   产出 shared-library-safe 的 `.lib`，之后交给各下游仓库消费（`RUSTY_V8_ARCHIVE` 指向制品）。
2. **自托管 runner**（自有 ≥100 GB 磁盘的机器）挂到仓库——但你已明确本机不编译 V8。
3. **不接受"未证实"**：直接用官方 Windows 预编译 `.lib` 链 cdylib。COFF 没有 ELF 那个链接错误（§3.2），**可能可行，但上游无任何 Windows cdylib 的先例或声明**——属"赌一把"。

**Linux 则不需要自建**：官方预编译归档已按 shared-library-safe TLS 模式构建，`build.rs` 自动注入正确 GN 参数，直接 `cargo build` 即可产出可用 cdylib。

**分发代价**：V8 动态库每平台约 **+38 MB**（release）；Windows **无 debug 预编译**，debug 构建只能走源码构建（落到上面的 runner 问题）。

### 3.6 Windows 上"走 Unix IPC"的平台现实

方案表述里的"内部实现走 Unix IPC"在 Windows 上需要注意三点（V8 与 Rust 都受影响）：

| 项 | 事实 | 来源 |
|---|---|---|
| AF_UNIX 可用性 | Windows **10 build 17063**（Insider）引入，**1809（October 2018 Update）/Server 2019** 起正式可用；需 `<afunix.h>` + Winsock | Microsoft DevBlogs / MS Learn / `uds_windows` README |
| AF_UNIX 限制 | 仅 `SOCK_STREAM`（无 DGRAM/SEQPACKET）、**无辅助数据**（不能传 fd/凭据）、**无 socketpair**、无抽象地址、绑定前需先 `DeleteFile` 旧路径；`sun_path` 上限 108 字节 | Microsoft DevBlogs |
| 已知隐患 | 阻塞式 `WSARecv` 在**对端关闭/重置时可能不返回**（OpenJDK 复现，MS 内部立案但无公开结论）→ 必须用 IOCP/overlapped 异步 I/O，别用阻塞读 | MS Q&A #700171 |
| Rust std 支持 | `std::os::windows::net::{UnixStream, UnixListener}` **仍是 nightly 实验 API**（`windows_unix_domain_sockets`），稳定版不可用 | Rust std 文档 |
| Tokio 支持 | `tokio::net::UnixStream` **仅 Unix**；Windows 对应物是 `tokio::net::windows::named_pipe::{NamedPipeServer, NamedPipeClient}`（原生 overlapped、支持消息模式） | docs.rs |
| 实际选择 | Windows 上用**命名管道**（首选，Tokio 原生支持）；或 `uds_windows` / `interprocess` crate 跨平台抽象；AF_UNIX 在 Windows 上**性能并不占优**——实测命名管道 11–27 µs vs AF_UNIX 18–31 µs（§3.4） | 社区基准 |

**结论**：跨平台"走 IPC"没问题，但**不要假定 Unix domain socket 是统一答案**——在 Windows 上命名管道才是首选（性能更好、Tokio 原生、有消息边界与安全描述符）。若要做跨平台抽象，`interprocess` 的 "local sockets" 正是这个语义（Windows 走命名管道、Unix 走 UDS）。另：IPC 用 **AF_UNIX 只限本机**；要跨机器则需 TCP（本项目不需要）。

### 3.7 本机网络阻断（本次实测发现，属运维约束）

尝试按"预编译路径"做实证（探针 `D:\tmp\v8-probe`，`v8 = "152"`，最小 V8 嵌入程序），**未能完成**：

| 探测 | 结果 |
|---|---|
| `https://api.github.com/repos/denoland/rusty_v8` | ✅ 200（1.05 s） |
| `https://static.crates.io/crates/v8/v8-152.2.0.crate` | ✅ 200，1.49 MB（74 KB/s，慢但可下） |
| **`https://github.com/.../releases/download/.../rusty_v8_release_x86_64-pc-windows-msvc.lib.gz`** | ❌ **code 000，12 s 超时；连续 4 次全部失败** |
| `https://raw.githubusercontent.com/...` | ❌ code 000，10 s 超时 |
| `cargo build`（v8 依赖下载） | ⏳ 22 分钟无进展，唯一缺失包是 `v8-152.2.0`；已手动终止 |

**这是本机网络对 `github.com`（release 资产与 raw）不可达造成的，不是 V8 或 rusty_v8 的技术问题**——但也意味着：

- 在这台机器上**当前无法构建任何依赖 `v8` crate 的工程**（预编译库是唯一来源，而它下不来）。
- 任何 V8 路线的本地开发都需要先解决网络：`RUSTY_V8_MIRROR` 指向可达镜像、配置代理，或改用能访问 GitHub 的机器。
- GitHub Actions runner 位于 GitHub 内网，**不受此限制**——所以"CI 构建"这条路在网络层面是通的。

**因此本报告对 V8 的性能侧数据（isolate/context 创建成本）只有文献值，没有本机实测值。** 这是本次评估的已知缺口，不影响 §3.2/§3.3 的结论方向（那些结论基于上游文档、issue、PR 与已完成的 IPC 实测）。

### 3.8 与既有「网络层 C ABI 草案」的关系：范式复用

这是本次修订中最重要的一条**认知修正**——初版评估把 `docs/network-c-abi-draft.md` 当作"因改用 reqwest 而作废的废纸"，这是错的。

**那份草案的性质**：它不是"网络层的接口设计"，而是一份**跨动态库边界的 ABI 范式规范**。其自述目标原文：

> "**目的**：为后续可能引入的 Zig 源码（编译为动态库链接）提供**稳定的 C ABI 范式规范**。"

也就是说：**网络层只是它的第一个候选消费者，不是它的主题。** 网络层后来改用 reqwest（trait 抽象 + 进程内实现），意味着"**网络层需要它**"这个具体实例不成立；但**这套范式本身正是 Rust ABI 不可用时唯一可行的边界规范**——而这恰恰就是 §3.1 实测得出的结论：

| 草案里的设计原则 | 在 V8/插件边界上的适用性 |
|---|---|
| **不暴露内部类型**（Rust `String`/`Vec`/`Future` 不得跨边界，一律用不透明指针 + `uint8_t*` + `size_t`） | ✅ **正是 `dylib` 失败的根因**：Rust ABI 要求整图动态 + 哈希命名的 `std-<hash>.dll`。C 兼容类型是唯一能跨工具链、跨版本稳定的表示 |
| **不透明句柄**（`struct mk_*` 前向声明，内部布局私有） | ✅ 插件边界同样需要（引擎句柄、realm 句柄、回调注册句柄） |
| **显式内存所有权**（谁 create 谁 free；访问器返回借用指针） | ✅ 跨 cdylib 边界的内存必须单一所有者，否则崩溃在另一个模块的堆上 |
| **panic 不跨 FFI**（每个 `extern "C"` 入口 `catch_unwind`） | ✅ 插件崩溃不得带走宿主——在 MusKitty 这种单进程浏览器里尤其关键 |
| **错误码 + 错误句柄分离** | ✅ 诊断与快速分支分离的通用做法 |
| **可空指针校验**（NULL → 错误码，不 UB） | ✅ 同上 |
| **ABI 版本探测**（`*_abi_version()`，宿主启动时校验，不匹配拒绝加载） | ✅ **对插件场景比对网络场景更关键**——插件由第三方构建，版本错配是常态 |
| **导出符号前缀**（统一 `mk_net_` 避免符号冲突） | ✅ 插件边界必须做（V8 自身还有 Abseil 符号冲突问题，见 §3.3） |

**需要为 V8/插件场景增补的内容**（草案写于网络语境，未覆盖）：

| 增补项 | 原因 |
|---|---|
| **引擎单例契约**：一份 V8 动态库 + 显式 `*_engine_init_once()`，重复初始化返回错误而非崩溃 | 对应 V8 `Platform` 单例与 sandbox 唯一性（§3.3） |
| **不可卸载契约**：库常驻到进程结束；不提供 `*_engine_free()` | 上游明确不支持卸载（#1589）；省掉一整类 use-after-unload |
| **粗粒度边界契约**：ABI 只暴露"批量/整段"操作，**明确禁止**逐属性/逐 DOM 操作的细粒度接口 | 实测 66 µs/往返，1000 次/帧即崩（§3.4） |
| **线程契约**：isolate 单线程进入、**绝不在 UI 线程阻塞等待**；异步回调 + 完成通知取代"阻塞式调用" | 替代草案中网络语境的 `block_on`（那是同步 fetch 的合理设计，但 V8 侧不适用） |
| **大载荷所有权**：超过阈值的数据走共享内存或显式拷贝 API，并在 ABI 上标注所有权转移 | 序列化成本与拷贝语义需要显式化 |

**结论**：`network-c-abi-draft.md` 的**范式部分应当被继承**——不是"作废"，而是"**主题归位**"：它从来是 FFI 边界的范式规范，只是当时挂在了网络层名下。方案 B 若启动，第一件事应当是把这份草案**升格为通用的 `docs/ffi-abi-spec.md`**（去掉 `mk_net_*` 的网络专属函数表，保留全部设计原则，补上上表五项 V8 契约），而不是从零设计一套 ABI。

### 3.9 方案 B 小结

- **形态**：把 `v8` crate 编成 **`cdylib`（C ABI）**——**不是** `dylib`（Rust ABI 实测不可行，§3.1）。产出的是一个**自包含的 V8 动态库**，宿主用 `LoadLibraryW`/`dlopen` 运行时加载（本机已验证该加载路径可用）。
- **插件拓扑**：必须是"**一份 V8 + 插件走 ABI/IPC**"（Node-API 模型）或"**每插件一进程**"，**绝不能每插件各带一份 V8**（§3.3：sandbox 唯一性、cage/线程池复制、卸载不可行）。
- **平台差异**：Linux 用官方预编译库即可（`build.rs` 自动注入 shared-library-safe GN 参数）；**Windows 需要自建**（官方 Windows 制品非该模式）——这正是"开 GitHub 仓库编译"的用处，且因 `v8` crate 自带 V8 源码（§3.5），不需要 100 GB 的完整检出。
- **IPC**：跨进程边界**必须粗粒度**；Windows 上首选**命名管道**（Tokio 原生、性能优于 AF_UNIX，§3.6）。
- **代价**：C++ 依赖 + FFI unsafe + 打破 README "no V8" 定位（需架构师裁决）；分发 +38 MB/平台；Windows 需付费 runner 或自托管 runner 编一次 V8。
- **边界规范**：直接继承 `network-c-abi-draft.md` 的范式并增补五项契约（§3.8）。

---

## 4. 横向对比

| 维度 | Boa 0.22 | rusty_v8 152 |
|---|---|---|
| 语言 / 依赖性质 | 纯 Rust | Rust 绑定 + **C++ 引擎** |
| 与"零 C/C++ 依赖"硬规则 | ✅ 兼容（实测确认） | ❌ 冲突（需架构师批准改规则） |
| 与"零 unsafe"硬规则 | ✅ 本仓库代码可保持 safe（依赖内部有 unsafe） | ⚠️ FFI 边界必然 unsafe（需批准） |
| 与 README "no V8" 定位 | ✅ 兼容 | ❌ 需改定位 |
| 许可（无传染性） | ✅ Unlicense OR MIT；依赖抽样全宽松 | ✅ MIT（绑定）+ BSD-3-Clause（V8） |
| MSRV | 1.91（项目现为 1.82） | 未声明；edition 2024 → 至少 1.85 |
| test262（同日同套件） | 95.59% | **97.59%** |
| 相对性能 | v8-jitless 的 **1/11.9** | 基准线（生产级 JIT） |
| DOM 支持 | ❌ **引擎层锁死**（#5513） | 引擎无 DOM，但**可以**构建（宿主自行实现绑定；Blink 即此路） |
| 事件循环 | 提供 `JobExecutor`/`HostHooks` 钩子，需自建 | 提供微任务队列 API，宏任务需自建 |
| 隔离（崩溃/安全） | 进程内（无隔离） | 可进程化（真隔离，Chromium 模型） |
| 分发体积 | +13.7 MB（单文件，静态） | +38 MB/平台（Windows 仅 release） |
| 构建（不需自编译 V8） | ✅ 本机 11 分钟 | ✅ 预编译路径可（但本机网络不通） |
| 版本稳定性 | 0.x，每次 minor 可能破坏 | 跟随 V8 版本（152.x），API 相对稳定 |
| 上手成本 | 低（加依赖即可） | 高（FFI + 进程 + 协议 + 序列化） |

---

## 5. 性能代价的"人话版"

- **Boa**：一个 1 万行、跑 50 ms 的 JS 在 V8 上是 50 ms，在 Boa 上大约 **0.6 s**（按 11.9 倍折算）。页面会明显卡顿。Boa 的价值不在跑得快，而在"纯 Rust、可审计、可一起改"。
- **IPC**：每帧 100 次往返就吃掉 **40%** 的 60 fps 预算（本机实测 66 µs/次）。1,000 次/帧直接崩到 15 fps。**这是"能"与"不能"的分界线，不是"快"与"慢"的差别。**
- **进程化本身的固定开销**：进程冷启动 + 引擎初始化。Boa 实测进程冷启动 196–297 ms；V8 带快照的 context 创建文献值 < 2 ms、isolate 启动 ~5 ms（厂商口径）。也就是说**每开一个插件进程**，用户侧感知约几十到几百毫秒——可接受的前提是"进程常驻、按需复用"，而不是"每次调用起一个进程"。
- **体积**：Boa +13.7 MB 单文件；V8 +38 MB 且按平台各一份。

---

## 6. 最终受益

### 6.1 接入 Boa 能得到什么

- **保住项目定位**："从零用 Rust 重写浏览器核心"的叙事完整，不引入 C++ 黑盒。
- **可审计、可修改**：引擎与浏览器同一语言，遇到问题能直接读引擎源码、甚至上游修（Boa 是社区项目，贡献门槛低于 V8）。
- **无 FFI/无进程边界**：调试简单，错误可回溯，栈可跨引擎与宿主（未来若 Boa 稳定，`Context` 的单线程模型恰好匹配 MusKitty 现有的单线程渲染管线）。
- **代价换来的"够用"场景**：纯计算类脚本、内置自动化、页面外的工具脚本。**不含页面 JS。**

### 6.2 接入 V8 能得到什么

- **真实 web 兼容性**：97.59% test262 + JIT + 生态里"只有 V8 能跑"的库。这是**唯一**能跑通真实网站 JS 的路线。
- **进程隔离带来的两个额外收益**（独立于性能）：
  - **崩溃隔离**：插件崩溃不带走浏览器（当前 MusKitty 是单进程，任何脚本 panic 都可能终止整个浏览器）。
  - **安全沙箱位**：进程边界是施加权限限制（文件/网络/渲染）的天然位置，也是未来做扩展权限模型的唯一现实支点。
- **性能天花板高**：JIT + 快照，足以支撑真实页面。

### 6.3 代价之外的真实约束

- V8 一进来，**"零 C/C++ 依赖"与"no V8"两条公开规则同时失效**——这不是技术问题，是项目定位问题，需要架构师显式裁决（AGENTS.md：FFI 需批准）。
- 且**它不会自动带来 DOM**。Blink 的 DOM 绑定（WebIDL 代码生成 + 数百个接口）是 Chromium 最大的工程量之一；MusKitty 走 V8 路线后，**最大的一块工作从"写引擎"变成"写绑定与宿主"**——这一点常被低估。

---

## 7. 建议

### 7.1 今天的建议

1. **两个引擎都先不接**。理由：Boa 的 DOM 阻断（#5513）与性能差距（11.9×）今天无法绕过；V8 会同时打破两条项目硬规则，而它带来的收益（页面 JS）在**宿主层缺失**时也无法兑现——现在接进来，只会得到"一个跑不了页面的引擎"。
2. **先做与引擎无关的宿主层**（HTML §4.12 脚本执行时机、事件循环与微任务、DOM 绑定接口设计、脚本→重排重绘的失效传播）。**这层做出来之后，换引擎是局部改动**；反过来先锁死引擎则风险最大。
3. **若要在两者中先做一个 spike**：选 **Boa**，且限定为"feature-gate 的脚本能力探针"（纯计算、无 DOM），用于验证宿主层的接口设计。成本已量化：+13.7 MB、11 分钟构建、MSRV 抬到 1.91。

### 7.2 若将来走 V8（触发条件见 7.3）

1. **先修网络**：本机 `github.com` release 资产不可达（§3.7），需 `RUSTY_V8_MIRROR` / 代理。CI runner 不受此限。
2. **形态：`cdylib`（C ABI）+ 一份 V8 共享**。不选 Rust ABI（实测不可行）；不让每个插件各带一份 V8（§3.3）。需要隔离时进程化**整个插件**，而不是分离 JS 与 DOM。
3. **先把 ABI 规范定下来**：把 `docs/network-c-abi-draft.md` 升格为通用 FFI ABI 规范并增补五项 V8 契约（单例 / 不可卸载 / 粗粒度 / 线程 / 大载荷所有权，见 §3.8）。**这一步应在写任何插件代码之前完成**，否则插件 ABI 会随实现漂移。
4. **Windows 的 V8 自建**：开 CI 仓库，`V8_FROM_SOURCE=1 GN_ARGS='v8_monolithic=true v8_monolithic_for_shared_library=true'`，产物用 `RUSTY_V8_ARCHIVE` 分发给下游。因 `v8` crate 自带源码（§3.5），不需要 100 GB 完整检出；但需 ≥150 GB SSD 的付费 runner（Team/Enterprise）或自托管 runner。
5. **边界协议按"批"设计**：一次导航、一段脚本执行、一批 DOM 变更——**协议里不应出现"单个 DOM 属性读写"这种消息**。Windows 上 IPC 首选命名管道（§3.6）。

### 7.3 明确的触发条件（满足才启动）

| 路线 | 触发条件 |
|---|---|
| Boa | ① 上游 #5513 关闭且提供了外部可用的奇异对象 API；② 综合基准相对 v8-jitless 差距 ≤3×；③ GC 重设计落地；④ 宿主层已完成 |
| V8（页面 JS） | ① 宿主层已完成且能跑起"无脚本"页面；② 架构师批准修改"零 C/C++ 依赖"与"no V8"两条规则；③ 网络/镜像就绪；④ V8 解码统一到 `cdylib` + C ABI 规范（§3.8）已定稿 |
| V8（只要插件隔离） | ① 插件需求真实出现（不是"将来可能"）；② 同上 ②③④；③ 接受 Windows 自建 V8 的 CI 成本 |

---

## 8. 待验证项与风险

| # | 项 | 状态 |
|---|---|---|
| 1 | V8 isolate/context 创建成本的本机实测 | **未完成**（网络阻断）。文献值：context <2 ms（带快照）、isolate ~5 ms（厂商口径） |
| 2 | V8 静态库链进 **Windows cdylib** 的实际行为 | 未验证。Linux 已由 PR #2008/#2009 修通；Windows 仅见 issue #1589 的疑云 |
| 3 | Boa 传递依赖的**全量**许可审计 | 抽样 ~50/159，全宽松；全量审计待 `cargo deny`（项目已具备 `deny.toml`） |
| 4 | Boa 实际页面级性能（非合成基准） | 未测。合成基准差距 11.9× 已足够说明量级 |
| 5 | IPC 在"批量结构化数据"下的真实成本 | 未测。已测的是小载荷 RTT；大批量应测序列化 + 共享内存方案 |
| 6 | MSRV 抬升对 CI 与发布流水线的影响 | 未评估。需要确认 MSRV 1.82 门禁的约束范围（哪些 crate 参与） |

---

## 附录 A：实测环境与方法

| 项 | 值 |
|---|---|
| 主机 | Windows 10 x64（10.0.19045），8 核 |
| 工具链 | rustc 1.98.1 / cargo 1.98.1，host `x86_64-pc-windows-msvc` |
| 磁盘 | C: 剩 29 GB，D: 剩 32 GB |
| 已有工具 | MSVC 2022 + VS18 BuildTools、Windows SDK（10.0.17763/22621/26100/28000）、CMake、ninja、depot_tools（**V8 源码构建按用户要求未执行**） |
| Boa 探针 | `D:\tmp\boa-probe`（`boa_engine = "0.22"`，default features，release）；`cargo build --release` 冷构建计时；启动/上下文/eval 计时见源码（`src/main.rs`、`src/bin/ipcbench.rs`） |
| IPC 探针 | TCP loopback + `set_nodelay(true)`，ping-pong，20,000 次往返取均值，预热 100 次 |
| V8 探针 | `D:\tmp\v8-probe`（`v8 = "152"`）——**未建成**，卡在预编译库下载（见 §3.7） |

## 附录 B：来源清单（一手）

**crates.io / docs.rs**
- `https://crates.io/api/v1/crates/boa_engine`、`https://crates.io/api/v1/crates/v8`
- `https://docs.rs/boa_engine/0.22.0/`（`Context` / `ContextBuilder` / `HostHooks` / `JobExecutor` / `NativeFunction` / `RuntimeLimits`）

**Boa 上游**
- `https://github.com/boa-dev/boa`（README / CHANGELOG / Cargo.toml / test262_config.toml）
- issue #5513（exotic objects / DOM 阻断）、#4988（Web API 缺口）、#4524（1.0 API 审计）、#5522（JIT 前的性能工作）、#5392（unsafe 声索）
- 数据仓库 `https://github.com/boa-dev/data`：`test262/refs/heads/main/latest.json`（95.99%）、`test262/refs/tags/v0.22/latest.json`（95.60%）、`bench/results/*.json`（综合分）
- `https://boajs.dev/roadmap`（GC 重设计、1.0 稳定化）
- 第三方横比 `https://test262.fyi`（`https://data.test262.fyi/index.json`）

**rusty_v8 / V8 / Chromium**
- `https://github.com/denoland/rusty_v8`（README / `build.rs` / `BUILD.gn` / `.github/workflows/ci.yml`）
- issue #411（外部 libv8 劝阻）、#1589（Windows DLL 不释放）、PR #2008/#2009（Linux shared-library TLS）、#1970
- release 资产清单（`api.github.com/repos/denoland/rusty_v8/releases/latest`）
- `https://v8.dev/docs/build`、`https://v8.dev/docs/embed`、`https://v8.dev/blog/custom-startup-snapshots`
- V8 源码：`src/init/v8.cc`、`include/v8-initialization.h`、`gni/v8.gni`、`BUILD.gn`
- `https://chromium.googlesource.com/chromium/src/+/main/mojo/README.md`
- Chromium `docs/windows_build_instructions.md`、`docs/linux/build_instructions.md`（≥100 GB 磁盘）

**GitHub / Windows / IPC**
- `https://docs.github.com/en/actions/reference/runners/github-hosted-runners`（标准 runner 规格）
- `https://docs.github.com/en/actions/concepts/runners/larger-runners`（**仅 Team/Enterprise，按分钟计费**）
- `https://learn.microsoft.com/en-us/windows/win32/ipc/interprocess-communications`（AF_UNIX 自 Win10 build 17063）
- `https://doc.rust-lang.org/std/os/windows/net/`（Windows UDS 仍为 nightly 实验 API）、`https://docs.rs/tokio/latest/tokio/net/struct.UnixStream.html`（Unix-only）
- 社区 IPC 基准：`https://github.com/smithtrenton/ipc-bench`、`https://github.com/aviNaftalis/ipc-shootout`、`https://github.com/goldsborough/ipc-bench`
