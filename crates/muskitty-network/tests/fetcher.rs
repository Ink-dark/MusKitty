//! 端到端集成测试：用 wiremock 起离线 mock server，验证 ReqwestFetcher + NetworkFetcher trait。

use muskitty_network::{NetworkFetcher, NetworkResponse, ReqwestFetcher};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn spawn_ok_server(body: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain")
                .set_body_string(body),
        )
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn fetch_success_returns_status_body_headers() {
    let server = spawn_ok_server("hello muskitty").await;
    let fetcher = ReqwestFetcher::new().expect("client build");
    let url = server.uri() + "/";
    let resp: NetworkResponse = fetcher.fetch(&url).await.expect("fetch ok");

    assert_eq!(resp.status, 200);
    assert!(resp.is_success());
    assert_eq!(resp.text(), "hello muskitty");
    assert_eq!(resp.body_bytes(), b"hello muskitty");
    assert_eq!(
        resp.header("content-type").unwrap(),
        "text/plain",
        "header lookup should be case-insensitive"
    );
    assert_eq!(resp.url, url, "final url should match request url");
}

#[tokio::test]
async fn fetch_404_does_not_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&server)
        .await;

    let fetcher = ReqwestFetcher::new().expect("client build");
    let resp = fetcher
        .fetch(&(server.uri() + "/missing"))
        .await
        .expect("fetch ok");

    assert_eq!(resp.status, 404);
    assert!(!resp.is_success());
    assert_eq!(resp.text(), "not found");
}

#[tokio::test]
async fn fetch_500_does_not_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/boom"))
        .respond_with(ResponseTemplate::new(500).set_body_string("server error"))
        .mount(&server)
        .await;

    let fetcher = ReqwestFetcher::new().expect("client build");
    let resp = fetcher
        .fetch(&(server.uri() + "/boom"))
        .await
        .expect("fetch ok");

    assert_eq!(resp.status, 500);
    assert!(!resp.is_success());
}

#[tokio::test]
async fn fetch_binary_body_preserved() {
    let server = MockServer::start().await;
    let body = vec![0u8, 1, 2, 255, 128, 64];
    Mock::given(method("GET"))
        .and(path("/bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body.clone()))
        .mount(&server)
        .await;

    let fetcher = ReqwestFetcher::new().expect("client build");
    let resp = fetcher
        .fetch(&(server.uri() + "/bin"))
        .await
        .expect("fetch ok");

    assert_eq!(resp.status, 200);
    assert_eq!(resp.body_bytes(), body.as_slice());
}

#[tokio::test]
async fn fetch_trait_object_via_generic_works() {
    // 验证上层可通过泛型约束 F: NetworkFetcher 使用，不绑定具体类型
    async fn do_fetch<F: NetworkFetcher>(
        fetcher: &F,
        url: &str,
    ) -> muskitty_network::NetworkResult<NetworkResponse> {
        fetcher.fetch(url).await
    }

    let server = spawn_ok_server("via trait").await;
    let fetcher = ReqwestFetcher::new().expect("client build");
    let resp = do_fetch(&fetcher, &(server.uri() + "/"))
        .await
        .expect("fetch ok");
    assert_eq!(resp.status, 200);
    assert_eq!(resp.text(), "via trait");
}

#[tokio::test]
async fn fetch_connection_refused_returns_error() {
    // 用一个几乎肯定没有监听的端口触发连接错误
    let fetcher = ReqwestFetcher::new().expect("client build");
    let result = fetcher.fetch("http://127.0.0.1:1/").await;
    assert!(result.is_err(), "connection refused should be NetworkError");
    let err = result.unwrap_err();
    assert!(
        matches!(err, muskitty_network::NetworkError::Http(_)),
        "expected Http error variant, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_body_over_limit_errors_with_body_too_large() {
    // F-14（审计 S-6）：响应体超过上限 → NetworkError::BodyTooLarge，
    // 不再无上限缓冲（敌意服务器可借 chunked 流 OOM abort 进程）。
    let server = spawn_ok_server("0123456789abcdef0123456789abcdef").await; // 32 字节
    let fetcher = ReqwestFetcher::with_max_body_bytes(16).expect("client build");
    let result = fetcher.fetch(&(server.uri() + "/")).await;
    let err = result.expect_err("over-limit body must error");
    assert!(
        matches!(err, muskitty_network::NetworkError::BodyTooLarge { .. }),
        "expected BodyTooLarge, got {err:?}"
    );
}

#[tokio::test]
async fn fetch_body_at_limit_succeeds() {
    // 恰好等于上限的响应体应完整送达（上限是 ≤ 语义）。
    let server = spawn_ok_server("0123456789abcdef").await; // 16 字节
    let fetcher = ReqwestFetcher::with_max_body_bytes(16).expect("client build");
    let resp = fetcher
        .fetch(&(server.uri() + "/"))
        .await
        .expect("at-limit body must succeed");
    assert_eq!(resp.body_bytes().len(), 16);
}

#[tokio::test]
async fn fetcher_is_clone_and_reuses() {
    let server = spawn_ok_server("clone").await;
    let fetcher = ReqwestFetcher::new().expect("client build");
    let fetcher2 = fetcher.clone();

    let r1 = fetcher
        .fetch(&(server.uri() + "/"))
        .await
        .expect("fetch1 ok");
    let r2 = fetcher2
        .fetch(&(server.uri() + "/"))
        .await
        .expect("fetch2 ok");
    assert_eq!(r1.text(), "clone");
    assert_eq!(r2.text(), "clone");
}
