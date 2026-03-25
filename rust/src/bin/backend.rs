use clap::Parser;
use std::net::SocketAddr;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:50051")]
    addr: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let (addr, _shutdown) = grpcproxy::backend::start_backend(args.addr).await?;
    println!("backend listening on {}", addr);
    tokio::signal::ctrl_c().await?;
    println!("\nshutting down...");
    Ok(())
}
