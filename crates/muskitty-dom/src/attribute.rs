//! HTML/DOM 属性与命名空间类型。
//!
//! 参见 DOM Living Standard §6.7 (Attributes) 和
//! WHATWG HTML §13.2.6.5 (Adjust foreign attributes)。

/// 命名空间标识符。
///
/// HTML 文档中元素和属性可能属于 HTML、SVG 或 MathML 命名空间。
/// 参见 WHATWG HTML §13.2.6.1。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Namespace {
    /// `http://www.w3.org/1999/xhtml`
    Html,
    /// `http://www.w3.org/2000/svg`
    Svg,
    /// `http://www.w3.org/1998/Math/MathML`
    MathMl,
}

impl Namespace {
    /// 返回该命名空间的 URI 字符串。
    pub fn uri(&self) -> Option<&'static str> {
        match self {
            Namespace::Html => Some("http://www.w3.org/1999/xhtml"),
            Namespace::Svg => Some("http://www.w3.org/2000/svg"),
            Namespace::MathMl => Some("http://www.w3.org/1998/Math/MathML"),
        }
    }
}

/// 元素属性。
///
/// 对应 DOM `Attr` 接口的核心字段。详见 DOM Living Standard §6.7。
#[derive(Debug, Clone)]
pub struct Attribute {
    /// 命名空间前缀（如 `xml:lang` 的 `xml`）。
    pub prefix: Option<String>,
    /// 命名空间 URI。
    pub namespace_uri: Option<String>,
    /// 限定名前缀的本地部分（不含前缀）。
    pub local_name: String,
    /// 属性值。
    pub value: String,
}

impl Attribute {
    /// 创建一个无命名空间的 HTML 属性。
    pub fn new(local_name: &str, value: &str) -> Self {
        Self {
            prefix: None,
            namespace_uri: None,
            local_name: local_name.to_string(),
            value: value.to_string(),
        }
    }

    /// 创建带命名空间的属性。
    pub fn with_namespace(
        prefix: Option<String>,
        namespace_uri: Option<String>,
        local_name: String,
        value: String,
    ) -> Self {
        Self {
            prefix,
            namespace_uri,
            local_name,
            value,
        }
    }
}
