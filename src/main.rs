mod args;
mod config;
mod connection;
mod delegate;
mod error;
mod events;
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
use kaspa_utils::fd_budget::try_set_fd_limit;
use resolver::Resolver;
use result::Result;
use std::sync::Arc;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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

    config::init()?;

    match args.action {
        Action::Test => {
            let nodes = test_config()?;
            if args.verbose {
                for node in nodes.iter() {
                    println!("{}", node.address);
                }
            }
        }
        Action::Login => {
            println!();
            config::generate_key()?;
            println!();
        }
        Action::Pack => {
            println!();
            config::pack()?;
            println!();
        }
        Action::Unpack => {
            println!();
            config::unpack()?;
            println!();
        }
        Action::Update => {
            println!();
            config::update_global_config().await?;
            println!();
        }
        Action::Run => {
            if let Err(err) = try_set_fd_limit(Limits::fd()) {
                log_error!("FD Limit", "{err}");
            }

            if args.trace {
                workflow_log::set_log_level(workflow_log::LevelFilter::Trace);
            } else {
                workflow_log::set_log_level(workflow_log::LevelFilter::Info);
            }
            panic::init_ungraceful_panic_handler();

            println!();
            println!("Kaspa RPC resolver v{}", env!("CARGO_PKG_VERSION"));

            tracing_subscriber::fmt::init();

            let resolver = Arc::new(Resolver::try_new(&args)?);
            resolver.init_http_server().await?;
            resolver.start().await?;
            resolver.listen().await?;
            resolver.stop().await?;
        }
    }

    Ok(())
}
