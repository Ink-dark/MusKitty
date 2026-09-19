//! CS-1 离线端到端：外链样式表全链路（真 reqwest → 导航线程采集/抓取 →
//! cascade → 像素）。
//!
//! 每条用例起一个原生 `TcpListener` 迷你静态服务器（单线程、按路径查表、
//! `connection: close`），断言最终 RGBA 像素——离线、确定性、无外网依赖。
//! 覆盖规划文档 §六的七条：相对路径 / 后表胜出 / 404 非致命 /
//! `media="print"` 跳过 / `@import` 链以导入表为基准 / 成环不挂死 /
//! `data:` 表，另加 `<link disabled>`。

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use muskitty_chrome::navigation::{spawn_http_navigation, NavigationDoc};
use muskitty_chrome::page::render_page_with_sheets;
use muskitty_renderer::RenderOutput;

/// 一条静态响应。
struct Route {
    content_type: &'static str,
    body: &'static str,
}

/// 迷你静态服务器句柄（drop 时停线程）。
struct Server {
    base: String,
    stop: Arc<AtomicBool>,
    port: u16,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // 唤醒阻塞在 accept 的循环（发一个立即关闭的连接）。
        let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// 起服务器；未命中的路径返回 404 `text/plain`。
fn start_server(routes: Vec<(&'static str, Route)>) -> Server {
    let routes: HashMap<String, Route> = routes
        .into_iter()
        .map(|(path, route)| (path.to_string(), route))
        .collect();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let join = std::thread::spawn(move || {
        for stream in listener.incoming() {
            if stop_thread.load(Ordering::SeqCst) {
                break;
            }
            let Ok(mut stream) = stream else { continue };
            let Ok(reader_stream) = stream.try_clone() else {
                continue;
            };
            let mut reader = BufReader::new(reader_stream);
            // 请求行：`GET /path HTTP/1.1`
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                continue;
            }
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .split(['?', '#'])
                .next()
                .unwrap_or("/")
                .to_string();
            // 丢弃其余请求头（读到空行为止）。
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if line == "\r\n" || line == "\n" => break,
                    Ok(_) => {}
                }
            }
            let response = match routes.get(&path) {
                Some(route) => format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    route.content_type,
                    route.body.len(),
                    route.body
                ),
                None => "HTTP/1.1 404 Not Found\r\ncontent-type: text/plain\r\ncontent-length: 9\r\nconnection: close\r\n\r\nnot found".to_string(),
            };
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    Server {
        base: format!("http://127.0.0.1:{port}"),
        stop,
        port,
        join: Some(join),
    }
}

fn html_route(body: &'static str) -> Route {
    Route {
        content_type: "text/html; charset=utf-8",
        body,
    }
}

fn css_route(body: &'static str) -> Route {
    Route {
        content_type: "text/css",
        body,
    }
}

/// 走真导航线程加载页面（含样式表），返回文档。
fn navigate(base: &str, path: &str) -> NavigationDoc {
    let rx = spawn_http_navigation(format!("{base}{path}"), 0, 1);
    let outcome = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("navigation outcome");
    outcome.result.expect("navigation ok")
}

/// 渲染文档并取 (x, y) 处 RGBA。
fn pixel_at(doc: &NavigationDoc, x: u32, y: u32) -> (u8, u8, u8, u8) {
    let out = render_page_with_sheets(&doc.html, &doc.sheets, 200, 100, 1.0).expect("render");
    let RenderOutput::Pixels { width, data, .. } = out else {
        panic!("expected pixels");
    };
    let i = ((y * width + x) * 4) as usize;
    (data[i], data[i + 1], data[i + 2], data[i + 3])
}

const PAGE: &str = r#"<!doctype html><html><head>
<link rel="stylesheet" href="/css/site.css">
</head><body><div></div></body></html>"#;

const RED_DIV_CSS: &str =
    "body{margin:0} div{display:block;width:100px;height:50px;background-color:#ff0000}";

#[test]
fn external_stylesheet_applies_via_relative_path() {
    let server = start_server(vec![
        ("/page.html", html_route(PAGE)),
        ("/css/site.css", css_route(RED_DIV_CSS)),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(doc.final_url, format!("{}/page.html", server.base));
    assert_eq!(doc.sheets.len(), 1);
    assert_eq!(
        doc.sheets[0].location.as_deref(),
        Some(format!("{}/css/site.css", server.base).as_str())
    );
    assert_eq!(doc.stats.fetched, 1);
    assert_eq!(doc.stats.failed, 0);
    assert_eq!(pixel_at(&doc, 10, 10), (255, 0, 0, 255), "外链 CSS 生效");
}

#[test]
fn later_stylesheet_wins_at_equal_specificity() {
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/first.css">
<link rel="stylesheet" href="/second.css">
</head><body><div></div></body></html>"#,
            ),
        ),
        ("/first.css", css_route(RED_DIV_CSS)),
        ("/second.css", css_route("div{background-color:#0000ff}")),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(doc.sheets.len(), 2);
    assert_eq!(pixel_at(&doc, 10, 10), (0, 0, 255, 255), "后出现的表胜出");
}

#[test]
fn missing_stylesheet_is_non_fatal() {
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/missing.css">
<link rel="stylesheet" href="/site.css">
</head><body><div></div></body></html>"#,
            ),
        ),
        ("/site.css", css_route(RED_DIV_CSS)),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(doc.sheets.len(), 1, "404 表跳过，其余表照常");
    assert_eq!(doc.stats.failed, 1);
    assert_eq!(pixel_at(&doc, 10, 10), (255, 0, 0, 255));
}

#[test]
fn media_print_stylesheet_is_not_applied() {
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/print.css" media="print">
<link rel="stylesheet" href="/screen.css" media="screen">
</head><body><div></div></body></html>"#,
            ),
        ),
        (
            "/print.css",
            css_route("body{margin:0} div{display:block;width:100px;height:50px;background-color:#00ff00}"),
        ),
        ("/screen.css", css_route(RED_DIV_CSS)),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(doc.sheets.len(), 2, "两张表都加载（media 求值在 cascade）");
    assert_eq!(
        pixel_at(&doc, 10, 10),
        (255, 0, 0, 255),
        "media=print 不生效，media=screen 生效"
    );
}

#[test]
fn disabled_stylesheet_is_not_applied() {
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/disabled.css" disabled>
<link rel="stylesheet" href="/site.css">
</head><body><div></div></body></html>"#,
            ),
        ),
        (
            "/disabled.css",
            css_route("body{margin:0} div{display:block;width:100px;height:50px;background-color:#00ff00}"),
        ),
        ("/site.css", css_route(RED_DIV_CSS)),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(
        pixel_at(&doc, 10, 10),
        (255, 0, 0, 255),
        "disabled 表不生效"
    );
}

#[test]
fn import_chain_resolves_against_importing_sheet() {
    // a.css 在 /css/ 下，其 @import "sub/b.css" 必须相对 **a.css** 解析
    // （而非文档 URL）→ /css/sub/b.css。
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/css/a.css">
</head><body><div></div></body></html>"#,
            ),
        ),
        (
            "/css/a.css",
            css_route("@import url(\"sub/b.css\");\ndiv{background-color:#ff0000}"),
        ),
        (
            "/css/sub/b.css",
            css_route("body{margin:0} div{display:block;width:100px;height:50px;background-color:#0000ff}"),
        ),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(doc.stats.imports, 1);
    assert_eq!(doc.stats.failed, 0);
    // b.css 的 div 规则与 a.css 的 div 规则等特异性 → 展开位置在先（import
    // 在 a.css 顶部），故 a.css 的红色胜出；几何（width/height/margin）来自 b.css。
    assert_eq!(pixel_at(&doc, 10, 10), (255, 0, 0, 255));
}

#[test]
fn import_cycle_terminates_within_timeout() {
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/a.css">
</head><body><div></div></body></html>"#,
            ),
        ),
        (
            "/a.css",
            css_route("@import url(\"b.css\");\nbody{margin:0} div{display:block;width:100px;height:50px;background-color:#ff0000}"),
        ),
        (
            "/b.css",
            css_route("@import url(\"a.css\");\ndiv{background-color:#0000ff}"),
        ),
    ]);
    // 导航本身带 30s 超时；成环若未终止会在这里 panic（recv_timeout）。
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(
        doc.stats.imports, 1,
        "只有 a→b 展开一次，b→a 被循环检测拦下"
    );
    assert!(doc.stats.skipped >= 1);
    assert_eq!(pixel_at(&doc, 10, 10), (255, 0, 0, 255));
}

#[test]
fn data_url_stylesheet_applies() {
    let server = start_server(vec![(
        "/page.html",
        html_route(
            r#"<!doctype html><html><head>
<link rel="stylesheet" href="data:text/css,body%7Bmargin%3A0%7D%20div%7Bdisplay%3Ablock%3Bwidth%3A100px%3Bheight%3A50px%3Bbackground-color%3A%2300cc00%7D">
</head><body><div></div></body></html>"#,
        ),
    )]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(doc.sheets.len(), 1);
    assert_eq!(doc.stats.failed, 0);
    assert_eq!(pixel_at(&doc, 10, 10), (0, 204, 0, 255), "data: 表生效");
}

#[test]
fn import_inside_style_element_resolves_against_document() {
    // 内嵌 <style> 没有自身 URL：其 @import 相对**文档 base** 解析。
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<style>@import url("more.css");</style>
</head><body><div></div></body></html>"#,
            ),
        ),
        ("/more.css", css_route(RED_DIV_CSS)),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(doc.stats.imports, 1);
    assert_eq!(doc.sheets.len(), 1);
    assert_eq!(pixel_at(&doc, 10, 10), (255, 0, 0, 255));
}

#[test]
fn media_attribute_feature_query_gates_the_sheet() {
    // MQ-V：`media` 属性不只是 ident——特性查询同样驱动整表门控。
    // 渲染视口 200×100 → `(min-width: 300px)` 不命中、`(max-width: 300px)` 命中。
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/wide.css" media="(min-width: 300px)">
<link rel="stylesheet" href="/narrow.css" media="(max-width: 300px)">
</head><body><div></div></body></html>"#,
            ),
        ),
        (
            "/wide.css",
            css_route("body{margin:0} div{display:block;width:100px;height:50px;background-color:#00ff00}"),
        ),
        ("/narrow.css", css_route(RED_DIV_CSS)),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(
        doc.sheets.len(),
        2,
        "both sheets load; media is evaluated in cascade"
    );
    assert_eq!(
        pixel_at(&doc, 10, 10),
        (255, 0, 0, 255),
        "the max-width sheet matches the 200px render viewport, the min-width one does not"
    );
}

#[test]
fn media_attribute_orientation_query_gates_the_sheet() {
    // `(orientation: landscape)` 在 200×100 视口成立、(portrait) 不成立。
    let server = start_server(vec![
        (
            "/page.html",
            html_route(
                r#"<!doctype html><html><head>
<link rel="stylesheet" href="/portrait.css" media="(orientation: portrait)">
<link rel="stylesheet" href="/landscape.css" media="(orientation: landscape)">
</head><body><div></div></body></html>"#,
            ),
        ),
        (
            "/portrait.css",
            css_route("body{margin:0} div{display:block;width:100px;height:50px;background-color:#00ff00}"),
        ),
        ("/landscape.css", css_route(RED_DIV_CSS)),
    ]);
    let doc = navigate(&server.base, "/page.html");
    assert_eq!(pixel_at(&doc, 10, 10), (255, 0, 0, 255));
}
