//! 地址栏导航：URL 分类 + http(s) 后台抓取 + 响应 → 页面文档。
//!
//! HTML Standard §7.2 navigation 的极简子集——仅顶级文档 GET，无历史栈、
//! 无子资源、无 MIME 嗅探：
//!
//! 1. 输入分类（[`classify_url`]）：`http`/`https` → 网络抓取；`file://` →
//!    本地文件；无 scheme 补全（默认 `https://`，localhost/127.0.0.1 补
//!    `http://`——本地开发服务常无 TLS 证书）；其余 scheme → 不支持。
//! 2. http(s) 在**独立线程**同步抓取（[`spawn_http_navigation`]），结果经
//!    channel 送回 winit 事件循环（app 层统一 flush 点消费）——网络 IO
//!    永不阻塞 UI 线程。加载语义与浏览器一致：加载期间保留旧页。
//! 3. 响应 → 文档（[`document_from_response`]）按 Content-Type 分发：
//!    `text/html` → 走完整渲染管线（提取 `<style>` 为 Author CSS）；
//!    `text/plain` → `<pre>` 回显；其余 → 提示页。4xx/5xx 不算失败，
//!    服务器错误页正文照常渲染；只有网络层错误（DNS / 连接 / TLS /
//!    超时 / 体积上限）才生成 [`error_page`]。
//!
//! 纯函数层（分类 / 转换 / 错误页）无窗口可测；端到端用原生
//! `TcpListener` 起离线 HTTP server（真 reqwest → 真线程 → 转换），
//! 不依赖外网。

use muskitty_network::NetworkResponse;

/// 一次导航的最终结果（channel 回传给 app 层）。
#[derive(Debug)]
pub struct NavigationOutcome {
    /// 发起导航的标签索引（提交时快照）。
    pub tab: usize,
    /// 导航代数（提交时目标标签的 epoch）；到站时与标签当前代数不等 =
    /// 过期导航（用户改址 / 关签后索引复用），app 层静默丢弃。
    pub epoch: u64,
    /// 请求的 URL（错误页与标题回退用；成功时标题用最终 URL）。
    pub url: String,
    /// 成功 → 转换出的文档；失败 → 网络层错误消息。
    pub result: Result<NavigationDoc, String>,
}

/// 抓取成功后转换出的页面文档。
#[derive(Debug)]
pub struct NavigationDoc {
    /// 最终 URL（重定向后；标签标题用它而非请求 URL）。
    pub final_url: String,
    /// 页面 HTML。
    pub html: String,
    /// 提取的 Author CSS（`<style>` 块拼接；页外 CSS `<link>` 不在本轮范围）。
    pub css: String,
}

/// 地址栏输入的导航分类（[`classify_url`] 的结果）。
#[derive(Debug, PartialEq, Eq)]
pub enum NavigationKind {
    /// http/https 顶级文档导航（规范化后的 URL）。
    Http(String),
    /// 本地文件（`file://` 展开后的路径）。
    File(String),
    /// 不支持的 scheme（`data:`/`about:`/`javascript:` 等或空输入）。
    Unsupported(String),
}

/// 地址栏输入 → 导航分类。
///
/// - 已带 `http(s)://`：原样导航。
/// - `file://` 或本地绝对路径（Windows 盘符 `D:\x` / Unix `/x`）：文件加载。
/// - 命中 [`NON_HTTP_SCHEMES`] 白名单的 scheme（`data:`/`about:` 等）：
///   不支持。scheme 判定必须走白名单而非语法探测——scheme 语法允许
///   含 `.`（如 `example.com:8080` 的 host:port），白名单才不会误伤。
/// - 无 scheme：补全——host 为 localhost/127.0.0.1 补 `http://`，否则补
///   `https://`（现代浏览器对键入 host 的默认处理）。
pub fn classify_url(input: &str) -> NavigationKind {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return NavigationKind::Unsupported(trimmed.to_string());
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return NavigationKind::Http(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("file://") {
        // Windows 盘符形式 file:///D:/x → 剥离后残留 "/D:/x"：多出的首个
        // '/' 会破坏 std::fs 路径解析，需去掉；Unix 绝对路径保留。
        let path = if rest.len() >= 3 && rest.starts_with('/') && rest.as_bytes()[2] == b':' {
            &rest[1..]
        } else {
            rest
        };
        return NavigationKind::File(path.to_string());
    }
    let bytes = trimmed.as_bytes();
    if (bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || trimmed.starts_with('/')
    {
        return NavigationKind::File(trimmed.to_string());
    }
    if let Some(scheme) = leading_scheme(trimmed) {
        if NON_HTTP_SCHEMES.contains(&scheme.as_str()) {
            return NavigationKind::Unsupported(trimmed.to_string());
        }
    }
    let scheme = if is_local_host(trimmed) {
        "http"
    } else {
        "https"
    };
    NavigationKind::Http(format!("{scheme}://{trimmed}"))
}

/// 键入地址栏时可识别的"非 HTTP"scheme 白名单（http/https/file 已在
/// 上方分支处理）：命中 → [`NavigationKind::Unsupported`]；未命中 →
/// 视为 `host:port` 形态的输入。
const NON_HTTP_SCHEMES: &[&str] = &[
    "data",
    "about",
    "javascript",
    "mailto",
    "ftp",
    "ws",
    "wss",
    "blob",
];

/// 取输入的合法 scheme 前缀并归一化为小写（URL Standard scheme 语法：
/// 首字符为字母，其余为字母/数字/`+ - .`，后随 `:`；大小写不敏感）。
/// 不匹配返回 None。
fn leading_scheme(s: &str) -> Option<String> {
    let end = s.find(':')?;
    let scheme = &s[..end];
    if scheme.is_empty() || !scheme.as_bytes()[0].is_ascii_alphabetic() {
        return None;
    }
    scheme
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        .then(|| scheme.to_ascii_lowercase())
}

/// 输入是否指向本机（补全 scheme 用）：取 authority（到首个 `/ ? #` 截断、
/// 去 userinfo、去端口）与 localhost / 127.0.0.1 比对。
fn is_local_host(input: &str) -> bool {
    let end = input.find(['/', '?', '#']).unwrap_or(input.len());
    let authority = &input[..end];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, h)| h)
        .split(':')
        .next()
        .unwrap_or("");
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1"
}

/// HTTP 响应 → 页面文档（Content-Type 分发）。
///
/// 状态码不筛选：4xx/5xx 的正文照常渲染（与浏览器渲染服务器错误页
/// 一致）；charset 依赖 `NetworkResponse::text` 的 UTF-8 lossy 解码。
pub fn document_from_response(resp: &NetworkResponse) -> NavigationDoc {
    let ct = resp
        .header("content-type")
        .unwrap_or("")
        .to_ascii_lowercase();
    let final_url = resp.url.clone();
    if ct.contains("text/html") || ct.is_empty() {
        // 缺 Content-Type 按 HTML 处理（大量服务器对 HTML 页省略）。
        let html = resp.text();
        let css = crate::page::extract_inline_style(&html);
        NavigationDoc {
            final_url,
            html,
            css,
        }
    } else if ct.contains("text/plain") {
        NavigationDoc {
            final_url,
            html: plain_text_page(&resp.text()),
            css: String::new(),
        }
    } else {
        NavigationDoc {
            final_url,
            html: unsupported_type_page(&ct, resp.body_bytes().len()),
            css: String::new(),
        }
    }
}

/// 网络错误页（DNS / 连接 / TLS / 超时 / 体积上限——HTML 文档加载失败，
/// 区别于 HTTP 错误状态）。
pub fn error_page(url: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><body>\
<h1 style=\"font-size:32px;color:#1a1a1a\">Navigation failed</h1>\
<p style=\"font-size:16px\">{}</p>\
<p style=\"font-size:14px\">{}</p></body></html>",
        escape_html(url),
        escape_html(message)
    )
}

/// 转义 `& < >`——URL / 错误消息 / 纯文本进 HTML 前的最小防护。
pub(crate) fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn plain_text_page(text: &str) -> String {
    format!(
        "<!doctype html><html><body><pre style=\"font-size:14px\">{}</pre></body></html>",
        escape_html(text)
    )
}

fn unsupported_type_page(ct: &str, len: usize) -> String {
    format!(
        "<!doctype html><html><body>\
<h1 style=\"font-size:32px;color:#1a1a1a\">Unsupported content</h1>\
<p style=\"font-size:16px\">Content-Type: {} ({} bytes)</p>\
<p style=\"font-size:14px\">MusKitty renders text/html and text/plain only.</p></body></html>",
        escape_html(ct),
        len
    )
}

/// 在独立线程同步抓取 http(s) URL，结果经 channel 回传。
///
/// 每次导航一个线程 + [`muskitty_network::fetch_blocking`] 自建的一次性
/// 运行时——导航是用户级低频事件，不值得为此养常驻执行器。接收端
/// （app 层）关闭后结果静默丢弃；抓取线程 panic 等价于 channel 断开，
/// 由 app 层按"接收器失效"清理。
pub fn spawn_http_navigation(
    url: String,
    tab: usize,
    epoch: u64,
) -> std::sync::mpsc::Receiver<NavigationOutcome> {
    let (tx, rx) = std::sync::mpsc::channel();
    // 失败回退路径留一份克隆：URL 被 move 进抓取线程后错误分支仍要用。
    let failure_url = url.clone();
    let spawned = std::thread::Builder::new()
        .name("muskitty-nav".to_string())
        .spawn(move || {
            let result = match muskitty_network::fetch_blocking(&url) {
                Ok(resp) => Ok(document_from_response(&resp)),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send(NavigationOutcome {
                tab,
                epoch,
                url,
                result,
            });
        });
    // 线程创建失败（资源耗尽）按导航失败处理，不 abort 浏览器进程。
    if let Err(e) = spawned {
        return failed_outcome_channel(
            format!("spawn thread failed: {e}"),
            tab,
            epoch,
            failure_url,
        );
    }
    rx
}

/// 线程创建失败的降级出口：直接在当前线程构造失败结果。
fn failed_outcome_channel(
    message: String,
    tab: usize,
    epoch: u64,
    url: String,
) -> std::sync::mpsc::Receiver<NavigationOutcome> {
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = tx.send(NavigationOutcome {
        tab,
        epoch,
        url,
        result: Err(message),
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- classify_url ----

    #[test]
    fn classify_http_https_passthrough() {
        assert_eq!(
            classify_url("https://example.com/a?b=1"),
            NavigationKind::Http("https://example.com/a?b=1".to_string())
        );
        assert_eq!(
            classify_url("HTTP://Example.COM"),
            NavigationKind::Http("HTTP://Example.COM".to_string())
        );
    }

    #[test]
    fn classify_bare_host_defaults_to_https() {
        assert_eq!(
            classify_url("example.com"),
            NavigationKind::Http("https://example.com".to_string())
        );
        assert_eq!(
            classify_url("  example.com/path  "),
            NavigationKind::Http("https://example.com/path".to_string())
        );
    }

    #[test]
    fn classify_localhost_defaults_to_http() {
        assert_eq!(
            classify_url("localhost:8080/x"),
            NavigationKind::Http("http://localhost:8080/x".to_string())
        );
        assert_eq!(
            classify_url("127.0.0.1"),
            NavigationKind::Http("http://127.0.0.1".to_string())
        );
    }

    #[cfg(windows)]
    #[test]
    fn classify_file_url_strips_drive_slash() {
        assert_eq!(
            classify_url("file:///D:/tmp/page.html"),
            NavigationKind::File("D:/tmp/page.html".to_string())
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn classify_file_url_keeps_absolute_path() {
        assert_eq!(
            classify_url("file:///etc/hosts"),
            NavigationKind::File("/etc/hosts".to_string())
        );
    }

    #[test]
    fn classify_other_scheme_unsupported() {
        assert_eq!(
            classify_url("data:text/html,<b>hi</b>"),
            NavigationKind::Unsupported("data:text/html,<b>hi</b>".to_string())
        );
        assert_eq!(
            classify_url("about:blank"),
            NavigationKind::Unsupported("about:blank".to_string())
        );
        assert_eq!(
            classify_url("JAVASCRIPT:alert(1)"),
            NavigationKind::Unsupported("JAVASCRIPT:alert(1)".to_string())
        );
    }

    #[test]
    fn classify_host_with_port_not_mistaken_for_scheme() {
        // scheme 语法允许含 '.'：host:port 必须走 host 分支而非 scheme 白名单。
        assert_eq!(
            classify_url("example.com:8080/x"),
            NavigationKind::Http("https://example.com:8080/x".to_string())
        );
    }

    #[cfg(windows)]
    #[test]
    fn classify_bare_windows_path_is_file() {
        assert_eq!(
            classify_url(r"D:\tmp\page.html"),
            NavigationKind::File(r"D:\tmp\page.html".to_string())
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn classify_bare_unix_path_is_file() {
        assert_eq!(
            classify_url("/etc/hosts"),
            NavigationKind::File("/etc/hosts".to_string())
        );
    }

    #[test]
    fn classify_empty_is_unsupported() {
        assert_eq!(
            classify_url("   "),
            NavigationKind::Unsupported(String::new())
        );
    }

    // ---- document_from_response ----

    #[test]
    fn document_html_extracts_style() {
        let resp = NetworkResponse::new(
            200,
            vec![(
                "content-type".to_string(),
                "text/html; charset=utf-8".to_string(),
            )],
            "https://example.com/".to_string(),
            b"<style>p{color:red}</style><p>hi</p>".to_vec(),
        );
        let doc = document_from_response(&resp);
        assert_eq!(doc.final_url, "https://example.com/");
        assert!(doc.html.contains("<p>hi</p>"));
        assert!(doc.css.contains("p{color:red}"));
    }

    #[test]
    fn document_missing_content_type_treated_as_html() {
        let resp = NetworkResponse::new(
            200,
            vec![],
            "https://example.com/".to_string(),
            b"<p>implicit html</p>".to_vec(),
        );
        let doc = document_from_response(&resp);
        assert!(doc.html.contains("implicit html"));
    }

    #[test]
    fn document_plain_text_wraps_in_escaped_pre() {
        let resp = NetworkResponse::new(
            200,
            vec![("content-type".to_string(), "text/plain".to_string())],
            "https://example.com/robots.txt".to_string(),
            b"<b>&raw</b>".to_vec(),
        );
        let doc = document_from_response(&resp);
        assert!(doc.html.contains("<pre"));
        assert!(!doc.html.contains("<b>"), "raw markup must be escaped");
        assert!(doc.html.contains("&lt;b&gt;&amp;raw&lt;/b&gt;"));
        assert!(doc.css.is_empty());
    }

    #[test]
    fn document_binary_type_gets_notice_page() {
        let resp = NetworkResponse::new(
            200,
            vec![("content-type".to_string(), "image/png".to_string())],
            "https://example.com/logo.png".to_string(),
            vec![0x89, 0x50, 0x4e, 0x47],
        );
        let doc = document_from_response(&resp);
        assert!(doc.html.contains("Unsupported content"));
        assert!(doc.html.contains("image/png"));
    }

    #[test]
    fn document_4xx_body_still_rendered() {
        let resp = NetworkResponse::new(
            404,
            vec![("content-type".to_string(), "text/html".to_string())],
            "https://example.com/missing".to_string(),
            b"<p>not found page</p>".to_vec(),
        );
        let doc = document_from_response(&resp);
        assert!(doc.html.contains("not found page"));
    }

    #[test]
    fn error_page_escapes_url_and_message() {
        let page = error_page("https://a.b/<x>", "err & <msg>");
        assert!(!page.contains("<x>"));
        assert!(page.contains("&lt;x&gt;"));
        assert!(page.contains("err &amp; &lt;msg&gt;"));
    }

    // ---- 端到端（离线）：原生 TcpListener + 真 reqwest + 真线程 ----

    #[test]
    fn spawn_http_navigation_end_to_end_offline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let (mut sock, _) = listener.accept().expect("accept");
            // 读一次请求（内容不关心），回固定 HTML 响应。
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            let body = "<!doctype html><html><head><style>p{color:red}</style></head>\
<body><p>muskitty-nav-e2e</p></body></html>";
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\n\
content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            sock.write_all(resp.as_bytes()).expect("write");
        });

        let rx = spawn_http_navigation(format!("http://127.0.0.1:{port}/"), 2, 7);
        let outcome = rx
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("outcome");
        server.join().expect("server thread");

        assert_eq!(outcome.tab, 2);
        assert_eq!(outcome.epoch, 7);
        assert!(outcome.url.starts_with("http://127.0.0.1:"));
        let doc = outcome.result.expect("loaded");
        assert!(doc.html.contains("muskitty-nav-e2e"));
        assert!(doc.css.contains("p{color:red}"));
        assert!(doc.final_url.starts_with("http://127.0.0.1:"));
    }

    #[test]
    fn spawn_http_navigation_refused_returns_failed_outcome() {
        // 127.0.0.1:1 与 network crate 的连接拒绝用例同一策略：几乎必然
        // 无监听，快速失败不拖慢测试。
        let rx = spawn_http_navigation("http://127.0.0.1:1/".to_string(), 0, 1);
        let outcome = rx
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("outcome");
        assert_eq!(outcome.url, "http://127.0.0.1:1/");
        assert!(
            outcome.result.is_err(),
            "refused must be Err, got {:?}",
            outcome.result
        );
    }
}
