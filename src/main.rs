use std::io::Write as _;

use clap::Parser;

use trek::cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Pack(args) => trek::pack::run(&args),
        Command::Version(args) => trek::version::run(&args),
        Command::Release(args) => trek::release::run(&args),
    };

    if let Err(error) = result {
        let _ = writeln!(std::io::stderr(), "error: {error:#}");
        std::process::exit(1);
    }
}
