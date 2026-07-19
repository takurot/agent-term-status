use clap::{Arg, ArgAction, Command};
use clap_complete::{generate, Shell};
use std::process::ExitCode;

mod daemon_commands;
mod doctor;
mod event_prototype;
mod query_commands;
mod socket_client;
mod theme_commands;

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
            Command::new("completions")
                .about("Generate shell completions to stdout")
                .arg(
                    Arg::new("shell")
                        .required(true)
                        .value_name("SHELL")
                        .value_parser(["bash", "zsh", "fish"])
                        .help("Target shell"),
                ),
        )
        .subcommand(
            Command::new("theme")
                .about("Manage themes")
                .arg_required_else_help(true)
                .subcommand(Command::new("list").about("List bundled and user themes"))
                .subcommand(
                    Command::new("preview")
                        .about("Preview a theme's state colors and symbols")
                        .arg(
                            Arg::new("name")
                                .required(true)
                                .value_name("THEME")
                                .help("Theme name to preview"),
                        ),
                )
                .subcommand(
                    Command::new("apply")
                        .about("Set the active theme in user config")
                        .arg(
                            Arg::new("name")
                                .required(true)
                                .value_name("THEME")
                                .help("Theme name to apply"),
                        ),
                ),
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
                )
                .subcommand(
                    Command::new("enable")
                        .about("Install launchd plist to auto-start daemon on login"),
                )
                .subcommand(Command::new("disable").about("Unload and remove launchd plist")),
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

fn main() -> ExitCode {
    let executable_name = invoked_name();
    let matches = cli(executable_name.clone()).get_matches();
    if let Some(event) = matches.subcommand_matches("event") {
        let state = event
            .get_one::<String>("state")
            .expect("state is a required arg");
        event_prototype::run(state);
        ExitCode::SUCCESS
    } else if let Some(completions) = matches.subcommand_matches("completions") {
        let shell = completions
            .get_one::<String>("shell")
            .expect("shell is a required arg");
        let mut cmd = cli(executable_name);
        print_completions(shell.as_str(), &mut cmd);
        ExitCode::SUCCESS
    } else if let Some(theme) = matches.subcommand_matches("theme") {
        if theme.subcommand_matches("list").is_some() {
            theme_commands::run_list();
        } else if let Some(preview) = theme.subcommand_matches("preview") {
            let name = preview
                .get_one::<String>("name")
                .expect("name is a required arg");
            theme_commands::run_preview(name);
        } else if let Some(apply) = theme.subcommand_matches("apply") {
            let name = apply
                .get_one::<String>("name")
                .expect("name is a required arg");
            theme_commands::run_apply(name);
        }
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
            Some(("enable", _)) => {
                daemon_commands::run_enable();
                ExitCode::SUCCESS
            }
            Some(("disable", _)) => {
                daemon_commands::run_disable();
                ExitCode::SUCCESS
            }
            _ => {
                eprintln!("unknown daemon subcommand");
                ExitCode::FAILURE
            }
        }
    } else if let Some(status) = matches.subcommand_matches("status") {
        let session = status.get_one::<String>("session").map(|s| s.as_str());
        query_commands::run_status(session);
        ExitCode::SUCCESS
    } else if let Some(list) = matches.subcommand_matches("list") {
        let json = list.get_flag("json");
        query_commands::run_list(json);
        ExitCode::SUCCESS
    } else if let Some(logs) = matches.subcommand_matches("logs") {
        let tail = logs.get_flag("tail");
        let level = logs.get_one::<String>("level").map(|s| s.as_str());
        query_commands::run_logs(tail, level);
        ExitCode::SUCCESS
    } else if let Some(dr) = matches.subcommand_matches("doctor") {
        let fix = dr.get_flag("fix");
        doctor::run_doctor(fix);
        ExitCode::SUCCESS
    } else {
        ExitCode::SUCCESS
    }
}

fn print_completions(shell: &str, cmd: &mut Command) {
    match shell {
        "bash" => generate(Shell::Bash, cmd, "ats", &mut std::io::stdout()),
        "zsh" => generate(Shell::Zsh, cmd, "ats", &mut std::io::stdout()),
        "fish" => generate(Shell::Fish, cmd, "ats", &mut std::io::stdout()),
        _ => {
            eprintln!("unsupported shell: {shell}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completions_zsh_emits_something() {
        let mut cmd = cli("ats".to_string());
        let mut output = Vec::new();
        generate(Shell::Zsh, &mut cmd, "ats", &mut output);
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.contains("#compdef ats"),
            "zsh completions should contain compdef"
        );
        assert!(output.contains("ats"), "should reference the command name");
    }

    #[test]
    fn completions_bash_emits_something() {
        let mut cmd = cli("ats".to_string());
        let mut output = Vec::new();
        generate(Shell::Bash, &mut cmd, "ats", &mut output);
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.contains("ats"),
            "bash completions should reference the command name"
        );
        assert!(!output.is_empty());
    }

    #[test]
    fn completions_fish_emits_something() {
        let mut cmd = cli("ats".to_string());
        let mut output = Vec::new();
        generate(Shell::Fish, &mut cmd, "ats", &mut output);
        let output = String::from_utf8(output).unwrap();
        assert!(
            output.contains("ats"),
            "fish completions should reference the command name"
        );
        assert!(!output.is_empty());
    }

    #[test]
    fn version_contains_git_describe() {
        let version = VERSION;
        assert!(
            version.contains("0.1.0"),
            "version should contain crate version"
        );
        assert!(
            version.contains("("),
            "version should contain git describe in parentheses"
        );
        assert!(
            version.contains(")"),
            "version should contain git describe in parentheses"
        );
    }

    #[test]
    fn invoked_name_uses_binary_name() {
        let name = invoked_name();
        assert!(!name.is_empty(), "invoked_name should return something");
    }
}
