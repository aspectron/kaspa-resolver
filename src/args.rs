pub use clap::Parser;
use std::str::FromStr;

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
    // / Show node data on each election
    // pub election: bool,
    /// Enable resolver status access via `/status`
    pub status: bool,
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
            // .arg(arg!(--election "Show node data on each election"))
            .arg(arg!(--status "Enable `/status` endpoint"))
            .arg(
                Arg::new("rate-limit")
                    .long("rate-limit")
                    .value_name("REQUESTS:SECONDS")
                    .num_args(0..=1)
                    .require_equals(true)
                    .help("Optional rate limit in the form `<requests>:<seconds>`"),
            )
            .arg(
                Arg::new("listen")
                    .long("listen")
                    .value_name("INTERFACE:PORT")
                    .num_args(0..=1)
                    .require_equals(true)
                    .help("listen interface and port [default: 127.0.0.1:8989]"),
            )
            .subcommand(Command::new("test").about("Test configuration"))
            .subcommand(Command::new("login").about("Create local update key"))
            .subcommand(Command::new("pack").about("Package configuration"))
            .subcommand(Command::new("unpack").about("Package configuration"))
            .subcommand(Command::new("update").about("Update configuration from GitHub"))
            // .subcommand(Command::new("reload").about("Reload configuration"))
        ;

        let matches = cmd.get_matches();

        let trace = matches.get_one::<bool>("trace").cloned().unwrap_or(false);
        let verbose = matches.get_one::<bool>("verbose").cloned().unwrap_or(false);
        // let election = matches.get_one::<bool>("trace").cloned().unwrap_or(false);
        let status = matches.get_one::<bool>("status").cloned().unwrap_or(false);

        // let enable_debug_mode = matches.get_one::<bool>("debug").cloned().unwrap_or(false);

        // let network_id = matches
        //     .get_one::<NetworkId>("network")
        //     .cloned()
        //     .unwrap_or(NetworkId::with_suffix(NetworkType::Testnet, 11));

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
            // election,
            status,
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
