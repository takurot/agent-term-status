use clap::Command;

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
}

fn main() {
    cli(invoked_name()).get_matches();
}
