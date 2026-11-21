use std::ffi::OsString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::cli::VersionArgs;

/// A numeric `MAJOR.MINOR.PATCH` version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

impl SemVer {
    fn parse(value: &str) -> Result<Self> {
        let mut parts = value.split('.');
        let major = parse_component(value, parts.next(), "major")?;
        let minor = parse_component(value, parts.next(), "minor")?;
        let patch = parse_component(value, parts.next(), "patch")?;

        if parts.next().is_some() {
            bail!("invalid version `{value}`: expected MAJOR.MINOR.PATCH");
        }

        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    fn bump(self, bump: Bump) -> Result<Self> {
        let (major, minor, patch) = match bump {
            Bump::Major => (
                self.major
                    .checked_add(1)
                    .ok_or_else(|| version_overflow(self))?,
                0,
                0,
            ),
            Bump::Minor => (
                self.major,
                self.minor
                    .checked_add(1)
                    .ok_or_else(|| version_overflow(self))?,
                0,
            ),
            Bump::Patch => (
                self.major,
                self.minor,
                self.patch
                    .checked_add(1)
                    .ok_or_else(|| version_overflow(self))?,
            ),
        };

        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

fn parse_component(version: &str, part: Option<&str>, name: &str) -> Result<u64> {
    let Some(part) = part else {
        bail!("invalid version `{version}`: missing {name} component");
    };

    if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("invalid version `{version}`: expected numeric MAJOR.MINOR.PATCH");
    }
    if part.len() > 1 && part.starts_with('0') {
        bail!("invalid version `{version}`: {name} component has a leading zero");
    }

    part.parse::<u64>()
        .with_context(|| format!("invalid version `{version}`: {name} component is too large"))
}

fn version_overflow(version: SemVer) -> anyhow::Error {
    anyhow!("cannot bump version `{version}`: numeric component overflow")
}

impl fmt::Display for SemVer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bump {
    Patch,
    Minor,
    Major,
}

impl Bump {
    fn from_args(args: &VersionArgs) -> Result<Option<Self>> {
        match (args.patch, args.minor, args.major) {
            (false, false, false) => Ok(None),
            (true, false, false) => Ok(Some(Self::Patch)),
            (false, true, false) => Ok(Some(Self::Minor)),
            (false, false, true) => Ok(Some(Self::Major)),
            _ => bail!("only one of --patch, --minor, or --major may be used"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct VersionSpan<'a> {
    start: usize,
    end: usize,
    value: &'a str,
}

/// Locate the version string in the first top-level `version` declaration,
/// skipping block comments and long strings.
fn find_version_span(contents: &str) -> Option<VersionSpan<'_>> {
    let (scan, bom_len) = match contents.strip_prefix('\u{feff}') {
        Some(rest) => (rest, '\u{feff}'.len_utf8()),
        None => (contents, 0),
    };
    let mut offset = bom_len;
    let mut block_end: Option<String> = None;

    for line in scan.split_inclusive('\n') {
        if let Some(end) = block_end.clone() {
            if let Some(close_index) = line.find(&end) {
                block_end = None;
                let remainder = &line[close_index + end.len()..];
                if let Some((open_end, close)) = long_bracket_open(remainder)
                    && !remainder[open_end..].contains(&close)
                {
                    block_end = Some(close);
                }
            }
            offset += line.len();
            continue;
        }

        if let Some((start, end)) = version_literal_span(line) {
            return Some(VersionSpan {
                start: offset + start,
                end: offset + end,
                value: &line[start..end],
            });
        }

        if let Some((open_end, close)) = long_bracket_open(line)
            && !line[open_end..].contains(&close)
        {
            block_end = Some(close);
        }

        offset += line.len();
    }

    None
}

fn long_bracket_open(line: &str) -> Option<(usize, String)> {
    let bytes = line.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                let quote = bytes[index];
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == b'\\' {
                        index += 2;
                    } else if bytes[index] == quote {
                        index += 1;
                        break;
                    } else {
                        index += 1;
                    }
                }
            }
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                return parse_long_bracket(bytes, index + 2);
            }
            b'[' => {
                if let Some(open) = parse_long_bracket(bytes, index) {
                    return Some(open);
                }
                index += 1;
            }
            _ => index += 1,
        }
    }

    None
}

fn parse_long_bracket(bytes: &[u8], start: usize) -> Option<(usize, String)> {
    if bytes.get(start) != Some(&b'[') {
        return None;
    }

    let mut index = start + 1;
    while bytes.get(index) == Some(&b'=') {
        index += 1;
    }
    if bytes.get(index) != Some(&b'[') {
        return None;
    }

    let equals = index - start - 1;
    let mut close = String::with_capacity(equals + 2);
    close.push(']');
    for _ in 0..equals {
        close.push('=');
    }
    close.push(']');
    Some((index + 1, close))
}

/// Return the byte range of the version literal inside one line.
fn version_literal_span(line: &str) -> Option<(usize, usize)> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let rest = trimmed.strip_prefix("version")?;
    let next = rest.chars().next()?;
    if next.is_alphanumeric() || next == '_' {
        return None;
    }

    let bytes = line.as_bytes();
    let mut position = leading + "version".len();
    position = skip_horizontal_whitespace(bytes, position);
    if matches!(bytes.get(position), Some(b'=' | b'(')) {
        position += 1;
        position = skip_horizontal_whitespace(bytes, position);
    }

    let quote = *bytes.get(position)?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }

    let content_start = position + 1;
    let content_end = content_start
        + bytes[content_start..]
            .iter()
            .position(|&byte| byte == quote)?;
    Some((content_start, content_end))
}

fn skip_horizontal_whitespace(bytes: &[u8], mut position: usize) -> usize {
    while matches!(bytes.get(position), Some(b' ' | b'\t')) {
        position += 1;
    }
    position
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VersionUpdate {
    current: SemVer,
    next: Option<SemVer>,
    updated: Option<String>,
}

fn update_version(contents: &str, bump: Option<Bump>) -> Result<VersionUpdate> {
    let (start, end, current) = {
        let span =
            find_version_span(contents).ok_or_else(|| anyhow!("no version declaration found"))?;
        let current = SemVer::parse(span.value)?;
        (span.start, span.end, current)
    };
    let next = bump.map(|bump| current.bump(bump)).transpose()?;
    let updated = next.map(|next| {
        let mut updated = contents.to_string();
        updated.replace_range(start..end, &next.to_string());
        updated
    });

    Ok(VersionUpdate {
        current,
        next,
        updated,
    })
}

const MAX_WRITE_ATTEMPTS: u32 = 100;

/// Write `contents` to `path` via a sibling temporary file and atomic rename.
///
/// Symlinks are resolved so the target is updated in place. File mode is
/// preserved. Ownership, extended attributes, and hard links are not preserved
/// because the rename replaces the directory entry.
///
/// # Errors
///
/// Returns an error if the manifest cannot be resolved or is not writable, or if
/// the temporary file cannot be written or renamed.
fn write_manifest(path: &Path, contents: &str) -> Result<()> {
    let target = fs::canonicalize(path)
        .with_context(|| format!("failed to resolve manifest {}", path.display()))?;

    // Keep read-only-file failures instead of bypassing them through rename.
    OpenOptions::new()
        .write(true)
        .open(&target)
        .with_context(|| format!("failed to write manifest {}", path.display()))?;

    let permissions = fs::metadata(&target)
        .with_context(|| format!("failed to inspect manifest {}", path.display()))?
        .permissions();

    for attempt in 0..MAX_WRITE_ATTEMPTS {
        let temporary = temporary_manifest_path(&target, attempt)?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => {
                let result = write_and_replace(file, &temporary, &target, &permissions, contents);
                if result.is_err() {
                    let _ = fs::remove_file(&temporary);
                }
                return result
                    .with_context(|| format!("failed to write manifest {}", path.display()));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to write manifest {}", path.display()));
            }
        }
    }

    bail!(
        "failed to create a temporary manifest for {}",
        path.display()
    )
}

fn write_and_replace(
    file: File,
    temporary: &Path,
    target: &Path,
    permissions: &fs::Permissions,
    contents: &str,
) -> Result<()> {
    let mut file = file;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(temporary, permissions.clone())?;
    fs::rename(temporary, target)?;
    Ok(())
}

fn temporary_manifest_path(target: &Path, attempt: u32) -> Result<PathBuf> {
    let name = target
        .file_name()
        .ok_or_else(|| anyhow!("manifest {} has no file name", target.display()))?;
    let mut temporary_name = OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(".{}.{attempt}.tmp", std::process::id()));
    Ok(target.with_file_name(temporary_name))
}

/// Execute the `version` subcommand.
///
/// # Errors
///
/// Returns an error if the manifest cannot be read or written, no version
/// declaration is found, the version is invalid, or a bump overflows.
pub fn run(args: &VersionArgs) -> Result<()> {
    let contents = fs::read_to_string(&args.manifest)
        .with_context(|| format!("failed to read manifest {}", args.manifest.display()))?;
    let bump = Bump::from_args(args)
        .with_context(|| format!("invalid version flags for {}", args.manifest.display()))?;
    let update = update_version(&contents, bump)
        .with_context(|| format!("failed to process manifest {}", args.manifest.display()))?;

    if let Some(updated) = &update.updated {
        write_manifest(&args.manifest, updated)?;
    }

    print_version(update.current, update.next, args.ci)
}

fn print_version(current: SemVer, next: Option<SemVer>, ci: bool) -> Result<()> {
    let output = if ci {
        let version = next.unwrap_or(current);
        format!("{version}\n")
    } else if let Some(next) = next {
        format!("Bumped version: {current} -> {next}\n")
    } else {
        format!("Current version: {current}\n")
    };

    write_stdout(&output)
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
    use crate::test_support::write_file;

    #[test]
    fn parses_numeric_semver() -> Result<()> {
        assert_eq!(
            SemVer::parse("1.2.3")?,
            SemVer {
                major: 1,
                minor: 2,
                patch: 3,
            }
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_versions() {
        assert!(SemVer::parse("1.2").is_err());
        assert!(SemVer::parse("1.2.3.4").is_err());
        assert!(SemVer::parse("1.2.x").is_err());
        assert!(SemVer::parse("01.2.3").is_err());
        assert!(SemVer::parse("").is_err());
    }

    #[test]
    fn bumps_versions() -> Result<()> {
        let version = SemVer::parse("1.2.3")?;

        assert_eq!(version.bump(Bump::Patch)?.to_string(), "1.2.4");
        assert_eq!(version.bump(Bump::Minor)?.to_string(), "1.3.0");
        assert_eq!(version.bump(Bump::Major)?.to_string(), "2.0.0");
        Ok(())
    }

    #[test]
    fn reports_component_overflow() -> Result<()> {
        let version = SemVer::parse(&format!("1.2.{}", u64::MAX))?;

        assert!(version.bump(Bump::Patch).is_err());
        Ok(())
    }

    #[test]
    fn preserves_each_syntax_style() -> Result<()> {
        let cases = [
            ("version '1.2.3'\n", "version '1.2.4'\n"),
            ("version(\"1.2.3\")\n", "version(\"1.2.4\")\n"),
            ("version = \"1.2.3\"\n", "version = \"1.2.4\"\n"),
            ("version = '1.2.3'\n", "version = '1.2.4'\n"),
            (
                "  version\t=\t\"1.2.3\" -- comment\n",
                "  version\t=\t\"1.2.4\" -- comment\n",
            ),
        ];

        for (contents, expected) in cases {
            let update = update_version(contents, Some(Bump::Patch))?;
            assert_eq!(
                update.next,
                Some(SemVer {
                    major: 1,
                    minor: 2,
                    patch: 4,
                })
            );
            assert_eq!(update.updated.as_deref(), Some(expected));
        }

        Ok(())
    }

    #[test]
    fn read_only_update_does_not_rewrite_contents() -> Result<()> {
        let update = update_version("version '1.2.3'\n", None)?;

        assert_eq!(update.current.to_string(), "1.2.3");
        assert_eq!(update.next, None);
        assert_eq!(update.updated, None);
        Ok(())
    }

    #[test]
    fn ignores_commented_version_declarations() -> Result<()> {
        let update = update_version("-- version '0.0.1'\nversion '1.2.3'\n", None)?;

        assert_eq!(update.current.to_string(), "1.2.3");
        Ok(())
    }

    #[test]
    fn preserves_line_endings() -> Result<()> {
        let update = update_version("version = \"1.2.3\"\r\n", Some(Bump::Minor))?;

        assert_eq!(update.updated.as_deref(), Some("version = \"1.3.0\"\r\n"));
        Ok(())
    }

    #[test]
    fn errors_when_version_declaration_is_missing() {
        assert!(update_version("return {}\n", None).is_err());
        assert!(update_version("version = get_version()\n", None).is_err());
        assert!(update_version("version == \"1.2.3\"\n", None).is_err());
    }

    #[test]
    fn rejects_multiple_bump_flags_from_args() {
        let args = VersionArgs {
            patch: true,
            minor: true,
            major: false,
            ci: false,
            manifest: PathBuf::from("fxmanifest.lua"),
        };

        assert!(Bump::from_args(&args).is_err());
    }

    #[test]
    fn ignores_versions_in_block_comments() -> Result<()> {
        let contents = "--[[\nversion '0.0.1'\n]]\nversion '1.2.3'\n";
        let update = update_version(contents, Some(Bump::Patch))?;

        assert_eq!(update.current.to_string(), "1.2.3");
        assert_eq!(
            update.updated.as_deref(),
            Some("--[[\nversion '0.0.1'\n]]\nversion '1.2.4'\n")
        );
        Ok(())
    }

    #[test]
    fn ignores_versions_in_long_strings() -> Result<()> {
        let contents = "local notes = [[\nversion '0.0.1'\n]]\nversion '1.2.3'\n";
        let update = update_version(contents, Some(Bump::Patch))?;

        assert_eq!(update.current.to_string(), "1.2.3");
        assert_eq!(
            update.updated.as_deref(),
            Some("local notes = [[\nversion '0.0.1'\n]]\nversion '1.2.4'\n")
        );
        Ok(())
    }

    #[test]
    fn ignores_leveled_long_brackets() -> Result<()> {
        let contents = "--[=[\nversion '0.0.1'\n]=]\nversion '1.2.3'\n";
        let update = update_version(contents, None)?;

        assert_eq!(update.current.to_string(), "1.2.3");
        Ok(())
    }

    #[test]
    fn ignores_long_brackets_inside_short_strings() -> Result<()> {
        let contents = "local marker = \"[[\"\nversion '1.2.3'\n";
        let update = update_version(contents, None)?;

        assert_eq!(update.current.to_string(), "1.2.3");
        Ok(())
    }

    #[test]
    fn handles_utf8_bom() -> Result<()> {
        let contents = "\u{feff}version '1.2.3'\n";
        let update = update_version(contents, Some(Bump::Patch))?;

        assert_eq!(update.updated.as_deref(), Some("\u{feff}version '1.2.4'\n"));
        Ok(())
    }

    #[test]
    fn write_failure_leaves_manifest_unchanged() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let manifest = dir.path().join("fxmanifest.lua");
        write_file(&manifest, "version '1.2.3'\n")?;
        let target = fs::canonicalize(&manifest)?;

        for attempt in 0..MAX_WRITE_ATTEMPTS {
            let temporary = temporary_manifest_path(&target, attempt)?;
            fs::write(temporary, "occupied")?;
        }

        let Err(error) = write_manifest(&manifest, "version '1.2.4'\n") else {
            anyhow::bail!("expected a write error");
        };

        assert!(format!("{error:#}").contains("failed to create a temporary manifest"));
        assert_eq!(fs::read_to_string(&manifest)?, "version '1.2.3'\n");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn write_manifest_preserves_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir()?;
        let manifest = dir.path().join("fxmanifest.lua");
        write_file(&manifest, "version '1.2.3'\n")?;
        fs::set_permissions(&manifest, fs::Permissions::from_mode(0o640))?;

        write_manifest(&manifest, "version '1.2.4'\n")?;

        assert_eq!(fs::metadata(&manifest)?.permissions().mode() & 0o777, 0o640);
        assert_eq!(fs::read_to_string(&manifest)?, "version '1.2.4'\n");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn write_manifest_updates_symlink_target() -> Result<()> {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir()?;
        let target = dir.path().join("target.lua");
        let link = dir.path().join("fxmanifest.lua");
        write_file(&target, "version '1.2.3'\n")?;
        symlink("target.lua", &link)?;

        write_manifest(&link, "version '1.2.4'\n")?;

        assert!(fs::symlink_metadata(&link)?.file_type().is_symlink());
        assert_eq!(fs::read_to_string(&target)?, "version '1.2.4'\n");
        Ok(())
    }
}
