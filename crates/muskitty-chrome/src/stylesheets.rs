//! 样式表来源接入（CS-1）：HTML 文档序采集 + 外链抓取 + `@import` 展开。
//!
//! 覆盖四件事（规范行号见
//! [docs/plans/2026-09-13-external-css-and-css-sources.md](../../../docs/plans/2026-09-13-external-css-and-css-sources.md) §五）：
//!
//! 1. **采集**（[`collect_sheet_sources`]）：DOM 先序遍历 = 文档序，`<style>` 取
//!    `textContent`（HTML §4.2.6），`<link rel=stylesheet>` 按 §4.2.4 的
//!    `rel` 词表 / `type` / `media` / `title` / `disabled` 语义处理，首个可解析的
//!    `<base href>` 生效（§4.2.3）。
//! 2. **抓取**（[`DocumentFetcher`]）：http(s) 走网络层（Content-Type 必须
//!    `text/css`，非 2xx 视为失败——CSSOM "fetch a CSS style sheet"）；
//!    file:// 本地读（无 Content-Type → 默认类型 text/css）；`data:` 解码；
//!    scheme 策略由 `muskitty_network::url::is_fetchable_subresource` 把关。
//! 3. **上限与去重**（[`LoadOptions`]）：单表字节数 / 每文档表数 / `@import`
//!    深度；同一 URL 只抓一次（失败也缓存，不重试）。
//! 4. **`@import` 展开**（[`Loader::expand_imports`]）：加载期按 CSS Cascade L5
//!    §"Importing Style Sheets" 的 in-place 语义把 import 替换为被导入表的规则；
//!    条件导入包一层 `@media` 复用既有求值；非法位置 / 循环 / 深度超限 / 抓取失败
//!    一律跳过该 import（页面其余照常）。
//!
//! 失败不致命：单个样式表抓取失败只计入 [`LoadStats`]，不影响文档渲染。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use muskitty_css::parse_comma_separated_list_of_component_values;
use muskitty_css::parser::ComponentValue;
use muskitty_css::tokenizer::Token;
use muskitty_cssom::{
    from_stylesheet, from_stylesheet_with_origin, CssMediaRule, CssRule, CssStyleSheet, Origin,
};
use muskitty_dom::{Namespace, Node};
use muskitty_network::url;

/// 抓取函数：已解析的绝对 URL → 样式表文本（失败为人类可读消息）。
pub type FetchFn<'a> = &'a mut dyn FnMut(&str) -> Result<String, String>;

/// 加载上限（**本实现策略**：浏览器无此类硬限，靠内存与超时兜底）。
#[derive(Debug, Clone, Copy)]
pub struct LoadOptions {
    /// 文档内样式表总数上限（内嵌 + 外链 + `@import` 展开出的表）。
    pub max_sheets: usize,
    /// 单张样式表文本字节上限（超过则整表跳过）。
    pub max_sheet_bytes: usize,
    /// `@import` 嵌套深度上限（顶层表的 import 深度为 1）。
    pub max_import_depth: usize,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            max_sheets: 64,
            max_sheet_bytes: 8 * 1024 * 1024,
            max_import_depth: 16,
        }
    }
}

/// 加载统计（失败/跳过只观测，不改变渲染结果）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LoadStats {
    /// 采集到的来源数（`<style>` + `<link rel=stylesheet>`）。
    pub sources: usize,
    /// 实际发起的抓取次数（不含缓存命中）。
    pub fetched: usize,
    /// 缓存命中次数（同一 URL 的重复引用；失败结果也命中）。
    pub cache_hits: usize,
    /// 抓取失败的表数。
    pub failed: usize,
    /// 被策略/上限/位置规则跳过的表或 import 数。
    pub skipped: usize,
    /// 成功展开的 `@import` 数。
    pub imports: usize,
}

/// 一张样式表来源（文档序）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetSource {
    /// 解析后的绝对 URL（内嵌 `<style>` 为 `None`）。
    pub location: Option<String>,
    /// `media` 属性原文（空串 = 无媒体条件）。
    pub media: String,
    /// `title` 属性（CSS style sheet set name，本轮只记录）。
    pub title: String,
    /// `rel="alternate stylesheet"`：未被显式启用 → 不生效（cascade 按 disabled 处理）。
    pub alternate: bool,
    /// `<link disabled>`（HTML §4.2.4 L754-760）。
    pub disabled: bool,
    /// 样式表文本（内嵌为 `<style>` 内容；外链抓取后填充）。
    pub css: String,
}

/// 采集结果：来源列表 + 生效的文档 base URL。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectedSheets {
    /// `<base href>` 生效后的解析基准（无 `<base>` 时 = 文档 URL）。
    pub base_url: String,
    /// 文档序来源。
    pub sources: Vec<SheetSource>,
}

/// `<link rel>` 词表拆分（HTML §4.2.4：空格分隔、ASCII 大小写不敏感）。
fn rel_tokens(rel: &str) -> impl Iterator<Item = &str> {
    rel.split_ascii_whitespace()
}

/// `rel` 是否含某关键词。
fn rel_has(rel: &str, token: &str) -> bool {
    rel_tokens(rel).any(|t| t.eq_ignore_ascii_case(token))
}

/// MIME 类型是否为 `text/css`（忽略参数，如 `text/css; charset=utf-8`）。
fn is_css_mime(mime: &str) -> bool {
    mime.split(';')
        .next()
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("text/css")
}

/// 文档 base URL：树序中**第一个** `href` 可解析的 `<base>`（HTML §4.2.3）。
///
/// 偏差记录：规范要求 base 只影响其**之后**出现的 URL；这里统一按最终 base
/// 解析全部来源（作者实践里 `<base>` 在 head 首位，差异只在病态文档出现）。
fn document_base_url(dom: &Rc<RefCell<Node>>, document_url: &str) -> String {
    for node in Node::descendants(dom) {
        let node_ref = node.borrow();
        let Some(el) = node_ref.kind.as_element() else {
            continue;
        };
        if el.namespace != Namespace::Html || el.local_name != "base" {
            continue;
        }
        if let Some(href) = el.get_attribute("href") {
            if let Some(resolved) = url::resolve(document_url, href) {
                return resolved;
            }
        }
    }
    document_url.to_string()
}

/// 按 HTML 文档序采集样式表来源（`<style>` 与 `<link rel=stylesheet>`）。
///
/// - 只认 HTML 命名空间元素（SVG/MathML 内的 `<style>`/`<link>` 不在本轮范围）；
/// - `<link>`：`rel` 含 `stylesheet` 才收集；`type` 存在且非 `text/css` 则**不抓**
///   （§4.2.4 L849 类型提示）；`href` 缺失/空白/解析失败则忽略；`alternate` /
///   `disabled` / `media` / `title` 原样带去下一阶段；
/// - `<style>`：`textContent` 即 CSS 源码（RAWTEXT，无实体解码）。
pub fn collect_sheet_sources(dom: &Rc<RefCell<Node>>, document_url: &str) -> CollectedSheets {
    let base_url = document_base_url(dom, document_url);
    let mut sources = Vec::new();
    for node in Node::descendants(dom) {
        let node_ref = node.borrow();
        let Some(el) = node_ref.kind.as_element() else {
            continue;
        };
        if el.namespace != Namespace::Html {
            continue;
        }
        let media = el.get_attribute("media").unwrap_or("").to_string();
        let title = el.get_attribute("title").unwrap_or("").to_string();
        match el.local_name.as_str() {
            "style" => {
                sources.push(SheetSource {
                    location: None,
                    media,
                    title,
                    alternate: false,
                    disabled: false,
                    css: node_ref.text_content().unwrap_or_default(),
                });
            }
            "link" => {
                let rel = el.get_attribute("rel").unwrap_or("");
                if !rel_has(rel, "stylesheet") {
                    continue;
                }
                if let Some(ty) = el.get_attribute("type") {
                    if !is_css_mime(ty) {
                        continue;
                    }
                }
                let Some(href) = el
                    .get_attribute("href")
                    .map(str::trim)
                    .filter(|h| !h.is_empty())
                else {
                    continue;
                };
                let Some(location) = url::resolve(&base_url, href) else {
                    continue;
                };
                sources.push(SheetSource {
                    location: Some(location),
                    media,
                    title,
                    alternate: rel_has(rel, "alternate"),
                    disabled: el.has_attribute("disabled"),
                    css: String::new(),
                });
            }
            _ => {}
        }
    }
    CollectedSheets { base_url, sources }
}

/// 样式表文本 → CSSOM 规则列表（`@charset` 已被解析器丢弃，见 css-parser 修复）。
fn parse_rules(css: &str) -> Vec<CssRule> {
    from_stylesheet(&muskitty_css::parse_stylesheet(css)).css_rules
}

/// 是否有本轮不支持的 `@import` 前缀（`layer`/`layer()`/`supports()`）。
///
/// 规范（CSS Cascade L5 §"Importing Style Sheets" L119-176）把 `layer()` 与
/// `supports()` 放在 url 与 media 之间；本实现读法的偏差记录：含这两种前缀的
/// import **整条跳过**（宁缺勿错——按 media 求值会让 `layer(a)` 被误判为不匹配
/// 而丢弃整张表，supports 不匹配时规范还要求不得抓取）。
fn has_unsupported_prefix(media: &[ComponentValue]) -> bool {
    media.iter().any(|cv| match cv {
        ComponentValue::Function(f) => {
            f.name.eq_ignore_ascii_case("layer") || f.name.eq_ignore_ascii_case("supports")
        }
        ComponentValue::PreservedToken(Token::Ident(name)) => {
            name.eq_ignore_ascii_case("layer") || name.eq_ignore_ascii_case("supports")
        }
        _ => false,
    })
}

/// 加载器：抓取缓存 + 上限 + `@import` 展开。
struct Loader<'a> {
    fetch: FetchFn<'a>,
    opts: LoadOptions,
    /// URL → 文本（`None` = 该 URL 抓取失败，重复引用不再重试）。
    cache: HashMap<String, Option<String>>,
    /// 已产出的表数（顶层 + 展开出的），与 `opts.max_sheets` 比对。
    sheets_created: usize,
    stats: LoadStats,
}

impl Loader<'_> {
    /// 抓取（带缓存）。失败只记一次并返回 `None`。
    fn fetch_text(&mut self, target: &str) -> Option<String> {
        if let Some(cached) = self.cache.get(target) {
            self.stats.cache_hits += 1;
            return cached.clone();
        }
        let result = match (self.fetch)(target) {
            Ok(text) => {
                self.stats.fetched += 1;
                Some(text)
            }
            Err(_) => {
                self.stats.failed += 1;
                None
            }
        };
        self.cache.insert(target.to_string(), result.clone());
        result
    }

    /// 展开一张表的顶层 `@import`（就地替换，CSS Cascade L5 L95-113）。
    ///
    /// `base` = 当前表的 URL（内嵌表用文档 base）；`stack` = 展开栈（循环检测）。
    fn expand_imports(
        &mut self,
        rules: Vec<CssRule>,
        base: &str,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> Vec<CssRule> {
        let mut out: Vec<CssRule> = Vec::with_capacity(rules.len());
        // 合法位置：@import 必须位于其他有效 at-rule 与 style rule 之前
        // （忽略 @charset 与 @layer 语句；CSS Cascade L5 L115-118）。
        let mut imports_allowed = true;
        for rule in rules {
            match rule {
                CssRule::Import(import) => {
                    if !imports_allowed || has_unsupported_prefix(&import.media) {
                        self.stats.skipped += 1;
                        continue;
                    }
                    let Some(target) = url::resolve(base, &import.href) else {
                        self.stats.skipped += 1;
                        continue;
                    };
                    if stack.iter().any(|u| u == &target) {
                        self.stats.skipped += 1;
                        continue;
                    }
                    if depth > self.opts.max_import_depth
                        || self.sheets_created >= self.opts.max_sheets
                    {
                        self.stats.skipped += 1;
                        continue;
                    }
                    let Some(text) = self.fetch_text(&target) else {
                        continue;
                    };
                    if text.len() > self.opts.max_sheet_bytes {
                        self.stats.skipped += 1;
                        continue;
                    }
                    self.sheets_created += 1;
                    self.stats.imports += 1;
                    stack.push(target.clone());
                    let expanded =
                        self.expand_imports(parse_rules(&text), &target, depth + 1, stack);
                    stack.pop();
                    if import.media.is_empty() {
                        out.extend(expanded);
                    } else {
                        // 条件导入 = 被包在 @media 里（L178-200），复用 cascade 既有求值。
                        out.push(CssRule::Media(CssMediaRule {
                            condition: import.media,
                            css_rules: expanded,
                        }));
                    }
                }
                // @layer 语句不影响 import 的合法位置判定。
                CssRule::LayerStatement(rule) => out.push(CssRule::LayerStatement(rule)),
                other => {
                    imports_allowed = false;
                    out.push(other);
                }
            }
        }
        out
    }
}

/// `media` 属性 → cascade 求值用的**扁平** media query 列表。
///
/// `parse_comma_separated_list_of_component_values` 按逗号分组返回；cascade 的
/// `eval_media_query` 期望用 `Token::Comma` 分隔的扁平列表（cascade `split_on_commas`），
/// 故在组间补回逗号 token。
fn parse_media_list(attr: &str) -> Vec<ComponentValue> {
    let groups = parse_comma_separated_list_of_component_values(attr);
    let mut out = Vec::new();
    for (i, mut group) in groups.into_iter().enumerate() {
        if i > 0 {
            out.push(ComponentValue::PreservedToken(Token::Comma));
        }
        out.append(&mut group);
    }
    out
}

/// CSS 文本 → 单张 Author 样式表（便捷构造；无 media/title/location）。
///
/// 单表入口（`page::render_page`）与测试用；多表 + 外链路径走
/// [`load_stylesheets`]。
pub fn author_sheet(css: &str) -> CssStyleSheet {
    from_stylesheet_with_origin(&muskitty_css::parse_stylesheet(css), Origin::Author)
}

/// 一张来源 → CSSOM 样式表（origin = Author；sheet 级字段直接落位）。
fn build_sheet(source: &SheetSource, css_rules: Vec<CssRule>) -> CssStyleSheet {
    CssStyleSheet {
        origin: Origin::Author,
        location: source.location.clone(),
        media: if source.media.is_empty() {
            Vec::new()
        } else {
            parse_media_list(&source.media)
        },
        title: source.title.clone(),
        alternate: source.alternate,
        disabled: source.disabled,
        css_rules,
    }
}

/// 采集 + 抓取 + `@import` 展开 → 文档序样式表 + 统计（CS-1 主入口）。
///
/// `fetch` 必须是**已解析绝对 URL → 文本**的抓取器（见 [`DocumentFetcher`]）；
/// 调用时机决定阻塞语义：http(s) 文档在导航线程内调用，file 文档在加载点同步调用。
pub fn load_stylesheets(
    dom: &Rc<RefCell<Node>>,
    document_url: &str,
    fetch: FetchFn<'_>,
    opts: &LoadOptions,
) -> (Vec<CssStyleSheet>, LoadStats) {
    let collected = collect_sheet_sources(dom, document_url);
    let mut loader = Loader {
        fetch,
        opts: *opts,
        cache: HashMap::new(),
        sheets_created: 0,
        stats: LoadStats {
            sources: collected.sources.len(),
            ..LoadStats::default()
        },
    };
    let mut sheets = Vec::new();
    for source in collected.sources {
        if loader.sheets_created >= loader.opts.max_sheets {
            loader.stats.skipped += 1;
            continue;
        }
        let css = match &source.location {
            None => source.css.clone(),
            Some(target) => match loader.fetch_text(target) {
                Some(text) => text,
                None => continue,
            },
        };
        if css.len() > loader.opts.max_sheet_bytes {
            loader.stats.skipped += 1;
            continue;
        }
        loader.sheets_created += 1;
        let base = source
            .location
            .clone()
            .unwrap_or_else(|| collected.base_url.clone());
        let mut stack = vec![base.clone()];
        let expanded = loader.expand_imports(parse_rules(&css), &base, 1, &mut stack);
        sheets.push(build_sheet(&source, expanded));
    }
    (sheets, loader.stats)
}

/// 按文档 base URL 抓取子资源样式表（scheme 策略 + MIME + 编码）。
///
/// - **scheme 策略**：`muskitty_network::url::is_fetchable_subresource`（http(s)
///   文档不得读 `file://`）；
/// - **http(s)**：非 2xx → 失败（CSSOM "fetch a CSS style sheet"）；Content-Type
///   存在且非 `text/css` → 失败；缺 Content-Type 按默认类型 `text/css` 处理
///   （HTML §4.6.8.23 L11657"默认类型"）；
/// - **file**：本地读，无 Content-Type → 默认类型 `text/css`；
/// - **data**：URL 自带的 media type 必须是 `text/css`；
/// - **编码**（D6 最小策略）：BOM 嗅探（UTF-8 / UTF-16LE / UTF-16BE），其余按
///   UTF-8 lossy（`@charset` / Content-Type charset 的非 UTF-8 解码未实现）。
pub struct DocumentFetcher {
    base: String,
}

impl DocumentFetcher {
    /// 以文档 base URL 构造（file 模式传 file URL，http 模式传最终响应 URL）。
    pub fn new(document_url: &str) -> Self {
        Self {
            base: document_url.to_string(),
        }
    }

    /// 抓取一个已解析的绝对 URL，返回样式表文本。
    pub fn fetch_text(&mut self, target: &str) -> Result<String, String> {
        if !url::is_fetchable_subresource(&self.base, target) {
            return Err(format!("blocked by subresource policy: {target}"));
        }
        match url::scheme(target).as_deref() {
            Some("data") => {
                let decoded = url::decode_data_url(target)
                    .ok_or_else(|| format!("invalid data URL: {target}"))?;
                if !is_css_mime(&decoded.media_type) {
                    return Err(format!("not text/css: {}", decoded.media_type));
                }
                Ok(decode_css_bytes(&decoded.bytes))
            }
            Some("file") => {
                let path = url::path_from_file_url(target)
                    .ok_or_else(|| format!("unmappable file URL: {target}"))?;
                let bytes = std::fs::read(&path).map_err(|e| format!("read {path}: {e}"))?;
                Ok(decode_css_bytes(&bytes))
            }
            Some("http") | Some("https") => {
                let resp = muskitty_network::fetch_blocking(target).map_err(|e| e.to_string())?;
                if !resp.is_success() {
                    return Err(format!("HTTP status {}", resp.status));
                }
                let ct = resp.header("content-type").unwrap_or("");
                if !ct.is_empty() && !is_css_mime(ct) {
                    return Err(format!("not text/css: {ct}"));
                }
                Ok(decode_css_bytes(resp.body_bytes()))
            }
            other => Err(format!("unsupported scheme: {other:?}")),
        }
    }
}

/// CSS 字节 → 文本（D6：BOM 嗅探 + UTF-8 为准）。
fn decode_css_bytes(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return decode_utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return decode_utf16(rest, false);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

/// UTF-16（按 BOM 指示的端序）→ 文本；截断的奇数字节丢弃。
fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if little_endian {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(html: &str) -> Rc<RefCell<Node>> {
        muskitty_html5_parser::parse(html)
    }

    /// 不抓任何外链（`Err`）的抓取器。
    fn no_fetch(_: &str) -> Result<String, String> {
        Err("no fetch in this test".to_string())
    }

    /// 固定映射的抓取器：URL → 文本；未命中 → `Err`。
    fn mapped<'a>(
        map: &'a HashMap<String, String>,
    ) -> impl FnMut(&str) -> Result<String, String> + 'a {
        move |url: &str| {
            map.get(url)
                .cloned()
                .ok_or_else(|| format!("no mapping for {url}"))
        }
    }

    // ---- 采集 ----

    #[test]
    fn collects_style_and_link_in_document_order() {
        let dom = parse(
            "<html><head><style>a{color:red}</style>\
             <link rel=\"stylesheet\" href=\"one.css\">\
             <style>b{color:blue}</style></head>\
             <body><link rel=stylesheet href=two.css></body></html>",
        );
        let collected = collect_sheet_sources(&dom, "https://e.com/page");
        let locs: Vec<Option<&str>> = collected
            .sources
            .iter()
            .map(|s| s.location.as_deref())
            .collect();
        assert_eq!(
            locs,
            vec![
                None,
                Some("https://e.com/one.css"),
                None,
                Some("https://e.com/two.css"),
            ],
            "style/link 交错必须保持文档序（body 内的 link 也是 body-ok，§4.6.8.20）"
        );
        assert_eq!(collected.sources[0].css, "a{color:red}");
        assert_eq!(collected.sources[2].css, "b{color:blue}");
    }

    #[test]
    fn ignores_style_text_inside_comments_and_scripts() {
        // 字符串扫描版（extract_inline_style）会命中这两处；DOM 路径不会。
        let dom = parse(
            "<html><head><!-- <style>i{color:red}</style> -->\
             <script>var s = \"<style>j{color:blue}</style>\";</script>\
             <style>k{color:green}</style></head><body></body></html>",
        );
        let collected = collect_sheet_sources(&dom, "https://e.com/");
        assert_eq!(collected.sources.len(), 1);
        assert_eq!(collected.sources[0].css, "k{color:green}");
    }

    #[test]
    fn link_attributes_follow_spec_semantics() {
        let html = "<html><head>\
            <link rel=\"next stylesheet\" href=\"a.css\" media=\"screen\" title=\"T\">\
            <link rel=\"alternate stylesheet\" href=\"b.css\">\
            <link rel=\"stylesheet\" href=\"c.css\" disabled>\
            <link rel=\"stylesheet\" href=\"d.css\" type=\"text/plain\">\
            <link rel=\"stylesheet\" href=\"e.css\" type=\"text/css; charset=utf-8\">\
            <link rel=\"stylesheet\" href=\"   \">\
            <link rel=\"stylesheet\">\
            <link rel=\"icon\" href=\"f.css\">\
            <link rel=\"STYLESHEET\" href=\"g.css\">\
            </head><body></body></html>";
        let dom = parse(html);
        let collected = collect_sheet_sources(&dom, "https://e.com/dir/");
        let got: Vec<(&str, &str, bool, bool)> = collected
            .sources
            .iter()
            .map(|s| {
                (
                    s.location.as_deref().unwrap_or(""),
                    s.media.as_str(),
                    s.alternate,
                    s.disabled,
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                ("https://e.com/dir/a.css", "screen", false, false),
                ("https://e.com/dir/b.css", "", true, false),
                ("https://e.com/dir/c.css", "", false, true),
                ("https://e.com/dir/e.css", "", false, false),
                ("https://e.com/dir/g.css", "", false, false),
            ],
            "type=text/plain、空白 href、缺 href、icon 全部忽略；rel 大小写不敏感"
        );
    }

    #[test]
    fn base_href_sets_resolution_base() {
        let dom = parse(
            "<html><head><base href=\"https://cdn.example/assets/\">\
             <link rel=stylesheet href=\"a.css\"></head><body></body></html>",
        );
        let collected = collect_sheet_sources(&dom, "https://e.com/page");
        assert_eq!(collected.base_url, "https://cdn.example/assets/");
        assert_eq!(
            collected.sources[0].location.as_deref(),
            Some("https://cdn.example/assets/a.css")
        );
    }

    #[test]
    fn base_href_relative_resolves_against_document() {
        let dom = parse(
            "<html><head><base href=\"/sub/\"><link rel=stylesheet href=\"a.css\"></head></html>",
        );
        let collected = collect_sheet_sources(&dom, "https://e.com/dir/page");
        assert_eq!(collected.base_url, "https://e.com/sub/");
        assert_eq!(
            collected.sources[0].location.as_deref(),
            Some("https://e.com/sub/a.css")
        );
    }

    // ---- 加载：内嵌 + 外链 + 统计 ----

    #[test]
    fn load_inline_sheets_only_needs_no_fetch() {
        let dom = parse("<html><head><style>a{color:red}</style></head></html>");
        let mut fetch = no_fetch;
        let (sheets, stats) =
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &LoadOptions::default());
        assert_eq!(sheets.len(), 1);
        assert!(sheets[0].location.is_none());
        assert_eq!(sheets[0].origin, Origin::Author);
        assert_eq!(stats.sources, 1);
        assert_eq!(stats.fetched, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn load_external_sheet_and_failed_link_is_non_fatal() {
        let mut map = HashMap::new();
        map.insert(
            "https://e.com/ok.css".to_string(),
            "a{color:red}".to_string(),
        );
        let dom = parse(
            "<html><head><link rel=stylesheet href=ok.css>\
             <link rel=stylesheet href=missing.css>\
             <style>b{color:blue}</style></head></html>",
        );
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        assert_eq!(sheets.len(), 2, "失败的表跳过，其余照常");
        assert_eq!(sheets[0].location.as_deref(), Some("https://e.com/ok.css"));
        assert!(sheets[1].location.is_none());
        assert_eq!(stats.fetched, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.cache_hits, 0);
    }

    #[test]
    fn duplicate_url_fetched_once_per_document() {
        let mut map = HashMap::new();
        map.insert(
            "https://e.com/a.css".to_string(),
            "a{color:red}".to_string(),
        );
        let dom = parse(
            "<html><head><link rel=stylesheet href=a.css>\
             <link rel=stylesheet href=a.css></head></html>",
        );
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        // 规范：两个 link 是两份独立样式表（06-4...md:627），但抓取只发一次。
        assert_eq!(sheets.len(), 2);
        assert_eq!(stats.fetched, 1);
        assert_eq!(stats.cache_hits, 1);
    }

    #[test]
    fn sheet_over_size_limit_is_skipped() {
        let mut map = HashMap::new();
        map.insert("https://e.com/big.css".to_string(), "x".repeat(100));
        let dom = parse("<html><head><link rel=stylesheet href=big.css></head></html>");
        let options = LoadOptions {
            max_sheet_bytes: 10,
            ..LoadOptions::default()
        };
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        assert!(sheets.is_empty());
        assert_eq!(stats.skipped, 1);
    }

    #[test]
    fn sheet_count_limit_skips_extra_sources() {
        let dom = parse(
            "<html><head><style>a{}</style><style>b{}</style><style>c{}</style></head></html>",
        );
        let options = LoadOptions {
            max_sheets: 2,
            ..LoadOptions::default()
        };
        let mut fetch = no_fetch;
        let (sheets, stats) = load_stylesheets(&dom, "https://e.com/", &mut fetch, &options);
        assert_eq!(sheets.len(), 2);
        assert_eq!(stats.skipped, 1);
    }

    #[test]
    fn media_and_title_attributes_land_on_the_sheet() {
        let dom = parse(
            "<html><head><link rel=stylesheet href=a.css media=\"screen and (min-width: 1px)\" title=\"Alt\"></head></html>",
        );
        let mut fetch = no_fetch;
        let (sheets, _) =
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &LoadOptions::default());
        // 抓取失败 → 无表；改用内嵌 <style media> 检查字段落位。
        assert!(sheets.is_empty());

        let dom = parse("<html><head><style media=\"print\">a{}</style></head></html>");
        let mut fetch = no_fetch;
        let (sheets, _) =
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &LoadOptions::default());
        assert_eq!(sheets.len(), 1);
        assert!(
            !sheets[0].media.is_empty(),
            "media 属性应解析为 component values"
        );
    }

    // ---- @import ----

    #[test]
    fn import_expands_in_place_with_media_wrapper() {
        let mut map = HashMap::new();
        map.insert(
            "https://e.com/root/a.css".to_string(),
            "a{color:red}".to_string(),
        );
        map.insert(
            "https://e.com/root/b.css".to_string(),
            "b{color:blue}".to_string(),
        );
        let dom = parse(
            "<html><head><style>@import url(\"a.css\");\
             @import \"b.css\" screen;\
             c{color:green}</style></head></html>",
        );
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/root/page", &mut fetch, &options)
        };
        assert_eq!(stats.imports, 2);
        let rules = &sheets[0].css_rules;
        assert_eq!(rules.len(), 3, "两条 import 就地展开 + 自身规则");
        assert!(matches!(&rules[0], CssRule::Style(_)));
        assert!(matches!(&rules[1], CssRule::Media(_)), "条件导入包 @media");
        assert!(matches!(&rules[2], CssRule::Style(_)));
    }

    #[test]
    fn import_resolves_against_importing_sheet_url() {
        let mut map = HashMap::new();
        map.insert(
            "https://cdn.example/css/outer.css".to_string(),
            "@import url(\"inner/inner.css\");\no{color:black}".to_string(),
        );
        map.insert(
            "https://cdn.example/css/inner/inner.css".to_string(),
            "i{color:white}".to_string(),
        );
        let dom = parse(
            "<html><head><link rel=stylesheet href=\"//cdn.example/css/outer.css\"></head></html>",
        );
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        assert_eq!(stats.imports, 1);
        assert_eq!(sheets.len(), 1);
        assert_eq!(sheets[0].css_rules.len(), 2);
        assert_eq!(
            stats.failed, 0,
            "嵌套 import 的基准是外层表 URL，不是文档 URL"
        );
    }

    #[test]
    fn import_cycle_terminates() {
        let mut map = HashMap::new();
        map.insert(
            "https://e.com/a.css".to_string(),
            "@import url(\"b.css\");\na{}".to_string(),
        );
        map.insert(
            "https://e.com/b.css".to_string(),
            "@import url(\"a.css\");\nb{}".to_string(),
        );
        let dom = parse("<html><head><link rel=stylesheet href=a.css></head></html>");
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        // a 展开出 b；b 的 import 指回 a → 栈内命中，跳过。
        assert_eq!(sheets.len(), 1);
        assert_eq!(stats.imports, 1);
        assert_eq!(stats.skipped, 1);
    }

    #[test]
    fn import_depth_limit_is_enforced() {
        let mut map = HashMap::new();
        for i in 0..5 {
            map.insert(
                format!("https://e.com/{i}.css"),
                format!("@import url(\"{}.css\");\nx{i}{{}}", i + 1),
            );
        }
        let dom = parse("<html><head><link rel=stylesheet href=0.css></head></html>");
        let options = LoadOptions {
            max_import_depth: 2,
            ..LoadOptions::default()
        };
        let (_, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        assert_eq!(stats.imports, 2, "深度 1、2 展开；深度 3 起跳过");
        assert!(stats.skipped >= 1);
    }

    #[test]
    fn import_after_other_rules_is_invalid() {
        let mut map = HashMap::new();
        map.insert("https://e.com/late.css".to_string(), "l{}".to_string());
        let dom = parse("<html><head><style>a{}@import url(\"late.css\");</style></head></html>");
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        assert_eq!(
            stats.imports, 0,
            "其他规则之后出现的 @import 无效（L115-118）"
        );
        assert_eq!(sheets[0].css_rules.len(), 1);
    }

    #[test]
    fn import_with_layer_or_supports_prefix_is_skipped_whole() {
        let mut map = HashMap::new();
        map.insert("https://e.com/x.css".to_string(), "x{}".to_string());
        let dom = parse(
            "<html><head><style>@import url(\"x.css\") layer(a);\
             @import url(\"x.css\") supports(display: flex);</style></head></html>",
        );
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        assert_eq!(stats.imports, 0);
        assert_eq!(stats.skipped, 2);
        assert!(sheets[0].css_rules.is_empty());
    }

    #[test]
    fn failed_import_is_skipped_and_siblings_survive() {
        let mut map = HashMap::new();
        map.insert("https://e.com/good.css".to_string(), "g{}".to_string());
        let dom = parse(
            "<html><head><style>@import url(\"missing.css\");\
             @import url(\"good.css\");</style></head></html>",
        );
        let options = LoadOptions::default();
        let (sheets, stats) = {
            let mut fetch = mapped(&map);
            load_stylesheets(&dom, "https://e.com/", &mut fetch, &options)
        };
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.imports, 1);
        assert_eq!(sheets[0].css_rules.len(), 1);
    }

    // ---- DocumentFetcher ----

    #[test]
    fn fetcher_reads_data_url_stylesheet() {
        let mut fetcher = DocumentFetcher::new("https://e.com/");
        let css = fetcher
            .fetch_text("data:text/css,p%7Bcolor%3Ared%7D")
            .expect("data url");
        assert_eq!(css, "p{color:red}");
        // 非 CSS media type 拒绝。
        assert!(fetcher.fetch_text("data:text/plain,hello").is_err());
    }

    #[test]
    fn fetcher_reads_file_url_and_blocks_cross_scheme() {
        let dir = std::env::temp_dir();
        let path = dir.join("muskitty_stylesheets_fetch_test.css");
        std::fs::write(&path, "body{margin:0}").unwrap();
        let file_url = url::file_url_from_path(&path.to_string_lossy()).expect("file url");

        let mut file_fetcher = DocumentFetcher::new("file:///D:/site/index.html");
        assert_eq!(
            file_fetcher.fetch_text(&file_url).expect("read css"),
            "body{margin:0}"
        );

        // http 文档不得读本地文件。
        let mut http_fetcher = DocumentFetcher::new("https://e.com/");
        let err = http_fetcher.fetch_text(&file_url).expect_err("must block");
        assert!(err.contains("subresource policy"), "got {err}");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fetcher_utf16_bom_decoding() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "a{color:red}".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_css_bytes(&bytes), "a{color:red}");
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(b"b{color:blue}");
        assert_eq!(decode_css_bytes(&bytes), "b{color:blue}");
    }

    #[test]
    fn css_style_sheet_crosses_threads() {
        // D2 前提：导航线程构造样式表后经 channel 回传。
        fn assert_send<T: Send>() {}
        assert_send::<CssStyleSheet>();
        assert_send::<LoadStats>();
    }
}
