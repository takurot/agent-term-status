use clap::{Arg, Command};

mod event_prototype;
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
}

fn main() {
    let matches = cli(invoked_name()).get_matches();
    if let Some(event) = matches.subcommand_matches("event") {
        let state = event
            .get_one::<String>("state")
            .expect("state is a required arg");
        event_prototype::run(state);
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
    }
}
