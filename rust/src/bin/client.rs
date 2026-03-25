use clap::Parser;
use grpcproxy::pb::{
    echo_service_client::EchoServiceClient, math_service_client::MathServiceClient, AddRequest,
    EchoRequest, FibRequest,
};
use tokio_stream::StreamExt;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:50052")]
    addr: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut echo = EchoServiceClient::connect(args.addr.clone()).await?;
    let mut math = MathServiceClient::connect(args.addr.clone()).await?;

    // 1. UnaryEcho (intercepted)
    println!("--- UnaryEcho (intercepted) ---");
    let resp = echo
        .unary_echo(EchoRequest {
            message: "hello world".into(),
        })
        .await?
        .into_inner();
    println!("  message={:?} source={:?}", resp.message, resp.source);

    // 2. ServerStreamEcho (intercepted)
    println!("--- ServerStreamEcho (intercepted) ---");
    let mut stream = echo
        .server_stream_echo(EchoRequest {
            message: "hello".into(),
        })
        .await?
        .into_inner();
    while let Some(resp) = stream.next().await {
        let resp = resp?;
        println!("  message={:?} source={:?}", resp.message, resp.source);
    }

    // 3. ClientStreamEcho (intercepted)
    println!("--- ClientStreamEcho (intercepted) ---");
    let reqs = tokio_stream::iter(
        ["hello", "world", "foo"]
            .iter()
            .map(|m| EchoRequest { message: m.to_string() }),
    );
    let resp = echo.client_stream_echo(reqs).await?.into_inner();
    println!("  message={:?} source={:?}", resp.message, resp.source);

    // 4. BidiStreamEcho (intercepted)
    println!("--- BidiStreamEcho (intercepted) ---");
    let reqs = tokio_stream::iter(
        ["hello", "world", "foo"]
            .iter()
            .map(|m| EchoRequest { message: m.to_string() }),
    );
    let mut stream = echo.bidi_stream_echo(reqs).await?.into_inner();
    while let Some(resp) = stream.next().await {
        let resp = resp?;
        println!("  message={:?} source={:?}", resp.message, resp.source);
    }

    // 5. Add (forwarded)
    println!("--- Add (forwarded) ---");
    let resp = math.add(AddRequest { a: 17, b: 25 }).await?.into_inner();
    println!("  result={} source={:?}", resp.result, resp.source);

    // 6. Fibonacci (forwarded)
    println!("--- Fibonacci (forwarded) ---");
    let mut stream = math.fibonacci(FibRequest { count: 10 }).await?.into_inner();
    while let Some(resp) = stream.next().await {
        let resp = resp?;
        println!("  value={} source={:?}", resp.value, resp.source);
    }

    Ok(())
}
