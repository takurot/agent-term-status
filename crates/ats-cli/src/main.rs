use clap::{Arg, Command};
use std::process::ExitCode;

mod event_prototype;
mod standalone_render;

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
                .about("Send a manual state event (standalone render when daemon is down)")
                .arg(Arg::new("state").required(true).value_name("STATE").help(
                    "Agent state in lowercase: idle|working|attention|risk|result|error|unknown",
                )),
        )
        .subcommand(
            Command::new("reset").about("Reset terminal pane visuals to default (standalone)"),
        )
}

#[tokio::main]
async fn main() -> ExitCode {
    let matches = cli(invoked_name()).get_matches();

    if let Some(event) = matches.subcommand_matches("event") {
        let state = event
            .get_one::<String>("state")
            .expect("state is a required arg");

        let socket_path = standalone_render::daemon_socket_path();
        let daemon_up = standalone_render::daemon_reachable(&socket_path);

        if daemon_up {
            // Daemon is available — delegate to it (future: send via socket).
            // For now, prototype path handles the render directly.
            event_prototype::run(state);
        } else {
            // Daemon is unreachable — standalone mode.
            let agent_state = event_prototype::parse_state(state);
            match agent_state {
                Some(as_) => {
                    eprintln!("ats: daemon unreachable — rendering in standalone mode");
                    standalone_render::standalone_render(as_, None).await;
                }
                None => {
                    eprintln!("ats event: unknown state {state:?}, ignoring");
                }
            }
        }
        ExitCode::SUCCESS
    } else if matches.subcommand_matches("reset").is_some() {
        standalone_render::standalone_reset().await;
        ExitCode::SUCCESS
    } else {
        ExitCode::SUCCESS
    }
}
