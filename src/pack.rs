use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use glob::{MatchOptions, Pattern};

use crate::archive::{sha256_file, validate_output, write_zip};
use crate::cli::PackArgs;
use crate::manifest::{Manifest, validate_pattern};
use crate::report::{Checksum, Outcome, render_markdown, render_preview};

/// The kind of item represented by an archive entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    File { source: PathBuf, size: u64 },
    Directory,
}

impl EntryKind {
    #[must_use]
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File { .. })
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        match self {
            Self::File { size, .. } => *size,
            Self::Directory => 0,
        }
    }
}

/// A single, validated archive entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// `/`-separated path inside the ZIP archive.
    pub path: String,
    pub kind: EntryKind,
}

/// An include pattern or discovered path that could not be processed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The original pattern line, or an archive-relative path for traversal problems.
    pub entry: String,
    pub reason: String,
}

/// The validated set of entries shared by archive creation and reporting.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Plan {
    pub entries: Vec<ArchiveEntry>,
    pub problems: Vec<Problem>,
}

impl Plan {
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.kind.is_file())
            .count()
    }

    #[must_use]
    pub fn directory_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.kind, EntryKind::Directory))
            .count()
    }

    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.entries.iter().map(|entry| entry.kind.size()).sum()
    }

    #[must_use]
    pub fn has_problems(&self) -> bool {
        !self.problems.is_empty()
    }
}

/// Output paths to exclude during discovery.
///
/// The lexical path identifies the output path itself, including symlinks. The
/// canonical paths catch aliases; symlink outputs skip canonical identity for
/// regular files because the final rename replaces the link, not its target.
#[derive(Debug, Clone, Default)]
struct ExcludedOutput {
    lexical: Option<PathBuf>,
    canonical: Option<PathBuf>,
    regular_canonical: Option<PathBuf>,
}

impl ExcludedOutput {
    fn new(output: &Path) -> Self {
        let metadata = fs::symlink_metadata(output).ok();
        let is_symlink = metadata
            .as_ref()
            .is_some_and(|metadata| metadata.file_type().is_symlink());
        let canonical = fs::canonicalize(output).ok();

        Self {
            lexical: lexical_absolute(output),
            regular_canonical: if is_symlink { None } else { canonical.clone() },
            canonical,
        }
    }

    fn is_output(&self, source: &Path, is_regular: bool) -> bool {
        if self
            .lexical
            .as_deref()
            .is_some_and(|output| source == output)
        {
            return true;
        }

        let identity = if is_regular {
            &self.regular_canonical
        } else {
            &self.canonical
        };
        identity
            .as_deref()
            .is_some_and(|output| fs::canonicalize(source).ok().as_deref() == Some(output))
    }
}

fn lexical_absolute(path: &Path) -> Option<PathBuf> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = fs::canonicalize(parent).ok()?;
    let file_name = path.file_name()?;
    Some(parent.join(file_name))
}

/// Discover files matching the manifest's include patterns.
///
/// Paths are matched relative to the input directory. Entries are deduplicated,
/// sorted by archive path, and parent directories are added explicitly. The
/// output archive is never included, even when a pattern matches it. Symbolic
/// links and special files are only reported when a pattern matches them.
#[must_use]
pub fn build_plan(manifest: &Manifest, output: &Path) -> Plan {
    let mut entries = BTreeMap::new();
    let mut problems = Vec::new();

    let root = match fs::canonicalize(&manifest.root) {
        Ok(root) => root,
        Err(error) => {
            problems.push(Problem {
                entry: manifest.root.display().to_string(),
                reason: describe_io_error(&error),
            });
            return Plan {
                entries: Vec::new(),
                problems,
            };
        }
    };

    let mut patterns = Vec::new();
    for raw in &manifest.patterns {
        let normalized = raw.strip_prefix("./").unwrap_or(raw);
        if normalized.is_empty() {
            problems.push(Problem {
                entry: raw.clone(),
                reason: "pattern is empty".into(),
            });
            continue;
        }

        if let Err(error) = validate_pattern(normalized) {
            problems.push(Problem {
                entry: raw.clone(),
                reason: error.to_string(),
            });
            continue;
        }

        match Pattern::new(normalized) {
            Ok(pattern) => patterns.push(pattern),
            Err(error) => problems.push(Problem {
                entry: raw.clone(),
                reason: error.to_string(),
            }),
        }
    }

    if manifest.patterns.is_empty() {
        problems.push(Problem {
            entry: manifest.path.display().to_string(),
            reason: "pattern file contains no include patterns".into(),
        });
    }

    if !patterns.is_empty() {
        let output = ExcludedOutput::new(output);
        collect_matches(&root, "", &patterns, &output, &mut entries, &mut problems);
    }

    if entries.is_empty() && problems.is_empty() {
        problems.push(Problem {
            entry: manifest.path.display().to_string(),
            reason: "no files matched the include patterns".into(),
        });
    }

    problems.sort_by(|left, right| {
        left.entry
            .cmp(&right.entry)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    problems.dedup();

    Plan {
        entries: entries.into_values().collect(),
        problems,
    }
}

const MATCH_OPTIONS: MatchOptions = MatchOptions {
    case_sensitive: true,
    require_literal_separator: true,
    require_literal_leading_dot: true,
};

fn collect_matches(
    directory: &Path,
    prefix: &str,
    patterns: &[Pattern],
    output: &ExcludedOutput,
    entries: &mut BTreeMap<String, ArchiveEntry>,
    problems: &mut Vec<Problem>,
) {
    let read_dir = match fs::read_dir(directory) {
        Ok(read_dir) => read_dir,
        Err(error) => {
            problems.push(Problem {
                entry: display_archive_path(prefix),
                reason: describe_io_error(&error),
            });
            return;
        }
    };

    for child in read_dir {
        let child = match child {
            Ok(child) => child,
            Err(error) => {
                problems.push(Problem {
                    entry: display_archive_path(prefix),
                    reason: format!("failed to read directory entry: {error}"),
                });
                continue;
            }
        };

        let name_os = child.file_name();
        let Some(name) = name_os.to_str() else {
            problems.push(Problem {
                entry: join_relative(prefix, &name_os.to_string_lossy()),
                reason: "file name is not valid UTF-8".into(),
            });
            continue;
        };
        let relative = join_relative(prefix, name);
        let source = child.path();
        let metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) => {
                problems.push(Problem {
                    entry: relative,
                    reason: describe_io_error(&error),
                });
                continue;
            }
        };

        if output.is_output(&source, metadata.is_file()) {
            continue;
        }

        if metadata.is_dir() {
            collect_matches(&source, &relative, patterns, output, entries, problems);
            continue;
        }

        if !patterns
            .iter()
            .any(|pattern| pattern.matches_with(&relative, MATCH_OPTIONS))
        {
            continue;
        }

        if metadata.is_file() {
            add_parent_directories(&relative, entries);
            entries.entry(relative.clone()).or_insert(ArchiveEntry {
                path: relative,
                kind: EntryKind::File {
                    source,
                    size: metadata.len(),
                },
            });
        } else {
            problems.push(Problem {
                entry: relative,
                reason: "not a regular file".into(),
            });
        }
    }
}

fn join_relative(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

fn add_parent_directories(relative: &str, entries: &mut BTreeMap<String, ArchiveEntry>) {
    let parts = relative.split('/').collect::<Vec<_>>();
    for index in 1..parts.len() {
        let directory = format!("{}/", parts[..index].join("/"));
        entries.entry(directory.clone()).or_insert(ArchiveEntry {
            path: directory,
            kind: EntryKind::Directory,
        });
    }
}

fn display_archive_path(archive_prefix: &str) -> String {
    if archive_prefix.is_empty() {
        ".".into()
    } else {
        archive_prefix.to_string()
    }
}

fn describe_io_error(error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        "not found".into()
    } else {
        error.to_string()
    }
}

/// Derive the default output archive name from the input directory name.
#[must_use]
pub(crate) fn default_output(input: &Path) -> PathBuf {
    let mut name = input
        .file_name()
        .map(ToOwned::to_owned)
        .or_else(|| {
            fs::canonicalize(input)
                .ok()
                .and_then(|path| path.file_name().map(ToOwned::to_owned))
        })
        .unwrap_or_else(|| OsString::from("archive"));
    name.push(".zip");
    PathBuf::from(name)
}

/// Execute the `pack` subcommand.
///
/// # Errors
///
/// Returns an error if the pattern file cannot be read, validation fails, the
/// output collides with the pattern file, the archive cannot be written or
/// hashed, or output cannot be printed.
pub fn run(args: &PackArgs) -> Result<()> {
    let pattern_file = args.input.join(".pack");
    let manifest = Manifest::read(&pattern_file)?;
    let output = match &args.output {
        Some(output) => output.clone(),
        None => default_output(&args.input),
    };
    let plan = build_plan(&manifest, &output);

    let mut failure = None;
    let mut checksum = None;

    if plan.has_problems() {
        failure = Some(validation_error(&plan));
    } else if let Err(error) = validate_pattern_file_collision(&manifest, &output) {
        failure = Some(error);
    } else if args.dry_run {
        if let Err(error) = validate_output(&plan, &output) {
            failure = Some(error);
        }
    } else if let Err(error) = write_zip(&plan, &output) {
        failure = Some(error);
    } else if args.sha256 {
        match sha256_file(&output) {
            Ok(hash) => checksum = Some(hash),
            Err(error) => failure = Some(error),
        }
    }

    let failure_message = failure.as_ref().map(|error| format!("{error:#}"));
    let outcome = if args.dry_run {
        Outcome::DryRun {
            error: failure_message.as_deref(),
        }
    } else {
        match failure_message.as_deref() {
            Some(message) => Outcome::Failed { error: message },
            None => Outcome::Written,
        }
    };
    let checksum_display = if args.sha256 {
        Some(match checksum.as_deref() {
            Some(checksum) => Checksum::Value(checksum),
            None => Checksum::Unavailable,
        })
    } else {
        None
    };

    if args.report {
        write_stdout(&render_markdown(
            &manifest,
            &plan,
            &output,
            outcome,
            checksum_display,
        ))?;
    } else if args.dry_run {
        write_stdout(&render_preview(&manifest, &plan, &output, checksum_display))?;
    } else if failure.is_none() {
        let summary = format!(
            "Created {} ({} files, {} directories, {} bytes)\n",
            output.display(),
            plan.file_count(),
            plan.directory_count(),
            plan.total_size()
        );
        write_stdout(&summary)?;
        if let Some(checksum) = &checksum {
            write_stdout(&format!("SHA-256: {checksum}\n"))?;
        }
    }

    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn validation_error(plan: &Plan) -> anyhow::Error {
    let details = plan
        .problems
        .iter()
        .map(|problem| format!("{}: {}", problem.entry, problem.reason))
        .collect::<Vec<_>>()
        .join("; ");
    anyhow!(
        "pack validation failed with {} problem(s): {details}",
        plan.problems.len()
    )
}

fn validate_pattern_file_collision(manifest: &Manifest, output: &Path) -> Result<()> {
    if !output.exists() {
        return Ok(());
    }

    let output_path = fs::canonicalize(output)
        .with_context(|| format!("failed to resolve output archive {}", output.display()))?;
    let pattern_path = fs::canonicalize(&manifest.path)
        .with_context(|| format!("failed to resolve pattern file {}", manifest.path.display()))?;

    if output_path == pattern_path {
        bail!(
            "output archive {} is also the pattern file",
            output.display()
        );
    }

    Ok(())
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
    use crate::test_support::{manifest_for, plan_for, write_file};

    fn output_for(root: &Path) -> PathBuf {
        root.join("out.zip")
    }

    #[test]
    fn includes_root_level_glob_with_size() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        write_file(&dir.path().join("b.lua"), "lua")?;
        write_file(&dir.path().join("nested/c.lua"), "nested")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["*.lua"], &output);

        assert_eq!(plan.problems, Vec::new());
        assert_eq!(plan.file_count(), 1);
        assert_eq!(plan.directory_count(), 0);
        assert_eq!(plan.total_size(), 3);
        assert_eq!(
            plan.entries[0],
            ArchiveEntry {
                path: "b.lua".into(),
                kind: EntryKind::File {
                    source: dir.path().join("b.lua"),
                    size: 3,
                },
            }
        );
        Ok(())
    }

    #[test]
    fn double_star_includes_root_and_nested_files() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.lua"), "a")?;
        write_file(&dir.path().join("nested/b.lua"), "bb")?;
        write_file(&dir.path().join("nested/deep/c.lua"), "ccc")?;
        write_file(&dir.path().join("notes.txt"), "ignore")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["**/*.lua"], &output);

        assert_eq!(plan.problems, Vec::new());
        assert_eq!(
            plan.entries
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "a.lua",
                "nested/",
                "nested/b.lua",
                "nested/deep/",
                "nested/deep/c.lua",
            ]
        );
        assert_eq!(plan.file_count(), 3);
        assert_eq!(plan.directory_count(), 2);
        assert_eq!(plan.total_size(), 6);
        Ok(())
    }

    #[test]
    fn deduplicates_overlapping_patterns() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.lua"), "a")?;
        write_file(&dir.path().join("nested/b.lua"), "b")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["*.lua", "**/*.lua"], &output);

        assert_eq!(plan.problems, Vec::new());
        assert_eq!(
            plan.entries
                .iter()
                .filter(|entry| entry.kind.is_file())
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.lua", "nested/b.lua"]
        );
        Ok(())
    }

    #[test]
    fn wildcards_do_not_match_hidden_files() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join(".hidden.lua"), "hidden")?;
        write_file(&dir.path().join("visible.lua"), "visible")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["**/*.lua"], &output);

        assert_eq!(
            plan.entries
                .iter()
                .filter(|entry| entry.kind.is_file())
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            vec!["visible.lua"]
        );
        Ok(())
    }

    #[test]
    fn explicit_pattern_can_include_pattern_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join(".pack"), "*\n")?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &[".pack"], &output);

        assert_eq!(plan.problems, Vec::new());
        assert_eq!(plan.file_count(), 1);
        assert_eq!(plan.entries[0].path, ".pack");
        Ok(())
    }

    #[test]
    fn output_archive_is_never_included() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = output_for(dir.path());
        write_file(&output, "old archive")?;

        let plan = plan_for(dir.path(), &["**/*"], &output);

        assert_eq!(
            plan.entries
                .iter()
                .filter(|entry| entry.kind.is_file())
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.txt"]
        );
        Ok(())
    }

    #[test]
    fn unmatched_files_are_ignored() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        write_file(&dir.path().join("b.log"), "log")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["*.txt"], &output);

        assert_eq!(plan.problems, Vec::new());
        assert_eq!(plan.file_count(), 1);
        assert_eq!(plan.entries[0].path, "a.txt");
        Ok(())
    }

    #[test]
    fn reports_when_patterns_match_nothing() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["*.lua"], &output);

        assert_eq!(plan.entries, Vec::new());
        assert_eq!(plan.problems.len(), 1);
        assert_eq!(
            plan.problems[0].reason,
            "no files matched the include patterns"
        );
        Ok(())
    }

    #[test]
    fn normalizes_leading_dot_slash_patterns() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.lua"), "a")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["./*.lua"], &output);

        assert_eq!(plan.problems, Vec::new());
        assert_eq!(plan.file_count(), 1);
        assert_eq!(plan.entries[0].path, "a.lua");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn reports_non_utf8_file_names() -> Result<()> {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let dir = tempfile::tempdir()?;
        let name = OsString::from_vec(vec![0xff, b'.', b'l', b'u', b'a']);
        let expected_entry = name.to_string_lossy().into_owned();
        fs::write(dir.path().join(name), "x")?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["**/*.lua"], &output);

        assert_eq!(plan.entries, Vec::new());
        assert_eq!(plan.problems.len(), 1);
        assert_eq!(plan.problems[0].entry, expected_entry);
        assert_eq!(plan.problems[0].reason, "file name is not valid UTF-8");
        Ok(())
    }

    #[test]
    fn reports_invalid_patterns_in_sorted_order() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &["../secret", "/absolute", "["], &output);

        assert_eq!(plan.entries, Vec::new());
        assert_eq!(plan.problems.len(), 3);
        assert_eq!(plan.problems[0].entry, "../secret");
        assert_eq!(
            plan.problems[0].reason,
            "pattern escapes the input directory"
        );
        assert_eq!(plan.problems[1].entry, "/absolute");
        assert_eq!(
            plan.problems[1].reason,
            "absolute patterns are not supported"
        );
        assert_eq!(plan.problems[2].entry, "[");
        Ok(())
    }

    #[test]
    fn reports_empty_pattern_file() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let output = output_for(dir.path());

        let plan = plan_for(dir.path(), &[], &output);

        assert_eq!(plan.entries, Vec::new());
        assert_eq!(plan.problems.len(), 1);
        assert_eq!(
            plan.problems[0].reason,
            "pattern file contains no include patterns"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn reports_matched_symlinks_but_ignores_unmatched_ones() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("target.txt"), "hello")?;
        std::os::unix::fs::symlink("target.txt", dir.path().join("link.txt"))?;
        let output = output_for(dir.path());

        let matched = plan_for(dir.path(), &["link.txt"], &output);
        assert_eq!(matched.entries, Vec::new());
        assert_eq!(matched.problems.len(), 1);
        assert_eq!(matched.problems[0].reason, "not a regular file");

        let unmatched = plan_for(dir.path(), &["target.txt"], &output);
        assert_eq!(unmatched.problems, Vec::new());
        assert_eq!(unmatched.file_count(), 1);
        Ok(())
    }

    #[test]
    fn default_output_uses_directory_name() {
        assert_eq!(
            default_output(Path::new("/tmp/project")),
            PathBuf::from("project.zip")
        );
    }

    #[test]
    fn default_output_resolves_dot_to_directory_name() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let name = dir
            .path()
            .file_name()
            .ok_or_else(|| anyhow!("temporary directory has no name"))?
            .to_string_lossy()
            .into_owned();

        assert_eq!(
            default_output(&dir.path().join(".")),
            PathBuf::from(format!("{name}.zip"))
        );
        Ok(())
    }

    #[test]
    fn dry_run_does_not_create_archive() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join(".pack"), "*.txt\n")?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = output_for(dir.path());
        let args = PackArgs {
            input: dir.path().to_path_buf(),
            output: Some(output.clone()),
            dry_run: true,
            report: false,
            sha256: false,
        };

        run(&args)?;

        assert!(!output.exists());
        Ok(())
    }

    #[test]
    fn invalid_patterns_fail_without_creating_archive() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join(".pack"), "[\n")?;
        let output = output_for(dir.path());
        let args = PackArgs {
            input: dir.path().to_path_buf(),
            output: Some(output.clone()),
            dry_run: false,
            report: false,
            sha256: false,
        };

        assert!(run(&args).is_err());
        assert!(!output.exists());
        Ok(())
    }

    #[test]
    fn output_equal_to_pattern_file_is_rejected() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join(".pack"), "*.txt\n")?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let pattern_file = dir.path().join(".pack");
        let args = PackArgs {
            input: dir.path().to_path_buf(),
            output: Some(pattern_file.clone()),
            dry_run: false,
            report: false,
            sha256: false,
        };

        assert!(run(&args).is_err());
        assert_eq!(fs::read_to_string(&pattern_file)?, "*.txt\n");
        Ok(())
    }

    #[test]
    fn missing_pattern_file_fails() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let output = output_for(dir.path());
        let args = PackArgs {
            input: dir.path().to_path_buf(),
            output: Some(output.clone()),
            dry_run: false,
            report: false,
            sha256: false,
        };

        assert!(run(&args).is_err());
        assert!(!output.exists());
        Ok(())
    }

    #[test]
    fn output_inside_input_is_excluded_on_rerun() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join(".pack"), "**/*\n")?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = output_for(dir.path());
        let args = PackArgs {
            input: dir.path().to_path_buf(),
            output: Some(output.clone()),
            dry_run: false,
            report: false,
            sha256: false,
        };

        run(&args)?;
        let first_size = fs::metadata(&output)?.len();
        run(&args)?;

        assert_eq!(fs::metadata(&output)?.len(), first_size);
        Ok(())
    }

    #[test]
    fn manifest_helper_keeps_expected_root() {
        let manifest = manifest_for(Path::new("/tmp/project"), &["*.lua"]);

        assert_eq!(manifest.root, Path::new("/tmp/project"));
        assert_eq!(manifest.path, Path::new("/tmp/project/.pack"));
        assert_eq!(manifest.patterns, vec!["*.lua"]);
    }
}
