# MusKitty Network C ABI 接口草案

> **日期**：2026-08-16
> **状态**：草案（Draft，未实施）
> **目的**：为后续可能引入的 Zig 源码（编译为动态库链接）提供稳定的 C ABI 范式规范。

---

## 一、目标与约束

### 目标

- 定义一个**语言无关的 C ABI**，作为 Rust（MusKitty 本体）与 Zig（潜在动态库）之间的桥接边界。
- 使 Zig 编译的动态库能被 MusKitty 通过 `dlopen`/`#[link]` 加载调用，反之亦然。
- ABI 稳定：Zig 侧升级 / 更换实现时，Rust 上层（以及未来 C 消费方）零改动。

### 约束（对齐 CLAUDE.md 硬约束）

- **C ABI 属 FFI 边界**：按 CLAUDE.md「零 unsafe（FFI 边界需架构师批准）」，本草案的 `unsafe` 全部集中在 FFI 薄层（`extern "C"` 导出/导入 + 指针转换），业务逻辑仍纯 safe Rust。
- **不暴露内部类型**：Rust 的 `String`/`Vec`/`Future`、Zig 的切片/结构体均不得跨 ABI 边界，一律用 C 兼容类型（不透明指针、`uint8_t*`、`size_t`）。
- **与现有 trait 语义等价**：C ABI 是 [`NetworkFetcher`](crates/muskitty-network/src/fetcher.rs) trait 的薄封装，语义严格对齐（4xx/5xx 不算错误、错误分类、`header()` 大小写不敏感）。

---

## 二、设计原则

| 原则 | 说明 |
|------|------|
| **不透明句柄** | 对象（fetcher / response / error）以 `struct mk_net_*` 前向声明的不透明指针暴露，内部布局私有，ABI 稳定 |
| **阻塞式 fetch** | Rust 侧在 FFI 函数内 `block_on`；Zig/C 侧为同步调用。异步回调（`mk_net_fetch_async` + 完成回调）列为后续扩展 |
| **UTF-8 字节 + 长度** | 所有字符串（URL / header name / header value / 错误消息）用 `const uint8_t*` + `size_t` 传递，**不依赖 NUL 结尾**（URL 与 header 可含任意字节） |
| **显式内存所有权** | create 返回的句柄由调用方 `_free`；访问器返回的指针指向句柄内部，句柄释放后失效；`error_out` 由被调用方填充、调用方释放 |
| **错误码 + 错误句柄** | 错误码（`enum`，快速分支）+ 错误句柄（人类可读消息，诊断），二者分离 |
| **panic 不跨 FFI** | 每个 `extern "C"` 函数入口 `catch_unwind`，panic 转 `MK_NET_ERR_INTERNAL`，绝不把 Rust/Zig panic 泄漏到对侧 |
| **可空指针校验** | 所有入参句柄/输出指针做 `NULL` 校验，非法返回 `MK_NET_ERR_NULL_ARG`，不触发 UB |

---

## 三、C 头文件（范式，可直接 `@cImport` / `#include`）

```c
/* mk_net.h — MusKitty Network C ABI（草案 v0.1） */

#ifndef MK_NET_H
#define MK_NET_H

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* —— 不透明句柄：内部布局私有，不得在调用侧解引用 —— */
typedef struct mk_net_fetcher  mk_net_fetcher;
typedef struct mk_net_response mk_net_response;
typedef struct mk_net_error    mk_net_error;

/* —— 错误码 —— */
typedef enum {
    MK_NET_OK                 = 0,  /* 成功 */
    MK_NET_ERR_INVALID_URL    = 1,  /* URL 解析/格式错误 */
    MK_NET_ERR_HTTP           = 2,  /* DNS / 连接 / TLS / 协议 / 超时 */
    MK_NET_ERR_NULL_ARG       = 3,  /* 传入空指针 */
    MK_NET_ERR_INVALID_HANDLE = 4,  /* 句柄无效或已释放 */
    MK_NET_ERR_INTERNAL       = 5,  /* 内部 panic / 未分类错误 */
} mk_net_error_code;

/* ================= fetcher 生命周期 ================= */

/* 创建 fetcher。成功返回非空句柄；失败返回 NULL 且 *error_out 非空。
 * error_out 可为 NULL（不关心错误详情）。*error_out 由调用方 mk_net_error_free。 */
mk_net_fetcher* mk_net_fetcher_create(mk_net_error** error_out);

/* 释放 fetcher。空指针安全（no-op）。释放后句柄失效。 */
void mk_net_fetcher_free(mk_net_fetcher* fetcher);

/* ================= fetch（阻塞式） ================= */

/* GET 指定 URL，阻塞直到完成。url 为 UTF-8 字节 + 长度（可不含 NUL）。
 * 成功返回非空 response（调用方 mk_net_response_free）；
 * 失败返回 NULL 且 *error_out 非空（调用方 mk_net_error_free）。
 * HTTP 4xx/5xx 不算错误，通过 mk_net_response_status 读取。
 * error_out 可为 NULL。 */
mk_net_response* mk_net_fetch(
    mk_net_fetcher* fetcher,
    const uint8_t* url,
    size_t url_len,
    mk_net_error** error_out
);

/* ================= response 访问器 ================= */
/* 返回的指针指向 response 内部，mk_net_response_free 后失效。 */

uint16_t mk_net_response_status(const mk_net_response* resp);

/* 响应头数量。 */
size_t mk_net_response_header_count(const mk_net_response* resp);

/* 取第 index 个响应头（保留插入顺序，同名可重复）。
 * name/value 为输出参数（UTF-8 + 长度），指向 response 内部。
 * 越界返回 false。 */
bool mk_net_response_header_at(
    const mk_net_response* resp,
    size_t index,
    const uint8_t** name, size_t* name_len,
    const uint8_t** value, size_t* value_len
);

/* 最终 URL（重定向后）。*url_len 返回字节数。 */
const uint8_t* mk_net_response_url(const mk_net_response* resp, size_t* url_len);

/* 响应体原始字节。*body_len 返回字节数。 */
const uint8_t* mk_net_response_body(const mk_net_response* resp, size_t* body_len);

/* 释放 response。空指针安全。 */
void mk_net_response_free(mk_net_response* resp);

/* ================= error 访问器 ================= */

mk_net_error_code mk_net_error_code(const mk_net_error* err);

/* 错误消息（UTF-8）。*msg_len 返回字节数。 */
const uint8_t* mk_net_error_message(const mk_net_error* err, size_t* msg_len);

/* 释放 error。空指针安全。 */
void mk_net_error_free(mk_net_error* err);

#ifdef __cplusplus
}
#endif

#endif /* MK_NET_H */
```

---

## 四、错误码表

| 枚举值 | 值 | 含义 | 映射自 Rust |
|--------|----|------|------------|
| `MK_NET_OK` | 0 | 成功 | — |
| `MK_NET_ERR_INVALID_URL` | 1 | URL 解析/格式错误 | `NetworkError::InvalidUrl` |
| `MK_NET_ERR_HTTP` | 2 | DNS/连接/TLS/协议/超时 | `NetworkError::Http` |
| `MK_NET_ERR_NULL_ARG` | 3 | 传入空指针（FFI 层校验） | — |
| `MK_NET_ERR_INVALID_HANDLE` | 4 | 句柄无效/已释放 | — |
| `MK_NET_ERR_INTERNAL` | 5 | 内部 panic 或未分类错误 | — |

---

## 五、内存所有权约定（跨 FFI 边界的核心）

| 资源 | 分配方 | 释放方 | 释放 API | 备注 |
|------|--------|--------|----------|------|
| fetcher 句柄 | 被调用方（`create`） | 调用方 | `mk_net_fetcher_free` | 空指针 no-op |
| response 句柄 | 被调用方（`fetch`） | 调用方 | `mk_net_response_free` | 空指针 no-op |
| error 句柄 | 被调用方（`create`/`fetch` 的 `*error_out`） | 调用方 | `mk_net_error_free` | 空指针 no-op |
| url / body / header 指针 | 指向 response 内部 | **不单独释放** | — | response 释放后失效 |
| error message 指针 | 指向 error 内部 | **不单独释放** | — | error 释放后失效 |

**铁律**：谁 `create`/`fetch` 出句柄，谁负责 `_free`；访问器返回的指针是「借用」，不得跨句柄生命周期使用。

---

## 六、线程安全

- `mk_net_fetcher`：内部持有 `Arc<…>`，可跨线程并发 `mk_net_fetch`（对应 Rust 侧 `NetworkFetcher::fetch` 的 `Send` bound）。
- `mk_net_response` / `mk_net_error`：非线程共享，单线程使用，跨线程需调用方自行同步。
- `mk_net_fetcher_free`：调用方需保证无并发 `fetch` 进行中（或实现为原子引用计数，草案阶段由调用方约定）。

---

## 七、Rust 侧实现范式

Rust 侧以 `#[no_mangle] extern "C"` 导出，内部薄封装 `NetworkFetcher` trait，`unsafe` 集中在指针转换与 `block_on`。

```rust
//! muskitty-network 的 C ABI 导出（草案范式）。FFI 边界，unsafe 集中于此。

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::{NetworkFetcher, NetworkResponse, ReqwestFetcher};

/// 不透明 fetcher：内部存 `Box<dyn NetworkFetcher + Send>`。
#[repr(C)]
pub struct MkNetFetcher {
    _private: [u8; 0], // 实际实现为 Box<dyn NetworkFetcher>
}

#[no_mangle]
pub unsafe extern "C" fn mk_net_fetcher_create(
    error_out: *mut *mut MkNetError,
) -> *mut MkNetFetcher {
    catch_unwind(AssertUnwindSafe(|| {
        let fetcher = match ReqwestFetcher::new() {
            Ok(f) => f,
            Err(e) => {
                if !error_out.is_null() {
                    *error_out = MkNetError::box_into_raw(MkNetError::from(e));
                }
                return std::ptr::null_mut();
            }
        };
        // Box<dyn NetworkFetcher> → *mut c_void → *mut MkNetFetcher
        let boxed: Box<dyn NetworkFetcher + Send> = Box::new(fetcher);
        Box::into_raw(Box::new(boxed)) as *mut MkNetFetcher
    }))
    .unwrap_or(std::ptr::null_mut()) // panic 已在上层被 error 承载；此处兜底
}

#[no_mangle]
pub unsafe extern "C" fn mk_net_fetch(
    fetcher: *mut MkNetFetcher,
    url: *const u8,
    url_len: usize,
    error_out: *mut *mut MkNetError,
) -> *mut MkNetResponse {
    // 1. NULL 校验
    if fetcher.is_null() || url.is_null() {
        // 填 MK_NET_ERR_NULL_ARG
        return std::ptr::null_mut();
    }
    // 2. 重建 &Box<dyn NetworkFetcher>
    // 3. 构造 &str（from_raw_parts，调用方保证 url 在调用期间有效）
    // 4. 需要 tokio runtime 的 block_on：fetcher 内部可持有 Runtime 句柄，
    //    或惰性创建。草案采用「fetcher 内部持有 Handle」。
    // 5. 结果映射：Ok(resp) → Box<NetworkResponse> → *mut MkNetResponse
    //    Err(e) → *error_out = MkNetError::box_into_raw(e)
    // 全程 catch_unwind，panic 转 MK_NET_ERR_INTERNAL
    todo!("实现范式见上，具体映射依赖 runtime 承载方式")
}
```

> 注：`block_on` 需要一个 tokio runtime。草案约定 `MkNetFetcher` 内部持有 `tokio::runtime::Handle`（在 `create` 时绑定当前 runtime，或惰性 `Runtime::new()`），`fetch` 内 `handle.block_on(...)`。若上层已有 runtime，可提供 `mk_net_fetcher_create_with_runtime` 变体避免每 fetcher 建 runtime。

---

## 八、Zig 侧集成范式

### 8.1 声明（@cImport）

```zig
const c = @cImport({
    @cInclude("mk_net.h");
});
```

### 8.2 调用示例（阻塞 fetch + defer 释放）

```zig
const std = @import("std");

/// 阻塞 fetch 一个 URL，返回响应体字节（调用方持有）。
/// 出错返回 error.FetchFailed（并打印错误消息到 stderr）。
pub fn fetch(allocator: std.mem.Allocator, url: []const u8) ![]u8 {
    var create_err: ?*c.mk_net_error = null;
    const fetcher = c.mk_net_fetcher_create(&create_err) orelse {
        defer if (create_err) |e| c.mk_net_error_free(e);
        logError(create_err);
        return error.CreateFailed;
    };
    defer c.mk_net_fetcher_free(fetcher);

    var fetch_err: ?*c.mk_net_error = null;
    const resp = c.mk_net_fetch(fetcher, url.ptr, url.len, &fetch_err) orelse {
        defer if (fetch_err) |e| c.mk_net_error_free(e);
        logError(fetch_err);
        return error.FetchFailed;
    };
    defer c.mk_net_response_free(resp);

    const status = c.mk_net_response_status(resp);
    if (status >= 400) return error.HttpStatus; // 4xx/5xx 由调用方判断

    var body_len: usize = 0;
    const body_ptr = c.mk_net_response_body(resp, &body_len);

    // 拷贝出响应体：访问器指针在 resp 释放后失效，故在 defer 前复制。
    const owned = try allocator.dupe(u8, body_ptr[0..body_len]);
    return owned;
}

fn logError(err: ?*c.mk_net_error) void {
    if (err) |e| {
        var msg_len: usize = 0;
        const msg = c.mk_net_error_message(e, &msg_len);
        std.debug.print("network error: {s}\n", .{msg[0..msg_len]});
    }
}
```

### 8.3 Zig 作为实现方（编译为动态库）

若 Zig 自研 HTTP 栈实现 C ABI（而非调用），则：

```zig
// build.zig 关键片段：编译为 C ABI 动态库
const lib = b.addSharedLibrary(.{
    .name = "muskitty_net_zig",
    .root_source_file = b.path("src/main.zig"),
    .target = target,
    .optimize = optimize,
});
lib.linkLibC();
b.installArtifact(lib);
```

Zig 侧导出函数（`callconv(.C)`）：

```zig
const NetFetcher = opaque {};

export fn mk_net_fetcher_create(error_out: *?*NetError) callconv(.C) ?*NetFetcher {
    // 内部用 std.mem.Allocator 分配 fetcher 结构
    // 错误填充 *error_out
}

export fn mk_net_fetch(
    fetcher: *NetFetcher,
    url: [*]const u8,
    url_len: usize,
    error_out: *?*NetError,
) callconv(.C) ?*NetResponse {
    // 同步阻塞 fetch（内部 std.Thread 或自实现事件循环）
}
```

> Zig 与 Rust 的关键对等点：`callconv(.C)` ↔ `extern "C"`；`opaque {}` ↔ 前向声明不透明 struct；`[*]const u8` ↔ `const uint8_t*`；`?*T` ↔ 可空指针。内存所有权遵循第五节铁律。

---

## 九、与 `NetworkFetcher` trait 的映射

| C ABI | Rust 侧 | 语义 |
|-------|---------|------|
| `mk_net_fetcher_create` | `ReqwestFetcher::new()` | 构造默认 fetcher |
| `mk_net_fetch` | `fetcher.fetch(url).await`（block_on 包装） | 阻塞式 GET |
| `mk_net_response_status` | `NetworkResponse::status` | HTTP 状态码 |
| `mk_net_response_header_count` / `_at` | `NetworkResponse::headers` 迭代 | 头列表 |
| `mk_net_response_url` | `NetworkResponse::url` | 最终 URL |
| `mk_net_response_body` | `NetworkResponse::body_bytes()` | 响应体字节 |
| `mk_net_error_code` / `_message` | `NetworkError` 变体 + `Display` | 错误分类 + 消息 |

**语义对齐**（严格遵循 trait 文档）：
- HTTP 4xx/5xx **不算错误**，`mk_net_fetch` 仍返回 response，调用方读 `status`。
- 仅网络层错误（DNS/连接/TLS/超时）与 URL 解析错误走 `*error_out`。
- `header_at` 的大小写不敏感查找语义由 Rust 侧 `NetworkResponse::header()` 承担（若 C ABI 暴露 `header(name)` 而非 `header_at(index)`）；草案以 `header_at` 暴露原始列表，上层自行实现查找。

---

## 十、ABI 稳定性与版本化

- **草案阶段（0.x）**：ABI 可能变动，不承诺兼容；每次变更递增 `MK_NET_ABI_VERSION`。
- **稳定后（1.0）**：承诺语义版本兼容——新增函数/枚举值为**可加**变更；删除/改签名/改布局为**破坏**变更。
- **导出符号前缀**：统一 `mk_net_`，避免与宿主符号冲突。
- **版本探测**：提供 `uint32_t mk_net_abi_version(void)`，宿主启动时校验，版本不匹配则拒绝加载。

---

## 十一、待定问题（Open Questions）

1. **异步范式**：是否需要 `mk_net_fetch_async(fetcher, url, len, callback, userdata)`？浏览器场景多资源并发是刚需，但草案先以阻塞式提供最小可用范式，异步回调列为下一迭代。
2. **runtime 承载**：`block_on` 依赖 tokio runtime——是 fetcher 内部持有 `Handle`，还是每个 fetch 惰性建 runtime？前者要求上层先启动 runtime，后者有重复建 runtime 开销。倾向 `create` 时绑定当前 `Handle` + 提供 `create_with_runtime` 变体。
3. **自定义 header / 请求体 / 方法**：草案仅 GET + URL，未来扩展 `mk_net_fetch_with_options`（携带 method/headers/body/超时）。
4. **响应头按名查找**：C ABI 用 `header_at(index)` 还是额外 `header(name)`？后者与 Rust `header()` 语义更贴，但增加一次跨 FFI 调用。
