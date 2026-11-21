use std::fmt::Write as _;
use std::path::Path;

use crate::manifest::Manifest;
use crate::pack::{EntryKind, Plan};

/// The result of a pack operation, used to render its status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome<'a> {
    /// The plan was validated and previewed without writing an archive.
    DryRun { error: Option<&'a str> },
    /// The archive was written successfully.
    Written,
    /// The operation failed while creating an archive.
    Failed { error: &'a str },
}

impl Outcome<'_> {
    #[must_use]
    fn status(self) -> String {
        match self {
            Self::DryRun { error: None } => "Success (dry run; archive not written)".into(),
            Self::DryRun { error: Some(error) } | Self::Failed { error } => {
                format!("Failed: {error}")
            }
            Self::Written => "Success (archive written)".into(),
        }
    }
}

/// Render a Markdown report for a completed or failed pack operation.
#[must_use]
pub fn render_markdown(
    manifest: &Manifest,
    plan: &Plan,
    output: &Path,
    outcome: Outcome<'_>,
) -> String {
    let mut report = String::new();

    let _ = writeln!(report, "# Pack Report");
    let _ = writeln!(report);
    let _ = writeln!(
        report,
        "- **Input directory:** `{}`",
        manifest.root.display()
    );
    let _ = writeln!(report, "- **Pattern file:** `{}`", manifest.path.display());
    let _ = writeln!(report, "- **Patterns:** {}", manifest.patterns.len());
    let _ = writeln!(report, "- **Output archive:** `{}`", output.display());
    let _ = writeln!(
        report,
        "- **Mode:** {}",
        if matches!(outcome, Outcome::DryRun { .. }) {
            "Dry run"
        } else {
            "Create archive"
        }
    );
    let _ = writeln!(report, "- **Status:** {}", outcome.status());
    let _ = writeln!(report);
    let _ = writeln!(report, "## Summary");
    let _ = writeln!(report);
    let _ = writeln!(report, "- **Files:** {}", plan.file_count());
    let _ = writeln!(report, "- **Directories:** {}", plan.directory_count());
    let _ = writeln!(
        report,
        "- **Total uncompressed size:** {} bytes",
        plan.total_size()
    );
    let _ = writeln!(report);
    let _ = writeln!(report, "## Included Files");
    let _ = writeln!(report);

    let files = plan
        .entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            EntryKind::File { size, .. } => Some((entry.path.as_str(), *size)),
            EntryKind::Directory => None,
        })
        .collect::<Vec<_>>();

    if files.is_empty() {
        let _ = writeln!(report, "_No files included._");
    } else {
        let _ = writeln!(report, "| File | Size (bytes) |");
        let _ = writeln!(report, "| --- | ---: |");
        for (path, size) in files {
            let _ = writeln!(report, "| {} | {size} |", code_cell(path));
        }
    }

    let _ = writeln!(report);
    let _ = writeln!(report, "## Problems");
    let _ = writeln!(report);

    if plan.problems.is_empty() {
        let _ = writeln!(report, "_None._");
    } else {
        let _ = writeln!(report, "| Entry | Problem |");
        let _ = writeln!(report, "| --- | --- |");
        for problem in &plan.problems {
            let _ = writeln!(
                report,
                "| {} | {} |",
                code_cell(&problem.entry),
                code_cell(&problem.reason)
            );
        }
    }

    report
}

/// Render the plain-text preview printed by `--dry-run` without `--report`.
#[must_use]
pub fn render_preview(manifest: &Manifest, plan: &Plan, output: &Path) -> String {
    let mut preview = String::new();

    let _ = writeln!(
        preview,
        "Dry run: {} file(s), {} directories, {} bytes from `{}` using `{}`",
        plan.file_count(),
        plan.directory_count(),
        plan.total_size(),
        manifest.root.display(),
        manifest.path.display()
    );
    let _ = writeln!(preview, "Archive `{}` was not written.", output.display());

    if plan.entries.is_empty() {
        let _ = writeln!(preview, "No entries would be included.");
    } else {
        for entry in &plan.entries {
            match &entry.kind {
                EntryKind::File { size, .. } => {
                    let _ = writeln!(preview, "{} ({size} bytes)", entry.path);
                }
                EntryKind::Directory => {
                    let _ = writeln!(preview, "{} (directory)", entry.path);
                }
            }
        }
    }

    if !plan.problems.is_empty() {
        let _ = writeln!(preview, "Problems:");
        for problem in &plan.problems {
            let _ = writeln!(preview, "{}: {}", problem.entry, problem.reason);
        }
    }

    preview
}

fn code_cell(value: &str) -> String {
    let value = value.replace('|', "\\|").replace(['\n', '\r'], " ");
    let fence = "`".repeat(longest_backtick_run(&value) + 1);

    if value.starts_with('`') || value.ends_with('`') {
        format!("{fence} {value} {fence}")
    } else {
        format!("{fence}{value}{fence}")
    }
}

fn longest_backtick_run(value: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for character in value.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::pack::{ArchiveEntry, Problem};

    fn sample_manifest() -> Manifest {
        Manifest {
            path: PathBuf::from("/tmp/.pack"),
            root: PathBuf::from("/tmp"),
            patterns: vec!["**/*".into()],
        }
    }

    fn sample_plan() -> Plan {
        Plan {
            entries: vec![
                ArchiveEntry {
                    path: "assets/".into(),
                    kind: EntryKind::Directory,
                },
                ArchiveEntry {
                    path: "assets/a.txt".into(),
                    kind: EntryKind::File {
                        source: PathBuf::from("/tmp/assets/a.txt"),
                        size: 5,
                    },
                },
            ],
            problems: Vec::new(),
        }
    }

    #[test]
    fn report_contains_manifest_archive_summary_and_files() {
        let report = render_markdown(
            &sample_manifest(),
            &sample_plan(),
            Path::new("/tmp/out.zip"),
            Outcome::DryRun { error: None },
        );

        assert!(report.contains("# Pack Report"));
        assert!(report.contains("- **Input directory:** `/tmp`"));
        assert!(report.contains("- **Pattern file:** `/tmp/.pack`"));
        assert!(report.contains("- **Patterns:** 1"));
        assert!(report.contains("- **Output archive:** `/tmp/out.zip`"));
        assert!(report.contains("- **Mode:** Dry run"));
        assert!(report.contains("Success (dry run; archive not written)"));
        assert!(report.contains("- **Files:** 1"));
        assert!(report.contains("- **Directories:** 1"));
        assert!(report.contains("- **Total uncompressed size:** 5 bytes"));
        assert!(report.contains("| `assets/a.txt` | 5 |"));
        assert!(report.contains("## Problems"));
        assert!(report.contains("_None._"));
    }

    #[test]
    fn report_uses_written_status() {
        let report = render_markdown(
            &sample_manifest(),
            &sample_plan(),
            Path::new("/tmp/out.zip"),
            Outcome::Written,
        );

        assert!(report.contains("- **Mode:** Create archive"));
        assert!(report.contains("Success (archive written)"));
    }

    #[test]
    fn report_keeps_dry_run_mode_when_failed() {
        let report = render_markdown(
            &sample_manifest(),
            &sample_plan(),
            Path::new("/tmp/out.zip"),
            Outcome::DryRun {
                error: Some("pack validation failed"),
            },
        );

        assert!(report.contains("- **Mode:** Dry run"));
        assert!(report.contains("- **Status:** Failed: pack validation failed"));
    }

    #[test]
    fn report_lists_problems_and_failed_status() {
        let plan = Plan {
            entries: Vec::new(),
            problems: vec![Problem {
                entry: "missing.txt".into(),
                reason: "not found".into(),
            }],
        };

        let report = render_markdown(
            &sample_manifest(),
            &plan,
            Path::new("/tmp/out.zip"),
            Outcome::Failed {
                error: "pack validation failed",
            },
        );

        assert!(report.contains("- **Status:** Failed: pack validation failed"));
        assert!(report.contains("| `missing.txt` | `not found` |"));
        assert!(report.contains("_No files included._"));
    }

    #[test]
    fn preview_lists_entries_and_states_archive_was_not_written() {
        let preview = render_preview(
            &sample_manifest(),
            &sample_plan(),
            Path::new("/tmp/out.zip"),
        );

        assert!(
            preview.contains(
                "Dry run: 1 file(s), 1 directories, 5 bytes from `/tmp` using `/tmp/.pack`"
            )
        );
        assert!(preview.contains("Archive `/tmp/out.zip` was not written."));
        assert!(preview.contains("assets/ (directory)"));
        assert!(preview.contains("assets/a.txt (5 bytes)"));
    }

    #[test]
    fn preview_lists_problems() {
        let plan = Plan {
            entries: Vec::new(),
            problems: vec![Problem {
                entry: "missing.txt".into(),
                reason: "not found".into(),
            }],
        };

        let preview = render_preview(&sample_manifest(), &plan, Path::new("/tmp/out.zip"));

        assert!(preview.contains("Problems:"));
        assert!(preview.contains("missing.txt: not found"));
    }

    #[test]
    fn code_cell_escapes_markdown_control_characters() {
        assert_eq!(code_cell("a|b"), "`a\\|b`");
        assert_eq!(code_cell("a`b"), "``a`b``");
        assert_eq!(code_cell("`a"), "`` `a ``");
        assert_eq!(code_cell("a\nb"), "`a b`");
    }
}
