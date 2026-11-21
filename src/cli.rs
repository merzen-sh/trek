use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand};

/// Command-line interface for trek.
#[derive(Debug, Parser)]
#[command(name = "trek", about = "Archive and resource version tools", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a ZIP archive from .pack include patterns.
    Pack(PackArgs),
    /// Show or bump the resource version in fxmanifest.lua.
    Version(VersionArgs),
    /// Bump the resource version and create a release archive.
    Release(ReleaseArgs),
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

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("bump")
        .args(["patch", "minor", "major"])
        .multiple(false)
))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "command-line flags are naturally booleans"
)]
pub struct VersionArgs {
    /// Bump the patch version.
    #[arg(long)]
    pub patch: bool,

    /// Bump the minor version and reset patch.
    #[arg(long)]
    pub minor: bool,

    /// Bump the major version and reset minor/patch.
    #[arg(long)]
    pub major: bool,

    /// Print only the raw version for scripts and CI.
    #[arg(long)]
    pub ci: bool,

    /// Path to the target fxmanifest.lua.
    #[arg(
        short = 'm',
        long,
        value_name = "PATH",
        default_value = "fxmanifest.lua"
    )]
    pub manifest: PathBuf,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("bump")
        .args(["patch", "minor", "major"])
        .multiple(false)
))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "command-line flags are naturally booleans"
)]
pub struct ReleaseArgs {
    /// Bump the patch version.
    #[arg(long)]
    pub patch: bool,

    /// Bump the minor version and reset patch.
    #[arg(long)]
    pub minor: bool,

    /// Bump the major version and reset minor/patch.
    #[arg(long)]
    pub major: bool,

    /// Output directory for the ZIP archive, created when missing.
    #[arg(short = 'o', long, value_name = "PATH", default_value = ".")]
    pub output: PathBuf,

    /// Path to the target fxmanifest.lua.
    #[arg(
        short = 'm',
        long,
        value_name = "PATH",
        default_value = "fxmanifest.lua"
    )]
    pub manifest: PathBuf,

    /// Include the archive's SHA-256 checksum in the report.
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

        let Command::Pack(args) = cli.command else {
            anyhow::bail!("expected pack command");
        };
        assert_eq!(args.input, PathBuf::from("src"));
        assert_eq!(args.output, Some(PathBuf::from("files.zip")));
        assert!(args.dry_run);
        assert!(args.report);
        assert!(args.sha256);

        Ok(())
    }

    #[test]
    fn defaults_input_to_current_directory() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["trek", "pack"])?;

        let Command::Pack(args) = cli.command else {
            anyhow::bail!("expected pack command");
        };
        assert_eq!(args.input, PathBuf::from("."));
        assert_eq!(args.output, None);
        assert!(!args.dry_run);
        assert!(!args.report);
        assert!(!args.sha256);

        Ok(())
    }

    #[test]
    fn parses_version_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "trek",
            "version",
            "--patch",
            "--ci",
            "-m",
            "resource/fxmanifest.lua",
        ])?;

        let Command::Version(args) = cli.command else {
            anyhow::bail!("expected version command");
        };
        assert!(args.patch);
        assert!(!args.minor);
        assert!(!args.major);
        assert!(args.ci);
        assert_eq!(args.manifest, PathBuf::from("resource/fxmanifest.lua"));

        Ok(())
    }

    #[test]
    fn version_defaults_to_local_manifest() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["trek", "version"])?;

        let Command::Version(args) = cli.command else {
            anyhow::bail!("expected version command");
        };
        assert!(!args.patch);
        assert!(!args.minor);
        assert!(!args.major);
        assert!(!args.ci);
        assert_eq!(args.manifest, PathBuf::from("fxmanifest.lua"));

        Ok(())
    }

    #[test]
    fn version_rejects_multiple_bump_flags() {
        assert!(Cli::try_parse_from(["trek", "version", "--patch", "--minor"]).is_err());
    }

    #[test]
    fn parses_release_options() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from([
            "trek",
            "release",
            "--patch",
            "--sha256",
            "-o",
            "dist",
            "-m",
            "resource/fxmanifest.lua",
        ])?;

        let Command::Release(args) = cli.command else {
            anyhow::bail!("expected release command");
        };
        assert!(args.patch);
        assert!(!args.minor);
        assert!(!args.major);
        assert!(args.sha256);
        assert_eq!(args.output, PathBuf::from("dist"));
        assert_eq!(args.manifest, PathBuf::from("resource/fxmanifest.lua"));

        Ok(())
    }

    #[test]
    fn release_defaults_to_current_directory_and_local_manifest() -> anyhow::Result<()> {
        let cli = Cli::try_parse_from(["trek", "release"])?;

        let Command::Release(args) = cli.command else {
            anyhow::bail!("expected release command");
        };
        assert!(!args.patch);
        assert!(!args.minor);
        assert!(!args.major);
        assert!(!args.sha256);
        assert_eq!(args.output, PathBuf::from("."));
        assert_eq!(args.manifest, PathBuf::from("fxmanifest.lua"));

        Ok(())
    }

    #[test]
    fn release_rejects_multiple_bump_flags() {
        assert!(Cli::try_parse_from(["trek", "release", "--patch", "--major"]).is_err());
    }
}
