mod args;
mod config;
mod connection;
mod delegate;
mod error;
mod group;
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
mod services;
mod tpl;
mod transport;
mod utils;

use crate::config::*;
use args::*;
use error::Error;
use resolver::Resolver;
use result::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        match error {
            Error::Config(s) => {
                log_error!("Config", "{s}");
            }
            error => {
                eprintln!("Error: {}", error);
            }
        }
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Arc::new(Args::parse());

    if args.test {
        let nodes = test_config()?;
        if args.verbose {
            for node in nodes.iter() {
                println!("{}", node.address);
            }
        }
        return Ok(());
    }

    workflow_log::set_log_level(workflow_log::LevelFilter::Info);
    panic::init_ungraceful_panic_handler();

    println!();
    println!(
        "Kaspa wRPC Resolver v{} starting...",
        env!("CARGO_PKG_VERSION")
    );

    tracing_subscriber::fmt::init();

    let nodes = load_config()?;

    let resolver = Arc::new(Resolver::try_new(nodes)?);
    resolver.init_http_server(&args).await?;
    resolver.start().await?;
    resolver.listen().await?;
    resolver.stop().await?;
    Ok(())
}
