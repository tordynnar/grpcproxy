use std::net::SocketAddr;
use std::pin::Pin;

use tokio::sync::oneshot;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use crate::pb::{
    echo_service_server::{EchoService, EchoServiceServer},
    math_service_server::{MathService, MathServiceServer},
    AddRequest, AddResponse, EchoRequest, EchoResponse, FibRequest, FibResponse,
};

pub struct EchoServiceImpl;

#[tonic::async_trait]
impl EchoService for EchoServiceImpl {
    async fn unary_echo(&self, req: Request<EchoRequest>) -> Result<Response<EchoResponse>, Status> {
        let msg = req.into_inner().message;
        Ok(Response::new(EchoResponse {
            message: msg,
            source: "backend".into(),
        }))
    }

    type ServerStreamEchoStream =
        Pin<Box<dyn Stream<Item = Result<EchoResponse, Status>> + Send>>;

    async fn server_stream_echo(
        &self,
        req: Request<EchoRequest>,
    ) -> Result<Response<Self::ServerStreamEchoStream>, Status> {
        let msg = req.into_inner().message;
        let stream = async_stream::stream! {
            for i in 0..3 {
                yield Ok(EchoResponse {
                    message: format!("{} #{}", msg, i + 1),
                    source: "backend".into(),
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
            message: messages.join(" "),
            source: "backend".into(),
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
                    message: r.message,
                    source: "backend".into(),
                });
            }
        };
        Ok(Response::new(Box::pin(output)))
    }
}

pub struct MathServiceImpl;

#[tonic::async_trait]
impl MathService for MathServiceImpl {
    async fn add(&self, req: Request<AddRequest>) -> Result<Response<AddResponse>, Status> {
        let inner = req.into_inner();
        if inner.a == 0 && inner.b == 0 {
            return Err(Status::invalid_argument("both operands are zero"));
        }
        Ok(Response::new(AddResponse {
            result: inner.a + inner.b,
            source: "backend".into(),
        }))
    }

    type FibonacciStream = Pin<Box<dyn Stream<Item = Result<FibResponse, Status>> + Send>>;

    async fn fibonacci(
        &self,
        req: Request<FibRequest>,
    ) -> Result<Response<Self::FibonacciStream>, Status> {
        let count = req.into_inner().count;
        let stream = async_stream::stream! {
            let (mut a, mut b): (i64, i64) = (0, 1);
            for _ in 0..count {
                yield Ok(FibResponse {
                    value: a,
                    source: "backend".into(),
                });
                let tmp = a + b;
                a = b;
                b = tmp;
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }
}

pub async fn start_backend(
    addr: SocketAddr,
) -> Result<(SocketAddr, oneshot::Sender<()>), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let local_addr = listener.local_addr()?;

    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(EchoServiceServer::new(EchoServiceImpl))
            .add_service(MathServiceServer::new(MathServiceImpl))
            .serve_with_incoming_shutdown(
                tokio_stream::wrappers::TcpListenerStream::new(listener),
                async { rx.await.ok(); },
            )
            .await
            .unwrap();
    });

    Ok((local_addr, tx))
}
