use clap::{Arg, Command};

mod event_prototype;
mod hook_commands;
mod socket_client;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("ATS_BUILD_GIT"), ")");

fn invoked_name() -> String {
    std::env::args_os()
        .next()
        .map(std::path::PathBuf::from)
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "ats".to_string())
}

fn cli(name: String) -> Command {
    Command::new("ats")
        .bin_name(name.clone())
        .display_name(name)
        .version(VERSION)
        .about("Visualize AI coding agent state in your terminal")
        .arg_required_else_help(true)
        .subcommand(
            Command::new("ingest")
                .about("Ingest hook event from stdin (hook path, fail-open)")
                .arg(
                    Arg::new("provider")
                        .long("provider")
                        .short('p')
                        .required(true)
                        .value_name("NAME")
                        .help("Provider name (e.g. claude, opencode)"),
                ),
        )
        .subcommand(
            Command::new("event")
                .about("Send a manual state event")
                .arg(Arg::new("state").required(true).value_name("STATE").help(
                    "Agent state in lowercase: idle|working|attention|risk|result|error|unknown",
                ))
                .arg(
                    Arg::new("activity")
                        .long("activity")
                        .short('a')
                        .value_name("LABEL")
                        .help("Activity label (e.g. 'Running tests')"),
                )
                .arg(
                    Arg::new("session")
                        .long("session")
                        .short('s')
                        .value_name("ID")
                        .help("Session identifier"),
                ),
        )
        .subcommand(
            Command::new("reset")
                .about("Reset renderer state")
                .arg(
                    Arg::new("all")
                        .long("all")
                        .help("Reset all sessions")
                        .action(clap::ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("session")
                        .long("session")
                        .short('s')
                        .value_name("ID")
                        .help("Reset a specific session"),
                ),
        )
}

fn main() {
    let matches = cli(invoked_name()).get_matches();
    if let Some(ingest) = matches.subcommand_matches("ingest") {
        let provider = ingest
            .get_one::<String>("provider")
            .expect("provider is a required arg");
        hook_commands::run_ingest(provider);
    } else if let Some(event) = matches.subcommand_matches("event") {
        let state = event
            .get_one::<String>("state")
            .expect("state is a required arg");
        let activity = event.get_one::<String>("activity").map(|s| s.as_str());
        let session = event.get_one::<String>("session").map(|s| s.as_str());
        hook_commands::run_event(state, activity, session);
    } else if let Some(reset) = matches.subcommand_matches("reset") {
        let all = reset.get_flag("all");
        let session = reset.get_one::<String>("session").map(|s| s.as_str());
        hook_commands::run_reset(all, session);
    }
}
