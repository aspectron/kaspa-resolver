pub use clap::{ArgAction, Parser};
use std::path::PathBuf;
use std::str::FromStr;

use crate::{log_error, log_success};

#[derive(Debug)]
pub enum Action {
    Login,
    Pack,
    Unpack,
    Update,
    Test,
    Run,
}

#[derive(Debug)]
pub struct Args {
    /// HTTP server port
    pub listen: String,
    /// Optional rate limit in the form `<requests>:<seconds>`, where `requests` is the number of requests allowed per specified number of `seconds`
    pub rate_limit: Option<RateLimit>,
    /// Verbose mode
    pub verbose: bool,
    /// Tracing mode
    pub trace: bool,
    /// Auto-update
    pub auto_update: bool,
    /// Custom config file
    pub user_config: Option<PathBuf>,
    // Show node data on each election
    // pub election: bool,
    // Enable resolver status access via `/status`
    // pub status: bool,
    /// Action to execute
    pub action: Action,
}

impl Args {
    pub fn parse() -> Args {
        #[allow(unused)]
        use clap::{arg, command, Arg, Command};

        let cmd = Command::new("kaspa-resolver")
            .about(format!(
                "resolver v{}", crate::VERSION
            ))
            .arg(arg!(--version "Display software version"))
            .arg(arg!(--verbose "Enable verbose logging"))
            .arg(arg!(--trace "Enable trace log level"))
            // .arg(arg!(--auto-update "Poll configuration updates"))
            // .arg(arg!(--election "Show node data on each election"))
            // .arg(arg!(--status "Enable `/status` endpoint"))
            .arg(
                Arg::new("auto-update")
                    .long("auto-update")
                    .action(ArgAction::SetTrue)
                    .help("Poll configuration updates (public nodes only)"),
            )
            .arg(
                Arg::new("rate-limit")
                    .long("rate-limit")
                    .value_name("REQUESTS:SECONDS")
                    .num_args(1)
                    .require_equals(true)
                    .help("Optional rate limit in the form `<requests>:<seconds>`"),
            )
            .arg(
                Arg::new("config-file")
                    .long("config-file")
                    .value_name("config.toml file")
                    .num_args(1)
                    .require_equals(true)
                    .help("TOML config file (absolute or relative to working directory)"),
            )
            .arg(
                Arg::new("listen")
                    .long("listen")
                    .value_name("INTERFACE:PORT")
                    .num_args(1)
                    .require_equals(true)
                    .help("Listen on custom interface and port [default: 127.0.0.1:8989]"),
            )
            .subcommand(Command::new("test").about("Test configuration"))
            .subcommand(Command::new("login").about("Create local update key"))
            .subcommand(Command::new("pack").about("Pack configuration"))
            .subcommand(Command::new("unpack").about("Unpack configuration"))
            .subcommand(Command::new("update").about("Update configuration from GitHub"))
            // .subcommand(Command::new("reload").about("Reload configuration"))
        ;

        let matches = cmd.get_matches();

        let trace = matches.get_one::<bool>("trace").cloned().unwrap_or(false);
        let verbose = matches.get_one::<bool>("verbose").cloned().unwrap_or(false);
        let auto_update = matches
            .get_one::<bool>("auto-update")
            .cloned()
            .unwrap_or(false);

        if auto_update {
            log_success!("Update", "Enabling auto-update");
        }
        // let private_cluster = matches.get_one::<bool>("private-cluster").cloned().unwrap_or(false);
        // let election = matches.get_one::<bool>("election").cloned().unwrap_or(false);
        // let status = matches.get_one::<bool>("status").cloned().unwrap_or(false);

        let user_config = matches.get_one::<String>("config-file").cloned().map(|s| {
            if s.contains('~') {
                let s = s.replace(
                    "~",
                    dirs::home_dir()
                        .expect("Unable to obtain user home folder")
                        .to_str()
                        .unwrap(),
                );
                PathBuf::from(s)
            } else if !s.starts_with('/') {
                std::env::current_dir()
                    .expect("Unable to obtain current working directory")
                    .join(s)
            } else {
                PathBuf::from(s)
            }
        });

        if let Some(user_config) = &user_config {
            log_success!(
                "Config",
                "Using custom config file: `{}`",
                user_config.display()
            );
            if auto_update {
                log_error!(
                    "Config",
                    "Auto-update is not supported with custom local config file..."
                );
                log_error!("Config", "Halting...");
                std::process::exit(1);
            }
        }

        let rate_limit = matches.get_one::<RateLimit>("rate-limit").cloned();
        let listen = matches
            .get_one::<String>("listen")
            .cloned()
            .unwrap_or("127.0.0.1:8989".to_string());

        let action = if matches.get_one::<bool>("version").cloned().unwrap_or(false) {
            println!("v{}", crate::VERSION);
            std::process::exit(0);
        } else if let Some(_matches) = matches.subcommand_matches("test") {
            Action::Test
        } else if let Some(_matches) = matches.subcommand_matches("login") {
            Action::Login
        } else if let Some(_matches) = matches.subcommand_matches("pack") {
            Action::Pack
        } else if let Some(_matches) = matches.subcommand_matches("unpack") {
            Action::Unpack
        } else if let Some(_matches) = matches.subcommand_matches("update") {
            Action::Update
        } else {
            Action::Run
        };

        Args {
            trace,
            verbose,
            auto_update,
            user_config,
            // election,
            // status,
            listen,
            rate_limit,
            action,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RateLimit {
    pub requests: u64,
    pub period: u64,
}

impl FromStr for RateLimit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split_once(':');
        let (requests, period) = match parts {
            None | Some(("", _)) | Some((_, "")) => {
                return Err("invalid rate limit, must be `<requests>:<period>`".to_string());
            }
            Some(x) => x,
        };
        let requests = requests.parse().map_err(|_| {
            format!(
                "Unable to parse number of requests, the value must be an integer, supplied: {:?}",
                requests
            )
        })?;
        let period = period.parse().map_err(|_| {
            format!("Unable to parse period, the value must be an integer specifying number of seconds, supplied: {:?}", period)
        })?;

        Ok(RateLimit { requests, period })
    }
}
