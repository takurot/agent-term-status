use clap::{Arg, ArgAction, Command};
use std::process::ExitCode;

mod daemon_commands;
mod event_prototype;

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
                .about("Manage the ats daemon")
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("start")
                        .about("Start the daemon (detaches by default)")
                        .arg(
                            Arg::new("foreground")
                                .long("foreground")
                                .action(ArgAction::SetTrue)
                                .help("Run daemon in foreground"),
                        ),
                )
                .subcommand(Command::new("stop").about("Stop the daemon via its PID file"))
                .subcommand(
                    Command::new("status")
                        .about("Check if the daemon is running")
                        .arg(
                            Arg::new("json")
                                .long("json")
                                .action(ArgAction::SetTrue)
                                .help("Output in JSON format"),
                        ),
                ),
        )
}

fn main() -> ExitCode {
    let matches = cli(invoked_name()).get_matches();

    if let Some(event) = matches.subcommand_matches("event") {
        let state = event
            .get_one::<String>("state")
            .expect("state is a required arg");
        event_prototype::run(state);
        ExitCode::SUCCESS
    } else if let Some(daemon) = matches.subcommand_matches("daemon") {
        match daemon.subcommand() {
            Some(("start", args)) => {
                let foreground = args.get_flag("foreground");
                daemon_commands::start(foreground)
            }
            Some(("stop", _)) => daemon_commands::stop(),
            Some(("status", args)) => {
                let json = args.get_flag("json");
                daemon_commands::status(json)
            }
            _ => {
                eprintln!("unknown daemon subcommand");
                ExitCode::FAILURE
            }
        }
    } else {
        ExitCode::SUCCESS
    }
}
