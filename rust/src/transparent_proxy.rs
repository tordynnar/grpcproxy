use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tonic::body::BoxBody;
use tower::{Service, ServiceExt};

/// A transparent gRPC proxy that forwards requests to a backend via a tonic Channel.
#[derive(Clone)]
pub struct TransparentProxy {
    channel: tonic::transport::Channel,
}

impl TransparentProxy {
    pub fn new_lazy(backend_addr: &str) -> Self {
        let endpoint = tonic::transport::Channel::from_shared(format!("http://{}", backend_addr))
            .expect("valid uri");
        let channel = endpoint.connect_lazy();
        TransparentProxy { channel }
    }
}

impl Service<http::Request<BoxBody>> for TransparentProxy {
    type Response = http::Response<BoxBody>;
    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<BoxBody>) -> Self::Future {
        // Clone the channel and use ready().await to properly handle poll_ready
        let channel = self.channel.clone();
        Box::pin(async move {
            let resp = channel
                .oneshot(req)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            let (parts, body) = resp.into_parts();
            let body = BoxBody::new(body);
            Ok(http::Response::from_parts(parts, body))
        })
    }
}
