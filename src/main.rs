mod args;
mod config;
mod connection;
mod error;
mod imports;
mod log;
mod monitor;
mod node;
mod panic;
mod params;
mod path;
mod resolver;
mod result;
mod rpc;
mod transport;

use args::*;
use resolver::Resolver;
use result::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Arc::new(Args::parse());

    workflow_log::set_log_level(workflow_log::LevelFilter::Info);
    panic::init_ungraceful_panic_handler();

    println!();
    println!(
        "Kaspa wRPC Resolver v{} starting...",
        env!("CARGO_PKG_VERSION")
    );

    tracing_subscriber::fmt::init();

    let resolver = Arc::new(Resolver::default());
    resolver.init_http_server(&args).await?;
    resolver.start().await?;
    resolver.listen().await?;
    resolver.stop().await?;
    Ok(())
}
