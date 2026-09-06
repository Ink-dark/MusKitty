# Phase 5 — Network Layer 规划

> **创建时间**：2026-08-09
> **前置状态**：Phase 4 Renderer B-3/B-4 已完成（DOM→CSS→Layout→Render 全链路打通）。网络层本轮启动，**不与现有 DOM/CSS/Layout/Renderer 链路接轨**，只做基础。
> **用户决策**：先用 reqwest 跑起来，远期自研 HTTP 栈。双向推进——短期接 reqwest，长期写自研路线，本轮两条线都有交付物。
> **核心架构决策**：trait 抽象 + 可插拔实现。上层依赖 `NetworkFetcher` trait 而非具体实现，便于未来切换自研栈时零改动，也便于剥离为独立 crate 后被多种上层复用。

---

## 目标

1. **短期线（本轮交付）**：用 reqwest 把网络层基础搭起来，纳入 workspace 使用范围，提供最小可跑 fetch API + 测试 + demo。
2. **长期线（本轮锚定）**：把自研 HTTP 栈的完整路线图写进本文档，作为远期目标锚点，指导后续迭代。
3. **架构线（贯穿）**：通过 `NetworkFetcher` trait 抽象，让"换实现"成为局部改动，不影响上层。

最终交付物（本轮）：
- `crates/muskitty-network/` workspace member，trait 抽象 + reqwest 后端
- 7 个 wiremock 离线测试全绿
- `examples/fetch_demo.rs` 可跑
- 本规划文档（含自研路线图）

---

## 架构：trait 抽象 + 可插拔实现

```
┌─────────────────────────────────────────────┐
│  上层（未来浏览器 fetch / resource loader）   │
│  依赖 NetworkFetcher trait，不绑定具体实现     │
└──────────────────┬──────────────────────────┘
                   │ F: NetworkFetcher
                   ▼
┌─────────────────────────────────────────────┐
│  muskitty-network crate                      │
│  ┌───────────────────────────────────────┐   │
│  │ NetworkFetcher trait (核心抽象)        │   │
│  │ async fn fetch(&self, url) -> Resp    │   │
│  └───────────┬───────────────────────────┘   │
│              │ impl                          │
│   ┌──────────┴──────────┐                    │
│   ▼                     ▼                    │
│ ┌─────────────┐   ┌──────────────────┐       │
│ │ReqwestFetcher│   │ NativeFetcher    │       │
│ │ (当前, 默认) │   │ (远期自研, TODO) │       │
│ │reqwest+rustls│   │ HTTP/1.1+2+3     │       │
│ └─────────────┘   └──────────────────┘       │
└─────────────────────────────────────────────┘
```

### 关键设计

1. **`NetworkFetcher` trait**（`src/fetcher.rs`）
   - 原生 `async fn in trait`（MSRV 1.82 稳定，无需 `async-trait` crate）
   - 暂不支持 `dyn` 分发，上层用泛型约束 `F: NetworkFetcher`；未来需要 trait 对象时引入 `trait_variant` 即可，接口签名不变
   - 当前仅 `fetch(url)`，随需求扩展（POST / headers / body / 超时等）

2. **`ReqwestFetcher`**（`src/reqwest_impl.rs`，`reqwest-backend` feature-gated，默认开启）
   - 基于 `reqwest::Client`，复用底层连接池，线程安全可克隆
   - TLS 用 `rustls-tls`（满足 AGENTS.md "零 C/C++ 依赖" 硬约束，不用 native-tls / OpenSSL）
   - 启用 `http2` + `charset`

3. **数据类型实现无关**（`src/response.rs` / `src/error.rs`）
   - `NetworkResponse { status, headers, url, body_bytes }` — 任何后端都产出此类型
   - `NetworkError { Http(reqwest::Error), InvalidUrl(String) }` — 当前 reqwest 后端的错误；自研后端会扩展变体（如 `Tls` / `Http1Parse` / `Hpack` 等）

4. **feature flag 切换后端**
   - `default = ["reqwest-backend"]` — 开箱即用
   - `muskitty-network = { version = "0.1", default-features = false }` — 仅 trait 抽象，上层自行实现或选其他后端
   - 未来自研栈就绪后：新增 `native-backend` feature，上层按需切换；最终 `default` 可改为 `["native-backend"]`

### 为什么不直接 `dyn NetworkFetcher`

- 原生 `async fn in trait` 的 `dyn` 支持需要额外 crate（`async-trait` 或 `trait_variant`），与"零额外依赖"原则冲突
- 当前上层调用点少，泛型约束 `F: NetworkFetcher` 足够
- 接口签名稳定，未来加 `dyn` 支持是纯增量改动，不破坏 API

### 剥离准备

当前作为主仓库 workspace member 开发（遵循 project_memory："新 crate 应在主仓库定稿后再剥离"）。架构上已为剥离做好准备：
- trait 抽象 + feature flag 使其可被任意上层复用
- 零项目内 path 依赖（不依赖 dom/css/layout 等），剥离时无依赖拓扑约束
- 剥离时机：自研后端就绪并稳定后，或上层（fetch 标准 / resource loader）开始大量使用时

---

## 短期线：reqwest 后端（本轮已交付）

### 已实现

| 文件 | 职责 |
|------|------|
| `src/lib.rs` | crate 文档 + 模块入口 + re-export + 便捷 `fetch()` |
| `src/fetcher.rs` | `NetworkFetcher` trait 定义（核心抽象） |
| `src/response.rs` | `NetworkResponse` 数据类型（实现无关） |
| `src/error.rs` | `NetworkError` / `NetworkResult`（实现无关） |
| `src/reqwest_impl.rs` | `ReqwestFetcher: NetworkFetcher`（feature-gated） |
| `tests/fetcher.rs` | 7 个 wiremock 离线集成测试 |
| `examples/fetch_demo.rs` | 可跑 demo：`cargo run --example fetch_demo -- <url>` |

### 测试覆盖

| 测试 | 验证内容 |
|------|---------|
| `fetch_success_returns_status_body_headers` | 200 响应：status / body / header 大小写不敏感 / final url |
| `fetch_404_does_not_error` | 4xx 不报错，调用方自行判断 status |
| `fetch_500_does_not_error` | 5xx 不报错 |
| `fetch_binary_body_preserved` | 二进制 body 字节完整保留 |
| `fetch_trait_object_via_generic_works` | 上层可用泛型 `F: NetworkFetcher` 调用，不绑定具体类型 |
| `fetch_connection_refused_returns_error` | 连接失败返回 `NetworkError::Http` |
| `fetcher_is_clone_and_reuses` | `Clone` 后两个实例都能正常 fetch |

### API 面（v0.1.0）

```rust
pub trait NetworkFetcher {
    async fn fetch(&self, url: &str) -> NetworkResult<NetworkResponse>;
}

pub struct NetworkResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub url: String,
    /* body_bytes private */
}
impl NetworkResponse {
    pub fn body_bytes(&self) -> &[u8];
    pub fn text(&self) -> String;        // UTF-8 lossy
    pub fn header(&self, name: &str) -> Option<&str>;
    pub fn is_success(&self) -> bool;    // 2xx
}

pub enum NetworkError { Http(reqwest::Error), InvalidUrl(String) }
pub type NetworkResult<T> = Result<T, NetworkError>;

#[cfg(feature = "reqwest-backend")]
pub struct ReqwestFetcher { /* reqwest::Client */ }
impl ReqwestFetcher {
    pub fn new() -> NetworkResult<Self>;
}
impl NetworkFetcher for ReqwestFetcher { /* ... */ }

#[cfg(feature = "reqwest-backend")]
pub async fn fetch(url: &str) -> NetworkResult<NetworkResponse>;
```

---

## 长期线：自研 HTTP 栈路线图

> 本节是远期目标锚点，不在本轮实现。每个阶段独立交付，按需启动。

### 阶段划分

| 阶段 | 内容 | 规范 | 入场门槛 |
|------|------|------|---------|
| N-1 | TCP + DNS 基础（异步 socket） | RFC 9293 / RFC 1035 | 本轮 reqwest 后端稳定 |
| N-2 | TLS 1.3（基于 rustls） | RFC 8446 | N-1 完成 |
| N-3 | HTTP/1.1 客户端（请求/响应/连接池） | RFC 9110 / RFC 9112 | N-1 + N-2 完成 |
| N-4 | HTTP/2（HPACK + 多路复用） | RFC 9113 | N-3 完成 |
| N-5 | HTTP/3 over QUIC | RFC 9114 / RFC 9000 | N-4 完成（远期） |
| N-6 | WHATWG Fetch 规范集成 | WHATWG Fetch | N-3 完成（可与 N-4/N-5 并行） |
| N-7 | 替换 reqwest 为默认后端 | — | N-3 + N-6 完成，N-4 选做 |

### 设计原则（自研阶段）

1. **规范优先**：每个阶段先读对应 RFC 章/节，再实现。参考优先级 RFC > WPT > Chromium 源码 > reqwest 源码
2. **零 C/C++ 依赖**：TLS 用 rustls（不是 ring 的 C 部分，用 rustls 的纯 Rust 后端 `rustls::crypto::ring` 已是默认；QUIC 用 quinn/quiche 的纯 Rust 实现）
3. **trait 不变**：自研后端实现 `NetworkFetcher`，上层零改动
4. **测试用 wiremock 复用**：自研后端的测试与 reqwest 后端共用同一套 wiremock fixture，保证语义等价
5. **增量替换**：N-3（HTTP/1.1）完成后即可作为可选后端跑起来，N-4（HTTP/2）作为增强，不阻塞主线

### 各阶段要点

**N-1 TCP + DNS**
- 异步 TCP：`tokio::net::TcpStream`（tokio 是异步运行时事实标准，纯 Rust）
- DNS 解析：`tokio::net::lookup_host` 或自研异步 resolver（trust-dns / hickory-dns 纯 Rust）
- 连接超时 / 读超时配置
- 验证：能 TCP 连接到 `example.com:80` 并发送/接收原始字节

**N-2 TLS 1.3**
- 基于 `rustls` crate（纯 Rust TLS 实现，server verification 用 webpki-roots）
- SNI / 证书验证 / ALPN 协商（为 HTTP/2 准备 `h2`）
- 验证：能 TLS 握手到 `https://example.com` 并发送/接收加密字节

**N-3 HTTP/1.1 客户端**
- RFC 9110 HTTP 语义 + RFC 9112 HTTP/1.1 消息格式
- 请求行 / headers / body（Content-Length + chunked transfer-encoding）
- 响应解析：status line / headers / body（含 chunked 解码）
- 连接池：keep-alive + 复用 + 空闲超时
- 实现 `NetworkFetcher` trait，与 `ReqwestFetcher` 行为等价（共用 wiremock 测试）
- 验证：wiremock 测试全绿 + 真实站点 fetch 对比

**N-4 HTTP/2**
- RFC 9113 HTTP/2 + RFC 7541 HPACK
- 帧（frame）解析 / 流（stream）多路复用 / 流量控制 / 优先级
- ALPN 协商 `h2`（N-2 已准备）
- 验证：HTTP/2 站点 fetch（如 google.com）+ h2spec 兼容性测试

**N-5 HTTP/3 over QUIC**（远期）
- RFC 9114 HTTP/3 + RFC 9000 QUIC
- 基于 quinn crate（纯 Rust QUIC 实现）
- 验证：HTTP/3 站点 fetch（如 cloudflare.com）

**N-6 WHATWG Fetch 规范集成**
- WHATWG Fetch 标准的 Request / Response / Headers / Body 抽象
- CORS / redirect / credentials 策略
- 与 DOM 集成（`<img>` / `<script>` / `<link>` / `fetch()` JS API）
- 这是网络层与现有 DOM/CSS/Layout/Renderer 链路接轨的入口

**N-7 替换默认后端**
- `default = ["native-backend"]`，reqwest 降级为可选 fallback
- 性能基准对比（自研 vs reqwest）
- 完整 WPT fetch 测试套件通过

### 不在本路线图范围

- **代理 / SOCKS**：推迟到 N-6 之后按需
- **Cookie jar**：推迟到 N-6（Fetch 规范包含 cookie 处理）
- **Service Worker 拦截**：远期，依赖 JS 运行时
- **WebTransport**：远期，依赖 HTTP/3

---

## 当前状态与下一步

### 本轮已完成（2026-08-09）

- [x] `muskitty-network` crate 骨架（trait + reqwest 后端 + feature flag）
- [x] 7 个 wiremock 离线测试全绿
- [x] `fetch_demo` example 可跑
- [x] 主 `Cargo.toml` 加入 `members`
- [x] 本规划文档（含自研路线图）
- [x] CLAUDE.md / PROGRESS.md 同步标注

### 下一步（待用户指令）

- **不立即启动自研**：本轮只搭基础 + 锚定路线图
- **触发自研的条件**：上层（fetch 标准 / resource loader）开始需要 reqwest 不支持的能力（如精细流控 / 自定义协议拦截 / 性能调优），或用户明确要求启动 N-1
- **接现有链路的条件**：N-6（WHATWG Fetch 集成）启动时，才会与 DOM/CSS/Layout/Renderer 链路接轨

---

## 验证

本轮交付验证（在工作区根或 crate 目录执行）：

```bash
cargo check -p muskitty-network
cargo test -p muskitty-network
cargo fmt -p muskitty-network -- --check
cargo clippy -p muskitty-network --all-targets -- -D warnings
cargo run -p muskitty-network --example fetch_demo -- https://example.com
```

---

## 接驳（2026-09-06，用户指令）

原计划"N-6（WHATWG Fetch 集成）启动时才接轨"；架构师于 2026-09-06 指令提前接驳，
范围收窄为**顶级文档 GET 导航**（HTML Standard §7.2 navigation 极简子集），
子资源 / 历史栈 / 刷新语义仍待后续：

- `muskitty-network`：补 [`fetch_blocking`] 同步便捷入口（内部一次性
  current_thread 运行时）——chrome 是同步 UI 层，异步运行时细节留在网络 crate，
  未来换自研后端上层零改动；`NetworkResponse::new` 转正为 pub（上层测试构造入口）。
- `muskitty-chrome` 新增 `navigation` 模块：`classify_url`（http/https / file:// /
  本地绝对路径 / scheme 白名单判定不支持；无 scheme 补 https，localhost 补 http）、
  `document_from_response`（Content-Type 分发，4xx/5xx 正文照常渲染）、
  `spawn_http_navigation`（独立线程 + channel 回传，不阻塞 winit UI 线程）。
- chrome `app.rs` 接线：地址栏提交 → 导航（加载期间保留旧页、标题先更新）；
  `(tab, epoch)` 导航代数使改址/关签后的过期结果静默丢弃。
- 上层依赖仍是 [`NetworkFetcher`] trait 语义（`fetch_blocking` 是 trait 的同步
  镜像），trait 抽象决策不变；自研栈路线 N-1~N-7 不变。

验证（新增）：

```bash
cargo test -p muskitty-chrome          # 含 navigation 离线 e2e（原生 TcpListener）
cargo test -p muskitty-chrome --no-default-features
cargo run -p muskitty-chrome           # 人工：地址栏输入 example.com → 渲染真实页面
```
