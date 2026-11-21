use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Command-line interface for trek.
#[derive(Debug, Parser)]
#[command(name = "trek", about = "Archive tools", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a ZIP archive from .pack include patterns.
    Pack(PackArgs),
}

#[derive(Debug, Args)]
pub struct PackArgs {
    /// Input directory containing a .pack include-pattern file.
    #[arg(long, value_name = "PATH", default_value = ".")]
    pub input: PathBuf,

    /// Output ZIP archive (default: the input folder name plus `.zip`).
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Preview the files that would be included without writing an archive.
    #[arg(long)]
    pub dry_run: bool,

    /// Print a Markdown report to stdout.
    #[arg(long)]
    pub report: bool,

    /// Print the SHA-256 checksum of the written archive.
    #[arg(long)]
    pub sha256: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pack_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "trek",
            "pack",
            "--input",
            "src",
            "--output",
            "files.zip",
            "--dry-run",
            "--report",
            "--sha256",
        ])?;

        match cli.command {
            Command::Pack(args) => {
                assert_eq!(args.input, PathBuf::from("src"));
                assert_eq!(args.output, Some(PathBuf::from("files.zip")));
                assert!(args.dry_run);
                assert!(args.report);
                assert!(args.sha256);
            }
        }

        Ok(())
    }

    #[test]
    fn defaults_input_to_current_directory() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["trek", "pack"])?;

        match cli.command {
            Command::Pack(args) => {
                assert_eq!(args.input, PathBuf::from("."));
                assert_eq!(args.output, None);
                assert!(!args.dry_run);
                assert!(!args.report);
                assert!(!args.sha256);
            }
        }

        Ok(())
    }
}
