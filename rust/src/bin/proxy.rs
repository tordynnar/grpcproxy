use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:50052")]
    addr: SocketAddr,
    #[arg(long, default_value = "127.0.0.1:50051")]
    backend: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let (addr, _shutdown) = grpcproxy::proxy::start_proxy(args.addr, &args.backend).await?;
    println!("proxy listening on {} (backend: {})", addr, args.backend);
    tokio::signal::ctrl_c().await?;
    println!("\nshutting down...");
    Ok(())
}
