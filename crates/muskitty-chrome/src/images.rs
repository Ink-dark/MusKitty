//! 背景图子资源接入（BG-1）：DOM 序采集 CSS `url()` → 按其表基准解析 → 抓取解码。
//!
//! 与样式表的 [`crate::stylesheets`] 同构（采集 → 解析 → 抓取 → 去重/上限 →
//! 失败非致命），差别在资源形态与基准：
//!
//! 1. **基准**：每张样式表的 `url()` 以**该表自身**的 `location` 为基准解析
//!    （内嵌 `<style>` 用文档 base）——CSS Backgrounds L3 §2 的 `url()` 语义，
//!    与 `@import` 的"以导入表为基准"同源。
//! 2. **资源形态**：抓取**字节**（不走 CSS 文本解码 / MIME 校验），按 PNG 魔数
//!    解码为 RGBA（renderer 的 [`muskitty_renderer::ImageBits`]）。
//! 3. **key 契约**：产出 `绝对 URL → ImageBits` 表，直接喂给
//!    `PaintInput.images`；renderer 不做 URL 解析。
//!
//! 失败非致命：单个图抓取/解码失败只计入 [`ImageLoadStats`]，该元素不画背景图。
//!
//! # 两步契约（顺序不能颠倒）
//!
//! 1. [`absolutize_image_urls`]：把样式表声明里的 `url()` 就地改写为**绝对
//!    URL**——paint 按声明字符串查表，故此步保证声明与资源表的 key 一致；
//! 2. [`load_images`]：采集（已绝对化）→ 抓取 → PNG 解码 → 资源表。

use std::collections::HashMap;

use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;
use muskitty_cssom::{CssRule, CssStyleSheet};
use muskitty_network::url;
use muskitty_renderer::ImageBits;

/// 图像抓取函数：已解析的绝对 URL → 原始字节（失败为人类可读消息）。
pub type FetchBytesFn<'a> = &'a mut dyn FnMut(&str) -> Result<Vec<u8>, String>;

/// 图像加载上限（**本实现策略**，与样式表上限同风格）。
#[derive(Debug, Clone, Copy)]
pub struct ImageLoadOptions {
    /// 每文档图像数上限（超出的引用跳过）。
    pub max_images: usize,
    /// 单张图像字节上限（超过则跳过解码）。
    pub max_image_bytes: usize,
}

impl Default for ImageLoadOptions {
    fn default() -> Self {
        Self {
            max_images: 64,
            max_image_bytes: 16 * 1024 * 1024,
        }
    }
}

/// 图像加载统计（失败/跳过只观测，不改变渲染结果）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImageLoadStats {
    /// 采集到的 `url()` 引用数**去重后**的条目数（同一绝对 URL 只算一次）。
    pub references: usize,
    /// 成功抓取次数（失败不计入，与样式表 `LoadStats::fetched` 同口径）。
    pub fetched: usize,
    /// 抓取或解码失败数。
    pub failed: usize,
    /// 被上限/不可用值（渐变、`none`、解析失败）跳过的引用数。
    pub skipped: usize,
    /// 成功解码入库的图像数。
    pub decoded: usize,
}

/// 从样式表集合采集 CSS `url()` 并按各表基准解析为绝对 URL。
///
/// 遍历每张表的规则树（含 `@media`/`@layer`/嵌套子规则，与 cascade 的
/// 展平顺序一致），取 `background-image` 与 `background` 简写的图像分量。
/// 返回按**首次出现顺序**去重的绝对 URL 列表（缓存与统计按此顺序）。
///
/// `document_base` 为文档 base URL（内嵌 `<style>`/无 location 的表的基准）。
pub fn collect_image_urls(sheets: &[CssStyleSheet], document_base: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for sheet in sheets {
        let base = sheet.location.as_deref().unwrap_or(document_base);
        collect_from_rules(&sheet.css_rules, base, &mut out);
    }
    out
}

/// 递归遍历规则树收集 `url()`（文档序 = cascade 展平序）。
fn collect_from_rules(rules: &[CssRule], base: &str, out: &mut Vec<String>) {
    for rule in rules {
        match rule {
            CssRule::Style(style_rule) => {
                for decl in &style_rule.style.declarations {
                    let name = decl.name.to_ascii_lowercase();
                    // `background-image` 长属性与 `background` 简写都携带图像分量。
                    if name == "background-image" || name == "background" {
                        for cv in &decl.value {
                            if let Some(raw) = url_from_component_value(cv) {
                                if let Some(abs) = url::resolve(base, &raw) {
                                    if !out.iter().any(|u| u == &abs) {
                                        out.push(abs);
                                    }
                                }
                            }
                        }
                    }
                }
                // 嵌套子规则（CSS nesting）同样携带声明。
                collect_from_rules(&style_rule.css_rules, base, out);
            }
            CssRule::Media(r) => collect_from_rules(&r.css_rules, base, out),
            CssRule::Supports(r) => collect_from_rules(&r.css_rules, base, out),
            CssRule::Container(r) => collect_from_rules(&r.css_rules, base, out),
            CssRule::LayerBlock(r) => collect_from_rules(&r.css_rules, base, out),
            CssRule::Scope(r) => collect_from_rules(&r.css_rules, base, out),
            CssRule::Other(r) => collect_from_rules(&r.child_rules, base, out),
            // @import 已在加载期就地展开（见 stylesheets::expand_imports），
            // 其余 at-rule（@font-face/@page/...）不携带 background-image。
            _ => {}
        }
    }
}

/// 从一个 component value 取出 `url()` 的原始字符串。
///
/// 两种 token 形态（css-syntax §4.3.8）：无引号 `url(x.png)` → `Token::Url`；
/// 带引号 `url("x.png")` → 函数名 `url` + `Token::String` 参数。
/// 渐变函数（`linear-gradient(...)` 等）当前不绘制也不抓取 → `None`。
fn url_from_component_value(cv: &ComponentValue) -> Option<String> {
    match cv {
        ComponentValue::PreservedToken(Token::Url(u)) => Some(u.clone()),
        ComponentValue::Function(f) if f.name.eq_ignore_ascii_case("url") => {
            f.value.iter().find_map(|inner| match inner {
                ComponentValue::PreservedToken(Token::String(s)) => Some(s.clone()),
                _ => None,
            })
        }
        _ => None,
    }
}

/// 把一个 component value 里的 `url()` 就地改写为绝对 URL（其余原样）。
///
/// paint 侧按**声明里的字符串**查图像表，故声明必须与表中的 key 同为绝对
/// 形式——相对 URL 的解析基准（表自身 location / 文档 base）只有加载层知道。
/// 已经是绝对 URL 的引用`resolve` 后不变（URL Standard 的绝对化是幂等的）。
fn absolutize_component_value(cv: &mut ComponentValue, base: &str) {
    match cv {
        ComponentValue::PreservedToken(Token::Url(u)) => {
            if let Some(abs) = url::resolve(base, u) {
                *u = abs;
            }
        }
        ComponentValue::Function(f) if f.name.eq_ignore_ascii_case("url") => {
            for inner in &mut f.value {
                if let ComponentValue::PreservedToken(Token::String(s)) = inner {
                    if let Some(abs) = url::resolve(base, s) {
                        *s = abs;
                    }
                }
            }
        }
        _ => {}
    }
}

/// 就地改写样式表里的 `background-image` / `background` 声明，把 `url()` 换成
/// 绝对 URL（BG-1）。
///
/// 必须在 cascade 之前调用：此后 paint 从 computed style 读到的就是绝对
/// URL，可直接在 [`crate::images::load_images`] 产出的表里查到。
/// 每张表用**自身** `location` 作基准（内嵌 `<style>` 用 `document_base`）——
/// 与 CSS `url()` 的解析语义（CSS Backgrounds L3 §2）及 `@import` 一致。
pub fn absolutize_image_urls(sheets: &mut [CssStyleSheet], document_base: &str) {
    for sheet in sheets.iter_mut() {
        absolutize_rules_for_sheet(sheet, document_base);
    }
}

/// 单张表的绝对化（[`load_stylesheets`](crate::stylesheets::load_stylesheets)
/// 在构造每张表时就地调用，保证所有调用方拿到一致形态）。
pub(crate) fn absolutize_rules_for_sheet(sheet: &mut CssStyleSheet, document_base: &str) {
    let base = sheet
        .location
        .clone()
        .unwrap_or_else(|| document_base.to_string());
    absolutize_rules(&mut sheet.css_rules, &base);
}

/// 递归改写规则树（与 [`collect_from_rules`] 的遍历顺序一致）。
fn absolutize_rules(rules: &mut [CssRule], base: &str) {
    for rule in rules.iter_mut() {
        match rule {
            CssRule::Style(style_rule) => {
                for decl in &mut style_rule.style.declarations {
                    let name = decl.name.to_ascii_lowercase();
                    if name == "background-image" || name == "background" {
                        for cv in &mut decl.value {
                            absolutize_component_value(cv, base);
                        }
                    }
                }
                absolutize_rules(&mut style_rule.css_rules, base);
            }
            CssRule::Media(r) => absolutize_rules(&mut r.css_rules, base),
            CssRule::Supports(r) => absolutize_rules(&mut r.css_rules, base),
            CssRule::Container(r) => absolutize_rules(&mut r.css_rules, base),
            CssRule::LayerBlock(r) => absolutize_rules(&mut r.css_rules, base),
            CssRule::Scope(r) => absolutize_rules(&mut r.css_rules, base),
            CssRule::Other(r) => absolutize_rules(&mut r.child_rules, base),
            _ => {}
        }
    }
}

/// 加载图像资源：采集（已按绝对 URL 去重）→ 抓取 → PNG 解码 →
/// `绝对 URL → ImageBits`。
///
/// 与 [`crate::stylesheets::load_stylesheets`] 同构：单图失败/超限只计入
/// 统计并跳过该 URL，页面其余照常渲染。同一 URL 不会重复抓取——
/// [`collect_image_urls`] 已按解析后的绝对 URL 去重（含 `./a.png` 与
/// `a.png` 解析到同一地址的情形），故此处无需再设缓存。
pub fn load_images(
    sheets: &[CssStyleSheet],
    document_base: &str,
    fetch: FetchBytesFn<'_>,
    opts: &ImageLoadOptions,
) -> (HashMap<String, ImageBits>, ImageLoadStats) {
    let targets = collect_image_urls(sheets, document_base);
    let mut stats = ImageLoadStats {
        references: targets.len(),
        ..ImageLoadStats::default()
    };
    let mut images: HashMap<String, ImageBits> = HashMap::new();
    for target in targets {
        if images.len() >= opts.max_images {
            stats.skipped += 1;
            continue;
        }
        let bytes = match fetch(&target) {
            Ok(bytes) => {
                stats.fetched += 1;
                bytes
            }
            Err(_) => {
                stats.failed += 1;
                continue;
            }
        };
        if bytes.len() > opts.max_image_bytes {
            stats.skipped += 1;
            continue;
        }
        // 解码失败（非 PNG / 截断）按"资源不可用"处理（非致命）。
        match ImageBits::from_png(&bytes) {
            Some(bits) => {
                stats.decoded += 1;
                images.insert(target, bits);
            }
            None => stats.failed += 1,
        }
    }
    (images, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muskitty_css::parse_stylesheet;
    use muskitty_cssom::from_stylesheet_with_origin;

    fn sheet(css: &str, location: Option<&str>) -> CssStyleSheet {
        let mut s =
            from_stylesheet_with_origin(&parse_stylesheet(css), muskitty_cssom::Origin::Author);
        s.location = location.map(str::to_string);
        s
    }

    #[test]
    fn collects_longhand_and_shorthand_urls() {
        let s = sheet(
            "div { background-image: url(a.png) } p { background: url(b.png) red }",
            Some("https://example.com/css/page.css"),
        );
        let urls = collect_image_urls(&[s], "https://example.com/");
        assert_eq!(
            urls,
            vec![
                "https://example.com/css/a.png".to_string(),
                "https://example.com/css/b.png".to_string(),
            ],
            "each url resolves against its own sheet location"
        );
    }

    #[test]
    fn quoted_and_unquoted_forms_both_resolve() {
        let s = sheet(
            r#"div { background-image: url("q.png") } p { background-image: url(u.png) }"#,
            Some("https://example.com/a/b.css"),
        );
        let urls = collect_image_urls(&[s], "https://example.com/");
        assert_eq!(
            urls,
            vec![
                "https://example.com/a/q.png".to_string(),
                "https://example.com/a/u.png".to_string(),
            ]
        );
    }

    #[test]
    fn inline_style_sheet_uses_document_base() {
        // location 为 None（内嵌 <style>）→ 用文档 base 解析。
        let s = sheet("div { background-image: url(i.png) }", None);
        let urls = collect_image_urls(&[s], "https://example.com/dir/");
        assert_eq!(urls, vec!["https://example.com/dir/i.png".to_string()]);
    }

    #[test]
    fn deduplicates_repeated_urls() {
        let s = sheet(
            "div { background: url(same.png) } p { background-image: url(same.png) }",
            Some("https://example.com/x.css"),
        );
        let urls = collect_image_urls(&[s], "https://example.com/");
        assert_eq!(urls.len(), 1, "same URL collected once");
    }

    #[test]
    fn urls_inside_at_rules_are_collected() {
        let s = sheet(
            "@media screen { div { background-image: url(m.png) } }",
            Some("https://example.com/x.css"),
        );
        let urls = collect_image_urls(&[s], "https://example.com/");
        assert_eq!(urls, vec!["https://example.com/m.png".to_string()]);
    }

    #[test]
    fn gradients_and_none_are_not_collected() {
        let s = sheet(
            "div { background-image: linear-gradient(red, blue) } p { background-image: none }",
            Some("https://example.com/x.css"),
        );
        let urls = collect_image_urls(&[s], "https://example.com/");
        assert!(urls.is_empty(), "gradients/none carry no fetchable URL");
    }

    /// 2x2 红蓝格 PNG。
    fn sample_png() -> Vec<u8> {
        let mut pixmap = tiny_skia::Pixmap::new(2, 2).unwrap();
        for y in 0..2u32 {
            for x in 0..2u32 {
                let c = if (x + y) % 2 == 0 {
                    tiny_skia::Color::from_rgba8(255, 0, 0, 255)
                } else {
                    tiny_skia::Color::from_rgba8(0, 0, 255, 255)
                };
                let u8c = c.premultiply().to_color_u8();
                pixmap.pixels_mut()[(y * 2 + x) as usize] =
                    tiny_skia::PremultipliedColorU8::from_rgba(
                        u8c.red(),
                        u8c.green(),
                        u8c.blue(),
                        u8c.alpha(),
                    )
                    .unwrap();
            }
        }
        pixmap.encode_png().unwrap()
    }

    #[test]
    fn load_images_fetches_decodes_and_dedupes() {
        let png = sample_png();
        let s = sheet(
            "div { background: url(a.png) } p { background: url(a.png) } q { background: url(b.png) }",
            Some("https://example.com/x.css"),
        );
        let mut calls = 0usize;
        let (images, stats) = load_images(
            &[s],
            "https://example.com/",
            &mut |target: &str| {
                calls += 1;
                if target.ends_with("a.png") {
                    Ok(png.clone())
                } else {
                    Err("404".to_string())
                }
            },
            &ImageLoadOptions::default(),
        );
        assert_eq!(calls, 2, "a.png deduped to one fetch, b.png once");
        assert_eq!(stats.references, 2);
        assert_eq!(stats.fetched, 1, "only a.png fetched successfully");
        assert_eq!(stats.decoded, 1);
        assert_eq!(stats.failed, 1, "b.png fetch failure is non-fatal");
        assert_eq!(images.len(), 1);
        let bits = images
            .get("https://example.com/a.png")
            .expect("decoded image");
        assert_eq!((bits.width, bits.height), (2, 2));
    }

    #[test]
    fn load_images_decode_failure_is_non_fatal() {
        // 抓取成功但不是 PNG → 解码失败，不产生条目（页面照常渲染）。
        let s = sheet(
            "div { background: url(a.png) }",
            Some("https://example.com/x.css"),
        );
        let (images, stats) = load_images(
            &[s],
            "https://example.com/",
            &mut |_: &str| Ok(b"not a png".to_vec()),
            &ImageLoadOptions::default(),
        );
        assert_eq!(stats.fetched, 1);
        assert_eq!(stats.failed, 1);
        assert!(images.is_empty(), "undecodable bytes yield no image");
    }

    #[test]
    fn load_images_respects_max_bytes() {
        let png = sample_png();
        let s = sheet(
            "div { background: url(a.png) }",
            Some("https://example.com/x.css"),
        );
        let (images, stats) = load_images(
            &[s],
            "https://example.com/",
            &mut |_: &str| Ok(png.clone()),
            &ImageLoadOptions {
                max_image_bytes: 8,
                ..ImageLoadOptions::default()
            },
        );
        assert_eq!(stats.skipped, 1, "oversized image skipped");
        assert!(images.is_empty());
    }
}
