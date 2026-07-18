use clap::Command;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("ATS_BUILD_GIT"), ")");

fn cli() -> Command {
    Command::new("ats")
        .bin_name("ats")
        .version(VERSION)
        .about("Visualize AI coding agent state in your terminal")
        .arg_required_else_help(true)
}

fn main() {
    cli().get_matches();
}
