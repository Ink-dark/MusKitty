//! 网络层错误类型。

use std::fmt;

/// 网络层错误。
#[derive(Debug)]
pub enum NetworkError {
    /// reqwest 底层错误（DNS / 连接 / TLS / 协议 / 超时等）。
    Http(reqwest::Error),
    /// URL 解析或格式错误。
    InvalidUrl(String),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::Http(e) => write!(f, "HTTP error: {e}"),
            NetworkError::InvalidUrl(u) => write!(f, "invalid URL: {u}"),
        }
    }
}

impl std::error::Error for NetworkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NetworkError::Http(e) => Some(e),
            NetworkError::InvalidUrl(_) => None,
        }
    }
}

impl From<reqwest::Error> for NetworkError {
    fn from(e: reqwest::Error) -> Self {
        NetworkError::Http(e)
    }
}

/// 网络层 Result 别名。
pub type NetworkResult<T> = Result<T, NetworkError>;
