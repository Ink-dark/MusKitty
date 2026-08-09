//! 最小可跑 demo：用 ReqwestFetcher fetch 一个 URL，打印状态码与 body 前 200 字符。
//!
//! 用法：`cargo run --example fetch_demo -- <url>`
//! 不传 URL 时默认 fetch `https://example.com`。

use muskitty_network::{NetworkFetcher, ReqwestFetcher};

#[tokio::main]
async fn main() -> muskitty_network::NetworkResult<()> {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".to_string());

    let fetcher = ReqwestFetcher::new()?;
    let resp = fetcher.fetch(&url).await?;

    println!("URL:     {}", resp.url);
    println!("Status:  {}", resp.status);
    println!("Success: {}", resp.is_success());
    for (k, v) in &resp.headers {
        println!("Header:  {k}: {v}");
    }

    let text = resp.text();
    let preview = if text.chars().count() > 200 {
        let end = text
            .char_indices()
            .nth(200)
            .map(|(i, _)| i)
            .unwrap_or(text.len());
        format!("{}…", &text[..end])
    } else {
        text
    };
    println!("\n--- body (first 200 chars) ---\n{preview}");

    Ok(())
}
