use clap::{Arg, Command};

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
                .about("Manage the ats-daemon service")
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("enable")
                        .about("Install launchd plist to auto-start daemon on login"),
                )
                .subcommand(Command::new("disable").about("Unload and remove launchd plist"))
                .subcommand(Command::new("status").about("Show daemon status")),
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
    }
}
