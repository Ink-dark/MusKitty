//! URL Standard 支撑：相对引用解析、`file://` ↔ 路径转换、子资源 scheme 策略、
//! `data:` 解码。
//!
//! 实现委托给 [`url`](https://docs.rs/url)（WHATWG URL Standard 的参考实现，
//! 纯 Rust）。**本模块的 pub 签名只出现 `&str`/`String`/`Vec<u8>`，`url::Url`
//! 不外泄**——对齐 [外部依赖解耦 ADR](../../../../docs/decisions/2026-08-16-external-dependency-decoupling.md)。
//!
//! 用途（CS-1）：`<link rel=stylesheet href>` 与 `@import` 的相对解析、本地文件
//! 模式的 base URL、`file://` 与路径互转、子资源抓取策略。

use ::url::Url;

/// 用 `base`（文档或样式表自身的绝对 URL）解析 `reference`。
///
/// - `base` 必须是可解析的绝对 URL，否则 `None`（不做"无 scheme 补全"，
///   那是地址栏输入分类 `chrome::classify_url` 的职责）；
/// - `reference` 支持相对引用、绝对 URL、协议相对（`//host/p`）、`data:`；
/// - 返回 WHATWG serialization 的绝对 URL；解析失败返回 `None`。
///
/// 例：`resolve("https://e.com/a/b.html", "c/d.css")` →
/// `Some("https://e.com/a/c/d.css")`。
pub fn resolve(base: &str, reference: &str) -> Option<String> {
    let base = Url::parse(base).ok()?;
    base.join(reference).ok().map(|u| u.to_string())
}

/// 本地路径 → `file://` URL（相对路径先按当前工作目录绝对化）。
///
/// Windows 盘符、反斜杠、非 ASCII 与空格由 `url` crate 按 URL Standard 处理：
/// `D:\site\index.html` → `file:///D:/site/index.html`。
pub fn file_url_from_path(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(p)
    };
    Url::from_file_path(&abs).ok().map(|u| u.to_string())
}

/// `file://` URL → 本地路径（非 `file:` scheme 或宿主不可映射时 `None`）。
pub fn path_from_file_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    if parsed.scheme() != "file" {
        return None;
    }
    Some(parsed.to_file_path().ok()?.to_string_lossy().into_owned())
}

/// 子资源抓取策略：`base`（文档 URL）指向 `target` 的样式表请求是否允许。
///
/// 这是本实现的安全边界（HTML/Security），不是规范算法：
/// - `data:`：任何 base 都允许（无网络、无文件访问）；
/// - `http(s)://`：base 为 http/https/file 时允许；
/// - `file:`：**仅** base 也是 file 时允许（http(s) 页面不得读本地文件，
///   与浏览器的 file 访问限制一致）；
/// - 其余 scheme（`about:`/`javascript:`/`blob:`/…）一律拒绝。
///
/// `target` 必须是已解析的绝对 URL（本函数不解析相对引用）。
pub fn is_fetchable_subresource(base: &str, target: &str) -> bool {
    let Ok(base_url) = Url::parse(base) else {
        return false;
    };
    let Ok(target_url) = Url::parse(target) else {
        return false;
    };
    match target_url.scheme() {
        "data" => true,
        "http" | "https" => matches!(base_url.scheme(), "http" | "https" | "file"),
        "file" => base_url.scheme() == "file",
        _ => false,
    }
}

/// 解码后的 `data:` URL。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataUrl {
    /// 媒体类型（`data:` 后、`,` 前的第一段；缺省时按规范取
    /// `text/plain;charset=US-ASCII`）。
    pub media_type: String,
    /// 解码后的字节。
    pub bytes: Vec<u8>,
}

/// `data:` URL 解码（URL Standard §data URL processor 的最小实现）。
///
/// - 只接受 `data:` scheme，其余返回 `None`；
/// - 处理 `[<mediatype>][;base64],<data>`；
/// - 字节序列先按规范**去掉全部 ASCII 空白**再解码；
/// - base64 走 forgiving 解码（标准字母表 + 可选填充 + 忽略空白），非法字符 → `None`；
/// - 非 base64 走百分号解码（`%XX`；非法序列按 URL Standard 保留原字符）。
pub fn decode_data_url(url: &str) -> Option<DataUrl> {
    let rest = url.strip_prefix("data:")?;
    // 规范：处理前去掉 input 中全部 ASCII 空白。
    let cleaned: String = rest.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let comma = cleaned.find(',')?;
    let meta = &cleaned[..comma];
    let body = &cleaned[comma + 1..];

    let (media_type, is_base64) = match meta.strip_suffix(";base64") {
        Some(m) => (m, true),
        None => (meta, false),
    };
    let media_type = if media_type.is_empty() {
        "text/plain;charset=US-ASCII".to_string()
    } else {
        media_type.to_string()
    };

    let bytes = if is_base64 {
        forgiving_base64_decode(body)?
    } else {
        percent_decode(body)
    };
    Some(DataUrl { media_type, bytes })
}

/// 百分号解码（URL Standard percent-decode）：`%XX` → 字节，其余字符按 UTF-8 原样入队；
/// `%` 后不是两位十六进制时保留字面 `%`。
fn percent_decode(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// forgiving-base64 解码（Infra Standard）：忽略 ASCII 空白与 `=` 填充，
/// 长度模 4 余 1、或出现字母表外字符 → `None`。
fn forgiving_base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for b in s.bytes() {
        if b.is_ascii_whitespace() || b == b'=' {
            continue;
        }
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    // 余下 bits 只能是 0（规范：多余位必须为 0，且余数不能是 1 个字符）。
    if bits >= 6 || (acc & ((1 << bits) - 1)) != 0 {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_paths() {
        let base = "https://example.com/a/b.html";
        assert_eq!(
            resolve(base, "c.css").as_deref(),
            Some("https://example.com/a/c.css")
        );
        assert_eq!(
            resolve(base, "./c.css").as_deref(),
            Some("https://example.com/a/c.css")
        );
        assert_eq!(
            resolve(base, "../d.css").as_deref(),
            Some("https://example.com/d.css")
        );
        assert_eq!(
            resolve(base, "../../e.css").as_deref(),
            Some("https://example.com/e.css")
        );
        assert_eq!(
            resolve(base, "/f.css").as_deref(),
            Some("https://example.com/f.css")
        );
        assert_eq!(
            resolve(base, "sub/dir/g.css").as_deref(),
            Some("https://example.com/a/sub/dir/g.css")
        );
    }

    #[test]
    fn resolve_absolute_protocol_relative_query_fragment() {
        let base = "https://example.com/a/b.html";
        assert_eq!(
            resolve(base, "https://cdn.example/h.css").as_deref(),
            Some("https://cdn.example/h.css")
        );
        assert_eq!(
            resolve(base, "//other.example/i.css").as_deref(),
            Some("https://other.example/i.css")
        );
        assert_eq!(
            resolve(base, "?v=2").as_deref(),
            Some("https://example.com/a/b.html?v=2")
        );
        assert_eq!(
            resolve(base, "#frag").as_deref(),
            Some("https://example.com/a/b.html#frag")
        );
        // 空引用 = base 去掉 fragment（URL Standard join 语义）。
        assert_eq!(
            resolve("https://example.com/a/b.html?q=1#f", "").as_deref(),
            Some("https://example.com/a/b.html?q=1")
        );
        // 相对引用替换 base 的 query/fragment。
        assert_eq!(
            resolve("https://example.com/a/b.html?q=1#f", "c.css").as_deref(),
            Some("https://example.com/a/c.css")
        );
    }

    #[test]
    fn resolve_percent_and_non_ascii() {
        assert_eq!(
            resolve("https://example.com/a/b.html", "st%20yle.css").as_deref(),
            Some("https://example.com/a/st%20yle.css")
        );
        assert_eq!(
            resolve("https://example.com/a/b.html", "样式.css").as_deref(),
            Some("https://example.com/a/%E6%A0%B7%E5%BC%8F.css")
        );
        // 前后空白按 URL Standard 预处理剥离。
        assert_eq!(
            resolve("https://example.com/a/b.html", "  c.css  ").as_deref(),
            Some("https://example.com/a/c.css")
        );
    }

    #[test]
    fn resolve_rejects_bad_inputs() {
        assert_eq!(resolve("not a url", "a.css"), None);
        assert_eq!(resolve("/a/b.html", "a.css"), None);
        assert_eq!(resolve("https://example.com/", "http://"), None);
    }

    #[test]
    fn resolve_file_base() {
        assert_eq!(
            resolve("file:///D:/site/index.html", "css/a.css").as_deref(),
            Some("file:///D:/site/css/a.css")
        );
        assert_eq!(
            resolve("file:///D:/site/index.html", "../shared/b.css").as_deref(),
            Some("file:///D:/shared/b.css")
        );
    }

    #[cfg(windows)]
    #[test]
    fn file_url_roundtrip_windows() {
        assert_eq!(
            file_url_from_path(r"D:\site\index.html").as_deref(),
            Some("file:///D:/site/index.html")
        );
        assert_eq!(
            path_from_file_url("file:///D:/site/index.html").as_deref(),
            Some(r"D:\site\index.html")
        );
        // 空格与非 ASCII 同样往返。
        assert_eq!(
            path_from_file_url(&file_url_from_path(r"D:\my site\样式.css").unwrap()).as_deref(),
            Some(r"D:\my site\样式.css")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn file_url_roundtrip_unix() {
        assert_eq!(
            file_url_from_path("/tmp/site/index.html").as_deref(),
            Some("file:///tmp/site/index.html")
        );
        assert_eq!(
            path_from_file_url("file:///tmp/site/index.html").as_deref(),
            Some("/tmp/site/index.html")
        );
    }

    #[test]
    fn path_from_file_url_rejects_non_file() {
        assert_eq!(path_from_file_url("https://example.com/x.css"), None);
        assert_eq!(path_from_file_url("data:text/css,a{}"), None);
    }

    #[test]
    fn subresource_policy() {
        let https = "https://example.com/page";
        let file = "file:///D:/site/index.html";
        assert!(is_fetchable_subresource(https, "https://cdn.example/a.css"));
        assert!(is_fetchable_subresource(https, "http://localhost:1/a.css"));
        assert!(is_fetchable_subresource(https, "data:text/css,a{}"));
        // http(s) 页面不得读本地文件。
        assert!(!is_fetchable_subresource(https, "file:///D:/secret.css"));
        // file 页面可以读本地文件，也可以引用远端样式（浏览器同）。
        assert!(is_fetchable_subresource(file, "file:///D:/site/a.css"));
        assert!(is_fetchable_subresource(file, "https://cdn.example/a.css"));
        assert!(is_fetchable_subresource(file, "data:text/css,a{}"));
        // 其余 scheme 一律拒。
        assert!(!is_fetchable_subresource(https, "about:blank"));
        assert!(!is_fetchable_subresource(https, "javascript:alert(1)"));
        assert!(!is_fetchable_subresource(
            https,
            "blob:https://example.com/u"
        ));
        assert!(!is_fetchable_subresource(https, "not a url"));
    }

    #[test]
    fn data_url_percent_decoding() {
        let d = decode_data_url("data:text/css,p%7Bcolor%3Ared%7D").expect("decode");
        assert_eq!(d.media_type, "text/css");
        assert_eq!(d.bytes, b"p{color:red}");
    }

    #[test]
    fn data_url_base64_decoding() {
        let d = decode_data_url("data:text/css;base64,aGVsbG8=").expect("decode");
        assert_eq!(d.media_type, "text/css");
        assert_eq!(d.bytes, b"hello");
        // 空白（含换行）按规范剥离。
        let d = decode_data_url("data:text/css;base64,aGVs\nbG8=").expect("decode");
        assert_eq!(d.bytes, b"hello");
        // 无填充也接受。
        let d = decode_data_url("data:text/css;base64,aGVsbG8").expect("decode");
        assert_eq!(d.bytes, b"hello");
    }

    #[test]
    fn data_url_default_media_type_and_errors() {
        let d = decode_data_url("data:,hi").expect("decode");
        assert_eq!(d.media_type, "text/plain;charset=US-ASCII");
        assert_eq!(d.bytes, b"hi");
        // 非 data: / 缺逗号 / 非法 base64 字符。
        assert!(decode_data_url("https://example.com/x").is_none());
        assert!(decode_data_url("data:text/css").is_none());
        assert!(decode_data_url("data:text/css;base64,!!!!").is_none());
        // 百分号解码保留非法序列。
        let d = decode_data_url("data:text/css,a%zzb").expect("decode");
        assert_eq!(d.bytes, b"a%zzb");
    }
}
