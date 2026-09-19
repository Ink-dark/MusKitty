//! BG-1 离线端到端：背景图子资源全链路（采集 → 按表基准解析 → 抓取 →
//! PNG 解码 → paint → 像素）。
//!
//! 覆盖三条：`data:` URL 图生效 / `file://` 相对路径图生效（临时目录 fixture）/
//! http 页引用 `file://` 图被 scheme 策略拒绝（背景色照常绘制）。
//! 全部离线、无外网依赖。

use std::collections::HashMap;

use muskitty_chrome::images::{load_images, ImageLoadOptions};
use muskitty_chrome::page::render_page_with_images;
use muskitty_chrome::stylesheets::{author_sheet, DocumentFetcher};
use muskitty_renderer::RenderOutput;

/// 1x1 纯色 PNG（测试内编码）。
fn one_pixel_png(r: u8, g: u8, b: u8) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(1, 1).unwrap();
    let c = tiny_skia::Color::from_rgba8(r, g, b, 255);
    let u8c = c.premultiply().to_color_u8();
    pixmap.pixels_mut()[0] =
        tiny_skia::PremultipliedColorU8::from_rgba(u8c.red(), u8c.green(), u8c.blue(), u8c.alpha())
            .unwrap();
    pixmap.encode_png().unwrap()
}

/// PNG 字节 → `data:image/png;base64,` URL（手写 base64，避免新增依赖）。
fn png_data_url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::from("data:image/png;base64,");
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(n >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

/// 渲染并取 (x, y) 处 RGBA。
fn pixel_at(
    html: &str,
    sheets: &[muskitty_cssom::CssStyleSheet],
    images: &HashMap<String, muskitty_renderer::ImageBits>,
    x: u32,
    y: u32,
) -> (u8, u8, u8, u8) {
    let out = render_page_with_images(html, sheets, images, 60, 40, 1.0).expect("render");
    let RenderOutput::Pixels { width, data, .. } = out else {
        panic!("expected pixels");
    };
    let i = ((y * width + x) * 4) as usize;
    (data[i], data[i + 1], data[i + 2], data[i + 3])
}

const BOX_HTML: &str =
    r#"<!doctype html><html><body><div style="width:40px;height:20px"></div></body></html>"#;

#[test]
fn data_url_background_image_applies() {
    // `data:image/png;base64,...` 背景图：解码 → paint → 盒内为图的颜色。
    let png = one_pixel_png(0, 128, 255);
    let css = format!(
        "body{{margin:0}} div{{display:block;width:40px;height:20px;background-image:url(\"{}\")}}",
        png_data_url(&png)
    );
    let mut sheet = author_sheet(&css);
    // 契约：先绝对化声明里的 url()，再按表收集抓取。
    muskitty_chrome::images::absolutize_image_urls(
        std::slice::from_mut(&mut sheet),
        "file:///tmp/page.html",
    );
    let (images, stats) = load_images(
        std::slice::from_ref(&sheet),
        "file:///tmp/page.html",
        &mut |url| {
            // data: 不经网络；这里直接复用 fetcher 的字节路径。
            DocumentFetcher::new("file:///tmp/page.html").fetch_bytes(url)
        },
        &ImageLoadOptions::default(),
    );
    assert_eq!(stats.decoded, 1, "data: PNG must decode: {stats:?}");
    assert_eq!(stats.failed, 0);
    assert_eq!(
        pixel_at(BOX_HTML, &[sheet], &images, 20, 10),
        (0, 128, 255, 255),
        "data: URL background image must paint in the box"
    );
}

#[test]
fn file_relative_background_image_applies() {
    // file:// 页面 + 同目录相对路径背景图：以样式表自身 location 为基准。
    let dir = std::env::temp_dir().join("muskitty_bg_image_fixture");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("img")).unwrap();
    std::fs::write(dir.join("img").join("tile.png"), one_pixel_png(0, 200, 0)).unwrap();
    std::fs::write(
        dir.join("page.css"),
        "body{margin:0} div{display:block;width:40px;height:20px;background-image:url(img/tile.png)}",
    )
    .unwrap();
    let page_url =
        muskitty_network::url::file_url_from_path(&dir.join("page.html").to_string_lossy())
            .expect("file url");
    let css_url =
        muskitty_network::url::file_url_from_path(&dir.join("page.css").to_string_lossy())
            .expect("file url");

    let css = std::fs::read_to_string(dir.join("page.css")).unwrap();
    let mut sheet = author_sheet(&css);
    sheet.location = Some(css_url);
    muskitty_chrome::images::absolutize_image_urls(std::slice::from_mut(&mut sheet), &page_url);

    let mut fetcher = DocumentFetcher::new(&page_url);
    let (images, stats) = load_images(
        std::slice::from_ref(&sheet),
        &page_url,
        &mut |url| fetcher.fetch_bytes(url),
        &ImageLoadOptions::default(),
    );
    assert_eq!(stats.decoded, 1, "relative file image must load: {stats:?}");
    assert_eq!(
        pixel_at(BOX_HTML, &[sheet], &images, 20, 10),
        (0, 200, 0, 255),
        "file:// relative background image must paint"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn http_page_cannot_load_file_background_image() {
    // scheme 策略：http 文档不得读 `file://`（与样式表同规则）。图像被拒后
    // 该元素不画背景图，但背景色照常绘制（失败非致命）。
    let css = "body{margin:0} div{display:block;width:40px;height:20px;\
               background-color:#ff0000;background-image:url(\"file:///D:/secret.png\")}";
    let mut sheet = author_sheet(css);
    sheet.location = Some("http://example.com/css/site.css".to_string());
    muskitty_chrome::images::absolutize_image_urls(
        std::slice::from_mut(&mut sheet),
        "http://example.com/page.html",
    );

    let page_url = "http://example.com/page.html";
    let mut fetcher = DocumentFetcher::new(page_url);
    let (images, stats) = load_images(
        std::slice::from_ref(&sheet),
        page_url,
        &mut |url| fetcher.fetch_bytes(url),
        &ImageLoadOptions::default(),
    );
    assert!(
        images.is_empty(),
        "file:// must be blocked for an http page"
    );
    assert_eq!(stats.failed, 1, "blocked fetch counts as failed: {stats:?}");
    assert_eq!(
        pixel_at(BOX_HTML, &[sheet], &images, 20, 10),
        (255, 0, 0, 255),
        "background-color must still paint when the image is blocked"
    );
}
