use std::convert::Infallible;
use std::net::SocketAddr;
use std::pin::Pin;

use http_body_util::BodyExt;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::sync::oneshot;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tower::Service;

use crate::pb::echo_service_server::{EchoService, EchoServiceServer};
use crate::pb::{EchoRequest, EchoResponse};
use crate::transparent_proxy::TransparentProxy;

pub struct InterceptedEchoService;

#[tonic::async_trait]
impl EchoService for InterceptedEchoService {
    async fn unary_echo(
        &self,
        req: Request<EchoRequest>,
    ) -> Result<Response<EchoResponse>, Status> {
        let msg = req.into_inner().message;
        Ok(Response::new(EchoResponse {
            message: msg.to_uppercase(),
            source: "proxy".into(),
        }))
    }

    type ServerStreamEchoStream =
        Pin<Box<dyn Stream<Item = Result<EchoResponse, Status>> + Send>>;

    async fn server_stream_echo(
        &self,
        req: Request<EchoRequest>,
    ) -> Result<Response<Self::ServerStreamEchoStream>, Status> {
        let upper = req.into_inner().message.to_uppercase();
        let stream = async_stream::stream! {
            for i in 0..3 {
                yield Ok(EchoResponse {
                    message: format!("[PROXY] {} #{}", upper, i + 1),
                    source: "proxy".into(),
                });
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn client_stream_echo(
        &self,
        req: Request<tonic::Streaming<EchoRequest>>,
    ) -> Result<Response<EchoResponse>, Status> {
        let mut stream = req.into_inner();
        let mut messages = Vec::new();
        while let Some(r) = stream.message().await? {
            messages.push(r.message);
        }
        Ok(Response::new(EchoResponse {
            message: messages.join(" ").to_uppercase(),
            source: "proxy".into(),
        }))
    }

    type BidiStreamEchoStream =
        Pin<Box<dyn Stream<Item = Result<EchoResponse, Status>> + Send>>;

    async fn bidi_stream_echo(
        &self,
        req: Request<tonic::Streaming<EchoRequest>>,
    ) -> Result<Response<Self::BidiStreamEchoStream>, Status> {
        let mut stream = req.into_inner();
        let output = async_stream::stream! {
            while let Some(r) = stream.message().await? {
                yield Ok(EchoResponse {
                    message: r.message.to_uppercase(),
                    source: "proxy".into(),
                });
            }
        };
        Ok(Response::new(Box::pin(output)))
    }
}

type BoxBody = http_body_util::combinators::UnsyncBoxBody<bytes::Bytes, Status>;

fn error_to_status(e: Box<dyn std::error::Error + Send + Sync>) -> Status {
    // tonic transport errors (connection refused, etc.) should be UNAVAILABLE,
    // matching Go's grpc-proxy behavior.
    if e.downcast_ref::<tonic::transport::Error>().is_some() {
        return Status::unavailable(e.to_string());
    }
    Status::internal(e.to_string())
}

fn status_to_response(status: Status) -> http::Response<BoxBody> {
    let (parts, body) = status.into_http().into_parts();
    let body = body
        .map_err(|e| Status::internal(e.to_string()))
        .boxed_unsync();
    http::Response::from_parts(parts, body)
}

pub async fn start_proxy(
    listen_addr: SocketAddr,
    backend_addr: &str,
) -> Result<(SocketAddr, oneshot::Sender<()>), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    let local_addr = listener.local_addr()?;

    let echo_svc = EchoServiceServer::new(InterceptedEchoService);
    let transparent = TransparentProxy::new_lazy(backend_addr);

    let (tx, mut rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (stream, _) = match result {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let echo = echo_svc.clone();
                    let proxy = transparent.clone();

                    tokio::spawn(serve_connection(stream, echo, proxy));
                }
                _ = &mut rx => break,
            }
        }
    });

    Ok((local_addr, tx))
}

/// Starts a passthrough proxy that forwards ALL services transparently (no interception).
/// Used for testing that the transparent proxy handles all streaming types.
pub async fn start_passthrough_proxy(
    listen_addr: SocketAddr,
    backend_addr: &str,
) -> Result<(SocketAddr, oneshot::Sender<()>), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    let local_addr = listener.local_addr()?;

    let transparent = TransparentProxy::new_lazy(backend_addr);

    let (tx, mut rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    let (stream, _) = match result {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                    let proxy = transparent.clone();
                    tokio::spawn(serve_passthrough_connection(stream, proxy));
                }
                _ = &mut rx => break,
            }
        }
    });

    Ok((local_addr, tx))
}

async fn serve_connection(
    stream: tokio::net::TcpStream,
    echo: EchoServiceServer<InterceptedEchoService>,
    proxy: TransparentProxy,
) {
    let svc = hyper::service::service_fn(
        move |req: http::Request<hyper::body::Incoming>| {
            let mut echo = echo.clone();
            let mut proxy = proxy.clone();
            async move {
                // Convert Incoming body to tonic's BoxBody
                let (parts, body) = req.into_parts();
                let body = body
                    .map_err(|e| Status::internal(e.to_string()))
                    .boxed_unsync();
                let req = http::Request::from_parts(parts, body);

                let path = req.uri().path().to_string();
                let resp = if path.starts_with("/grpcproxy.EchoService/") {
                    // Infallible error, so unwrap is safe
                    echo.call(req).await.unwrap_or_else(|e| match e {})
                } else {
                    match proxy.call(req).await {
                        Ok(resp) => resp,
                        Err(e) => status_to_response(error_to_status(e)),
                    }
                };

                Ok::<_, Infallible>(resp)
            }
        },
    );

    hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
        .http2_only()
        .serve_connection(TokioIo::new(stream), svc)
        .await
        .ok();
}

async fn serve_passthrough_connection(
    stream: tokio::net::TcpStream,
    proxy: TransparentProxy,
) {
    let svc = hyper::service::service_fn(
        move |req: http::Request<hyper::body::Incoming>| {
            let mut proxy = proxy.clone();
            async move {
                let (parts, body) = req.into_parts();
                let body = body
                    .map_err(|e| Status::internal(e.to_string()))
                    .boxed_unsync();
                let req = http::Request::from_parts(parts, body);

                let resp = match proxy.call(req).await {
                    Ok(resp) => resp,
                    Err(e) => status_to_response(error_to_status(e)),
                };

                Ok::<_, Infallible>(resp)
            }
        },
    );

    hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
        .http2_only()
        .serve_connection(TokioIo::new(stream), svc)
        .await
        .ok();
}
