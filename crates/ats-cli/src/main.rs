use clap::{Arg, Command};
use clap_complete::{generate, Shell};

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
}

fn main() {
    let executable_name = invoked_name();
    let matches = cli(executable_name.clone()).get_matches();
    if let Some(event) = matches.subcommand_matches("event") {
        let state = event
            .get_one::<String>("state")
            .expect("state is a required arg");
        event_prototype::run(state);
    } else if let Some(completions) = matches.subcommand_matches("completions") {
        let shell = completions
            .get_one::<String>("shell")
            .expect("shell is a required arg");
        let mut cmd = cli(executable_name);
        print_completions(shell.as_str(), &mut cmd);
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
