use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::cli::{PackArgs, ReleaseArgs};
use crate::pack;
use crate::report::{Checksum, Outcome, ReleaseVersion, render_release};
use crate::version::{self, Bump, VersionChange};

/// Execute the `release` subcommand: bump the version, then pack.
///
/// The output directory is created when missing. A successful version bump is
/// not rolled back if packing fails, matching the bump-then-pack order.
///
/// On success, prints the Markdown Release Report. Failures are returned as
/// errors and printed by `main` as plain text, never as a Markdown report.
///
/// # Errors
///
/// Returns an error if the version cannot be bumped, the output directory
/// cannot be created, the pattern file cannot be read, validation fails, the
/// archive cannot be written or hashed, or the report cannot be printed.
pub fn run(args: &ReleaseArgs) -> Result<()> {
    let bump =
        Bump::from_flags(args.patch, args.minor, args.major).context("invalid version flags")?;
    let change = version::bump_manifest(&args.manifest, bump)?;

    let input = pack_input(&args.manifest);
    let output = args.output.join(pack::default_output(&input));
    fs::create_dir_all(&args.output).with_context(|| {
        format!(
            "failed to create output directory {}",
            args.output.display()
        )
    })?;

    let pack_args = PackArgs {
        input,
        output: Some(output),
        dry_run: false,
        report: false,
        sha256: args.sha256,
    };
    let outcome = pack::execute(&pack_args)?;
    if let Some(failure) = outcome.failure {
        return Err(failure);
    }

    // `pack::execute` records a hashing error in `failure`, so a successful
    // outcome with `--sha256` always carries a checksum.
    let checksum = if args.sha256 {
        Some(match outcome.checksum.as_deref() {
            Some(checksum) => Checksum::Value(checksum),
            None => Checksum::Unavailable,
        })
    } else {
        None
    };

    write_stdout(&render_release(
        &args.manifest,
        release_version(&change),
        &outcome.manifest,
        &outcome.plan,
        &outcome.output,
        Outcome::Written,
        checksum,
    ))
}

fn pack_input(manifest: &Path) -> PathBuf {
    manifest
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn release_version(change: &VersionChange) -> ReleaseVersion<'_> {
    match change.next.as_deref() {
        Some(next) => ReleaseVersion::Bumped {
            from: &change.current,
            to: next,
        },
        None => ReleaseVersion::Unchanged(&change.current),
    }
}

fn write_stdout(text: &str) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout
        .write_all(text.as_bytes())
        .context("failed to write to stdout")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_input_uses_manifest_parent() {
        assert_eq!(
            pack_input(Path::new("resource/fxmanifest.lua")),
            PathBuf::from("resource")
        );
    }

    #[test]
    fn pack_input_uses_current_directory_for_bare_manifest() {
        assert_eq!(pack_input(Path::new("fxmanifest.lua")), PathBuf::from("."));
    }

    #[test]
    fn release_version_reports_bumped_and_unchanged_versions() {
        let bumped = VersionChange {
            current: "1.2.3".into(),
            next: Some("1.2.4".into()),
        };
        assert_eq!(
            release_version(&bumped),
            ReleaseVersion::Bumped {
                from: "1.2.3",
                to: "1.2.4",
            }
        );

        let unchanged = VersionChange {
            current: "1.2.3".into(),
            next: None,
        };
        assert_eq!(
            release_version(&unchanged),
            ReleaseVersion::Unchanged("1.2.3")
        );
    }
}
