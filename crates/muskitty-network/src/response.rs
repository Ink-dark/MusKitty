//! HTTP 响应封装。

/// HTTP 响应。持有 body 字节，提供同步访问。
#[derive(Debug, Clone)]
pub struct NetworkResponse {
    /// HTTP 状态码（如 200 / 404 / 500）。
    pub status: u16,
    /// 响应头列表 `(name, value)`，保留插入顺序，同名头可重复。
    pub headers: Vec<(String, String)>,
    /// 最终 URL（重定向后的最终地址）。
    pub url: String,
    /// 响应体原始字节。
    body_bytes: Vec<u8>,
}

impl NetworkResponse {
    #[allow(dead_code)]
    pub(crate) fn new(
        status: u16,
        headers: Vec<(String, String)>,
        url: String,
        body_bytes: Vec<u8>,
    ) -> Self {
        Self {
            status,
            headers,
            url,
            body_bytes,
        }
    }

    /// 响应体原始字节。
    pub fn body_bytes(&self) -> &[u8] {
        &self.body_bytes
    }

    /// 响应体文本（UTF-8 lossy 解码，不检测 charset）。
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body_bytes).into_owned()
    }

    /// 按名称查找首个响应头（大小写不敏感）。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// 状态码是否为 2xx 成功。
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}
