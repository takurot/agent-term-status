use clap::{Arg, Command};

mod daemon_commands;
mod doctor;
mod event_prototype;
mod query_commands;
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
            Command::new("event")
                .about("Send a manual state event (Phase 0 prototype: tmux border only)")
                .arg(Arg::new("state").required(true).value_name("STATE").help(
                    "Agent state in lowercase: idle|working|attention|risk|result|error|unknown",
                )),
        )
        .subcommand(
            Command::new("daemon")
                .about("Manage the ats-daemon service")
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("enable")
                        .about("Install launchd plist to auto-start daemon on login"),
                )
                .subcommand(Command::new("disable").about("Unload and remove launchd plist"))
                .subcommand(Command::new("status").about("Show daemon status")),
        )
        .subcommand(
            Command::new("status")
                .about("Show daemon and session status")
                .arg(
                    Arg::new("session")
                        .long("session")
                        .short('s')
                        .value_name("ID")
                        .help("Show status for a specific session"),
                ),
        )
        .subcommand(
            Command::new("list").about("List all sessions").arg(
                Arg::new("json")
                    .long("json")
                    .help("Output in JSON format")
                    .action(clap::ArgAction::SetTrue),
            ),
        )
        .subcommand(
            Command::new("logs")
                .about("Read daemon log file (no redaction)")
                .arg(
                    Arg::new("tail")
                        .long("tail")
                        .help("Show last 20 lines")
                        .action(clap::ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("level")
                        .long("level")
                        .short('l')
                        .value_name("LEVEL")
                        .help("Filter by log level (trace, debug, info, warn, error)"),
                ),
        )
        .subcommand(
            Command::new("doctor")
                .about("Diagnose configuration and environment (SPEC 21 #9)")
                .arg(
                    Arg::new("fix")
                        .long("fix")
                        .help("Attempt automatic repairs")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
}

fn main() {
    let matches = cli(invoked_name()).get_matches();
    if let Some(event) = matches.subcommand_matches("event") {
        let state = event
            .get_one::<String>("state")
            .expect("state is a required arg");
        event_prototype::run(state);
    } else if let Some(daemon) = matches.subcommand_matches("daemon") {
        if daemon.subcommand_matches("enable").is_some() {
            daemon_commands::run_enable();
        } else if daemon.subcommand_matches("disable").is_some() {
            daemon_commands::run_disable();
        } else if daemon.subcommand_matches("status").is_some() {
            daemon_commands::run_status();
        }
    } else if let Some(status) = matches.subcommand_matches("status") {
        let session = status.get_one::<String>("session").map(|s| s.as_str());
        query_commands::run_status(session);
    } else if let Some(list) = matches.subcommand_matches("list") {
        let json = list.get_flag("json");
        query_commands::run_list(json);
    } else if let Some(logs) = matches.subcommand_matches("logs") {
        let tail = logs.get_flag("tail");
        let level = logs.get_one::<String>("level").map(|s| s.as_str());
        query_commands::run_logs(tail, level);
    } else if let Some(dr) = matches.subcommand_matches("doctor") {
        let fix = dr.get_flag("fix");
        doctor::run_doctor(fix);
    }
}
