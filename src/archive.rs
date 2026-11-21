use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::pack::{EntryKind, Plan};

/// Write a ZIP archive from a validated plan.
///
/// The archive is written to a uniquely named sibling temporary file and
/// renamed into place only after it is complete, so a failed write never
/// clobbers an existing archive. Entries are written in plan order with fixed
/// metadata, making repeated runs with the same input byte-identical.
///
/// # Errors
///
/// Returns an error if the plan has problems, the output or its temporary
/// sibling collides with an input, the output directory is missing, or a
/// source file cannot be read or written.
pub fn write_zip(plan: &Plan, output: &Path) -> Result<()> {
    if plan.has_problems() {
        bail!(
            "cannot create archive: pack validation failed with {} problem(s)",
            plan.problems.len()
        );
    }

    validate_output(plan, output)?;

    let (temporary, file) = create_temporary_file(output)?;
    let temporary_path = match fs::canonicalize(&temporary) {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| {
                format!(
                    "failed to resolve temporary archive for {}",
                    output.display()
                )
            });
        }
    };
    if let Err(error) = ensure_not_source(plan, &temporary_path, "temporary archive", &temporary) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    if let Err(error) = write_archive_file(plan, file, output) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temporary, output)
        .with_context(|| format!("failed to replace archive {}", output.display()))
    {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    Ok(())
}

/// Validate that an output archive does not collide with the files in the plan.
///
/// # Errors
///
/// Returns an error if the output is a directory, its parent directory is
/// missing or not a directory, or the output path is a planned source file.
pub(crate) fn validate_output(plan: &Plan, output: &Path) -> Result<()> {
    let output_path = output_identity(output)?;
    if output_path.is_dir() {
        bail!("output archive {} is a directory", output.display());
    }

    ensure_not_source(plan, &output_path, "output archive", output)
}

/// Use the canonical path for non-symlink outputs and the lexical path for
/// symlinks, which the final rename replaces rather than follows.
fn output_identity(output: &Path) -> Result<PathBuf> {
    match fs::symlink_metadata(output) {
        Ok(metadata) if !metadata.file_type().is_symlink() => fs::canonicalize(output)
            .with_context(|| format!("failed to resolve output archive {}", output.display())),
        _ => absolute_output_path(output),
    }
}

fn absolute_output_path(output: &Path) -> Result<PathBuf> {
    let parent = parent_directory(output);
    let parent = fs::canonicalize(parent).with_context(|| {
        format!(
            "output directory {} for {} does not exist",
            parent.display(),
            output.display()
        )
    })?;
    if !parent.is_dir() {
        bail!(
            "output directory {} for {} is not a directory",
            parent.display(),
            output.display()
        );
    }

    let file_name = output
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("output archive {} has no file name", output.display()))?;
    Ok(parent.join(file_name))
}

#[must_use]
fn parent_directory(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn ensure_not_source(plan: &Plan, candidate: &Path, role: &str, display: &Path) -> Result<()> {
    for entry in &plan.entries {
        if let EntryKind::File { source, .. } = &entry.kind {
            let source_path = fs::canonicalize(source)
                .with_context(|| format!("failed to resolve {}", source.display()))?;
            if source_path == candidate {
                bail!(
                    "{role} {} is also packed as `{}`",
                    display.display(),
                    entry.path
                );
            }
        }
    }

    Ok(())
}

const MAX_TEMP_ATTEMPTS: u32 = 100;

fn create_temporary_file(output: &Path) -> Result<(PathBuf, File)> {
    for attempt in 0..MAX_TEMP_ATTEMPTS {
        let temporary = temporary_path(output, attempt);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to create archive {}", output.display()));
            }
        }
    }

    bail!(
        "failed to create a unique temporary archive for {} after {MAX_TEMP_ATTEMPTS} attempts",
        output.display()
    )
}

fn write_archive_file(plan: &Plan, file: File, output: &Path) -> Result<()> {
    let mut writer = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default());

    for entry in &plan.entries {
        match &entry.kind {
            EntryKind::Directory => {
                writer
                    .add_directory(&entry.path, options)
                    .with_context(|| format!("failed to add directory {}", entry.path))?;
            }
            EntryKind::File { source, .. } => {
                writer
                    .start_file(&entry.path, options)
                    .with_context(|| format!("failed to add file {}", entry.path))?;
                let input = File::open(source)
                    .with_context(|| format!("failed to open source file {}", source.display()))?;
                let mut input = BufReader::new(input);
                std::io::copy(&mut input, &mut writer)
                    .with_context(|| format!("failed to write file {}", entry.path))?;
            }
        }
    }

    let mut buffered = writer
        .finish()
        .with_context(|| format!("failed to finish archive {}", output.display()))?;
    buffered
        .flush()
        .with_context(|| format!("failed to flush archive {}", output.display()))?;
    Ok(())
}

#[must_use]
fn temporary_path(output: &Path, attempt: u32) -> PathBuf {
    let mut name = output
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_default();
    name.push(format!(".{}.{attempt}.partial", std::process::id()));
    output.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Read;

    use zip::ZipArchive;

    use super::*;
    use crate::pack::{ArchiveEntry, Problem};
    use crate::test_support::{plan_for, write_file};

    #[test]
    fn creates_archive_with_preserved_directory_structure() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        write_file(&dir.path().join("nested/b.txt"), "world")?;
        let output = dir.path().join("out.zip");
        let plan = plan_for(dir.path(), &["a.txt", "nested/*.txt"], &output);

        write_zip(&plan, &output)?;

        let mut archive = ZipArchive::new(File::open(&output)?)?;
        let mut names = Vec::new();
        for index in 0..archive.len() {
            let file = archive.by_index(index)?;
            names.push(file.name()?.to_string());
        }
        assert_eq!(names, vec!["a.txt", "nested/", "nested/b.txt"]);

        {
            let file = archive.by_name("a.txt")?;
            assert_eq!(file.last_modified(), Some(DateTime::default()));
        }

        let mut contents = String::new();
        archive
            .by_name("nested/b.txt")?
            .read_to_string(&mut contents)?;
        assert_eq!(contents, "world");
        Ok(())
    }

    #[test]
    fn archive_bytes_are_deterministic() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        write_file(&dir.path().join("nested/b.txt"), "world")?;
        let first = dir.path().join("first.zip");
        let plan = plan_for(dir.path(), &["**/*.txt"], &first);
        let second = dir.path().join("second.zip");

        write_zip(&plan, &first)?;
        write_zip(&plan, &second)?;

        assert_eq!(fs::read(&first)?, fs::read(&second)?);
        Ok(())
    }

    #[test]
    fn failed_write_preserves_existing_archive() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let output = dir.path().join("out.zip");
        fs::write(&output, b"old archive")?;
        let source = dir.path().join("source-directory");
        fs::create_dir(&source)?;
        let plan = Plan {
            entries: vec![ArchiveEntry {
                path: "source".into(),
                kind: EntryKind::File { source, size: 0 },
            }],
            problems: Vec::new(),
        };

        assert!(write_zip(&plan, &output).is_err());
        assert_eq!(fs::read(&output)?, b"old archive".as_slice());
        assert_eq!(
            fs::read_dir(dir.path())?
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().ends_with(".partial"))
                .count(),
            0
        );
        Ok(())
    }

    #[test]
    fn stale_dot_partial_file_is_packed_without_corruption() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        write_file(&dir.path().join("out.zip.partial"), "stale")?;
        let output = dir.path().join("out.zip");
        let plan = plan_for(dir.path(), &["**/*"], &output);

        write_zip(&plan, &output)?;

        let mut archive = ZipArchive::new(File::open(&output)?)?;
        let mut stale = String::new();
        archive
            .by_name("out.zip.partial")?
            .read_to_string(&mut stale)?;
        assert_eq!(stale, "stale");
        Ok(())
    }

    #[test]
    fn rejects_output_that_is_a_directory() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = dir.path().join("output-directory");
        let plan = plan_for(dir.path(), &["a.txt"], &output);
        fs::create_dir(&output)?;

        let error = match write_zip(&plan, &output) {
            Ok(()) => anyhow::bail!("expected an error"),
            Err(error) => error,
        };

        assert!(format!("{error:#}").contains("is a directory"));
        Ok(())
    }

    #[test]
    fn rejects_output_with_missing_parent_directory() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = dir.path().join("missing/out.zip");
        let plan = plan_for(dir.path(), &["a.txt"], &output);

        let error = match write_zip(&plan, &output) {
            Ok(()) => anyhow::bail!("expected an error"),
            Err(error) => error,
        };
        let message = format!("{error:#}");

        assert!(message.contains("does not exist"));
        assert!(message.contains("out.zip"));
        assert!(!output.exists());
        Ok(())
    }

    #[test]
    fn temporary_paths_include_pid_and_attempt() {
        let output = Path::new("/tmp/out.zip");
        let first = temporary_path(output, 0).to_string_lossy().into_owned();
        let second = temporary_path(output, 1).to_string_lossy().into_owned();
        let pid = std::process::id().to_string();

        assert_ne!(first, second);
        assert!(first.contains(&pid));
        assert!(first.ends_with(".0.partial"));
    }

    #[test]
    fn planned_source_at_in_use_temp_name_is_skipped() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("a.txt"), "hello")?;
        let output = dir.path().join("out.zip");
        let planned_temp = temporary_path(&output, 0);
        write_file(&planned_temp, "stale")?;
        let plan = plan_for(dir.path(), &["**/*"], &output);
        let planned_name = planned_temp
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("temporary path has no file name"))?
            .to_string_lossy()
            .into_owned();

        write_zip(&plan, &output)?;

        let mut archive = ZipArchive::new(File::open(&output)?)?;
        let mut stale = String::new();
        archive.by_name(&planned_name)?.read_to_string(&mut stale)?;
        assert_eq!(stale, "stale");
        assert!(!temporary_path(&output, 1).exists());
        Ok(())
    }

    #[test]
    fn refuses_to_write_when_plan_has_problems() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let output = dir.path().join("out.zip");
        let plan = Plan {
            entries: vec![ArchiveEntry {
                path: "a.txt".into(),
                kind: EntryKind::Directory,
            }],
            problems: vec![Problem {
                entry: "missing.txt".into(),
                reason: "not found".into(),
            }],
        };

        assert!(write_zip(&plan, &output).is_err());
        assert!(!output.exists());
        Ok(())
    }

    #[test]
    fn planned_source_collision_is_detected() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("a.txt");
        write_file(&source, "hello")?;
        let plan = Plan {
            entries: vec![ArchiveEntry {
                path: "a.txt".into(),
                kind: EntryKind::File {
                    source: source.clone(),
                    size: 5,
                },
            }],
            problems: Vec::new(),
        };

        let error = match validate_output(&plan, &source) {
            Ok(()) => anyhow::bail!("expected an error"),
            Err(error) => error,
        };

        assert!(format!("{error:#}").contains("also packed as `a.txt`"));
        Ok(())
    }
}
