use grpcproxy::pb::{
    echo_service_client::EchoServiceClient, math_service_client::MathServiceClient, AddRequest,
    EchoRequest, FibRequest,
};
use std::net::SocketAddr;
use tokio::sync::oneshot;
use tokio_stream::StreamExt;

struct TestEnv {
    echo: EchoServiceClient<tonic::transport::Channel>,
    math: MathServiceClient<tonic::transport::Channel>,
    _backend_shutdown: oneshot::Sender<()>,
    _proxy_shutdown: oneshot::Sender<()>,
}

async fn setup() -> TestEnv {
    let backend_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (backend_addr, backend_shutdown) = grpcproxy::backend::start_backend(backend_addr)
        .await
        .expect("start backend");

    let proxy_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (proxy_addr, proxy_shutdown) =
        grpcproxy::proxy::start_proxy(proxy_addr, &backend_addr.to_string())
            .await
            .expect("start proxy");

    // Give servers a moment to start accepting connections
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let url = format!("http://{}", proxy_addr);
    let echo = EchoServiceClient::connect(url.clone())
        .await
        .expect("connect echo");
    let math = MathServiceClient::connect(url)
        .await
        .expect("connect math");

    TestEnv {
        echo,
        math,
        _backend_shutdown: backend_shutdown,
        _proxy_shutdown: proxy_shutdown,
    }
}

struct PassthroughEnv {
    echo: EchoServiceClient<tonic::transport::Channel>,
    _backend_shutdown: oneshot::Sender<()>,
    _proxy_shutdown: oneshot::Sender<()>,
}

/// Setup with a passthrough proxy (no EchoService registered — everything forwarded).
async fn setup_passthrough() -> PassthroughEnv {
    let backend_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (backend_addr, backend_shutdown) = grpcproxy::backend::start_backend(backend_addr)
        .await
        .expect("start backend");

    let proxy_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (proxy_addr, proxy_shutdown) =
        grpcproxy::proxy::start_passthrough_proxy(proxy_addr, &backend_addr.to_string())
            .await
            .expect("start passthrough proxy");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let url = format!("http://{}", proxy_addr);
    let echo = EchoServiceClient::connect(url)
        .await
        .expect("connect echo");

    PassthroughEnv {
        echo,
        _backend_shutdown: backend_shutdown,
        _proxy_shutdown: proxy_shutdown,
    }
}

// --- Intercepted: EchoService (handled by proxy, uppercased, source="proxy") ---

#[tokio::test]
async fn test_unary_echo_intercepted() {
    let mut env = setup().await;
    let resp = env
        .echo
        .unary_echo(EchoRequest {
            message: "hello".into(),
        })
        .await
        .expect("unary echo")
        .into_inner();
    assert_eq!(resp.source, "proxy");
    assert_eq!(resp.message, "HELLO");
}

#[tokio::test]
async fn test_server_stream_echo_intercepted() {
    let mut env = setup().await;
    let mut stream = env
        .echo
        .server_stream_echo(EchoRequest {
            message: "hello".into(),
        })
        .await
        .expect("server stream echo")
        .into_inner();

    let mut responses = Vec::new();
    while let Some(resp) = stream.next().await {
        responses.push(resp.expect("recv"));
    }

    assert_eq!(responses.len(), 3);
    for (i, resp) in responses.iter().enumerate() {
        assert_eq!(resp.source, "proxy");
        assert_eq!(resp.message, format!("[PROXY] HELLO #{}", i + 1));
    }
}

#[tokio::test]
async fn test_client_stream_echo_intercepted() {
    let mut env = setup().await;
    let reqs = tokio_stream::iter(
        ["hello", "world"]
            .iter()
            .map(|m| EchoRequest { message: m.to_string() }),
    );
    let resp = env
        .echo
        .client_stream_echo(reqs)
        .await
        .expect("client stream echo")
        .into_inner();
    assert_eq!(resp.source, "proxy");
    assert_eq!(resp.message, "HELLO WORLD");
}

#[tokio::test]
async fn test_bidi_stream_echo_intercepted() {
    let mut env = setup().await;
    let messages: Vec<&str> = vec!["hello", "world", "foo"];
    let reqs = tokio_stream::iter(
        messages
            .iter()
            .map(|m| EchoRequest { message: m.to_string() })
            .collect::<Vec<_>>(),
    );
    let mut stream = env
        .echo
        .bidi_stream_echo(reqs)
        .await
        .expect("bidi stream echo")
        .into_inner();

    let mut responses = Vec::new();
    while let Some(resp) = stream.next().await {
        responses.push(resp.expect("recv"));
    }

    assert_eq!(responses.len(), messages.len());
    for (resp, msg) in responses.iter().zip(messages.iter()) {
        assert_eq!(resp.source, "proxy");
        assert_eq!(resp.message, msg.to_uppercase());
    }
}

// --- Forwarded: MathService (transparently proxied to backend, source="backend") ---

#[tokio::test]
async fn test_add_forwarded() {
    let mut env = setup().await;
    let resp = env
        .math
        .add(AddRequest { a: 2, b: 3 })
        .await
        .expect("add")
        .into_inner();
    assert_eq!(resp.source, "backend");
    assert_eq!(resp.result, 5);
}

#[tokio::test]
async fn test_fibonacci_forwarded() {
    let mut env = setup().await;
    let mut stream = env
        .math
        .fibonacci(FibRequest { count: 7 })
        .await
        .expect("fibonacci")
        .into_inner();

    let want: Vec<i64> = vec![0, 1, 1, 2, 3, 5, 8];
    let mut got = Vec::new();
    while let Some(resp) = stream.next().await {
        let resp = resp.expect("recv");
        assert_eq!(resp.source, "backend");
        got.push(resp.value);
    }
    assert_eq!(got, want);
}

// --- Transparent proxy: all 4 streaming types forwarded (source="backend") ---
// These tests verify that the transparent proxy correctly handles all RPC types,
// not just unary and server-streaming.

#[tokio::test]
async fn test_unary_echo_forwarded() {
    let mut env = setup_passthrough().await;
    let resp = env
        .echo
        .unary_echo(EchoRequest {
            message: "hello".into(),
        })
        .await
        .expect("unary echo forwarded")
        .into_inner();
    assert_eq!(resp.source, "backend");
    assert_eq!(resp.message, "hello");
}

#[tokio::test]
async fn test_server_stream_echo_forwarded() {
    let mut env = setup_passthrough().await;
    let mut stream = env
        .echo
        .server_stream_echo(EchoRequest {
            message: "hello".into(),
        })
        .await
        .expect("server stream echo forwarded")
        .into_inner();

    let mut responses = Vec::new();
    while let Some(resp) = stream.next().await {
        responses.push(resp.expect("recv"));
    }

    assert_eq!(responses.len(), 3);
    for (i, resp) in responses.iter().enumerate() {
        assert_eq!(resp.source, "backend");
        assert_eq!(resp.message, format!("hello #{}", i + 1));
    }
}

#[tokio::test]
async fn test_client_stream_echo_forwarded() {
    let mut env = setup_passthrough().await;
    let reqs = tokio_stream::iter(
        ["hello", "world"]
            .iter()
            .map(|m| EchoRequest { message: m.to_string() })
            .collect::<Vec<_>>(),
    );
    let resp = env
        .echo
        .client_stream_echo(reqs)
        .await
        .expect("client stream echo forwarded")
        .into_inner();
    assert_eq!(resp.source, "backend");
    assert_eq!(resp.message, "hello world");
}

#[tokio::test]
async fn test_bidi_stream_echo_forwarded() {
    let mut env = setup_passthrough().await;
    let messages: Vec<&str> = vec!["hello", "world", "foo"];
    let reqs = tokio_stream::iter(
        messages
            .iter()
            .map(|m| EchoRequest { message: m.to_string() })
            .collect::<Vec<_>>(),
    );
    let mut stream = env
        .echo
        .bidi_stream_echo(reqs)
        .await
        .expect("bidi stream echo forwarded")
        .into_inner();

    let mut responses = Vec::new();
    while let Some(resp) = stream.next().await {
        responses.push(resp.expect("recv"));
    }

    assert_eq!(responses.len(), messages.len());
    for (resp, msg) in responses.iter().zip(messages.iter()) {
        assert_eq!(resp.source, "backend");
        assert_eq!(resp.message, *msg);
    }
}

// --- Backend down: intercepted calls still work, forwarded calls fail ---

struct BackendDownEnv {
    echo: EchoServiceClient<tonic::transport::Channel>,
    math: MathServiceClient<tonic::transport::Channel>,
    _proxy_shutdown: oneshot::Sender<()>,
}

async fn setup_backend_down() -> BackendDownEnv {
    // Point the proxy at a port where nothing is listening.
    let proxy_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (proxy_addr, proxy_shutdown) =
        grpcproxy::proxy::start_proxy(proxy_addr, "127.0.0.1:1")
            .await
            .expect("start proxy");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let url = format!("http://{}", proxy_addr);
    let echo = EchoServiceClient::connect(url.clone())
        .await
        .expect("connect echo");
    let math = MathServiceClient::connect(url)
        .await
        .expect("connect math");

    BackendDownEnv {
        echo,
        math,
        _proxy_shutdown: proxy_shutdown,
    }
}

#[tokio::test]
async fn test_unary_echo_backend_down() {
    let mut env = setup_backend_down().await;
    // Intercepted call should still work
    let resp = env
        .echo
        .unary_echo(EchoRequest {
            message: "hello".into(),
        })
        .await
        .expect("intercepted call should succeed")
        .into_inner();
    assert_eq!(resp.source, "proxy");
    assert_eq!(resp.message, "HELLO");
}

#[tokio::test]
async fn test_add_backend_down() {
    let mut env = setup_backend_down().await;
    // Forwarded call should fail
    let result = env.math.add(AddRequest { a: 2, b: 3 }).await;
    assert!(result.is_err(), "Add should fail when backend is down");
}

#[tokio::test]
async fn test_fibonacci_backend_down() {
    let mut env = setup_backend_down().await;
    // Forwarded streaming call should fail at call or first recv
    let result = env.math.fibonacci(FibRequest { count: 7 }).await;
    match result {
        Err(_) => {} // error at call time
        Ok(resp) => {
            let mut stream = resp.into_inner();
            let recv = stream.next().await;
            match recv {
                None => panic!("expected error, got empty stream"),
                Some(Ok(_)) => panic!("expected error, got successful response"),
                Some(Err(_)) => {} // error at recv time
            }
        }
    }
}
