use std::fs;
use std::process::Command;

use anyhow::Result;

mod common;

use common::{sha256_hex, write_file, zip_names};

fn trek() -> Command {
    Command::new(env!("CARGO_BIN_EXE_trek"))
}

#[test]
fn pack_includes_root_level_glob() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;
    write_file(&dir.path().join("nested/util.lua"), "util")?;
    write_file(&dir.path().join("notes.txt"), "notes")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(result.status.success());
    assert_eq!(zip_names(&output)?, vec!["main.lua"]);
    Ok(())
}

#[test]
fn pack_double_star_includes_nested_files() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "**/*.lua\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;
    write_file(&dir.path().join("nested/util.lua"), "util")?;
    write_file(&dir.path().join("nested/deep/app.lua"), "app")?;
    write_file(&dir.path().join("notes.txt"), "notes")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(result.status.success());
    assert_eq!(
        zip_names(&output)?,
        vec![
            "main.lua",
            "nested/",
            "nested/deep/",
            "nested/deep/app.lua",
            "nested/util.lua",
        ]
    );
    Ok(())
}

#[test]
fn pack_deduplicates_overlapping_patterns() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.lua\n**/*.lua\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;
    write_file(&dir.path().join("nested/util.lua"), "util")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(result.status.success());
    assert_eq!(
        zip_names(&output)?,
        vec!["main.lua", "nested/", "nested/util.lua"]
    );
    Ok(())
}

#[test]
fn pack_excludes_pattern_file_and_output_archive() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "**/*\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    for _ in 0..2 {
        let result = trek()
            .args(["pack", "--input"])
            .arg(dir.path())
            .args(["--output"])
            .arg(&output)
            .output()?;
        assert!(result.status.success());
    }

    assert_eq!(zip_names(&output)?, vec!["a.txt"]);
    Ok(())
}

#[test]
fn pack_can_explicitly_include_pattern_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), ".pack\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(result.status.success());
    assert_eq!(zip_names(&output)?, vec![".pack"]);
    Ok(())
}

#[test]
fn pack_dry_run_report_prints_markdown_without_writing() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .arg("--dry-run")
        .arg("--report")
        .output()?;

    assert!(result.status.success());
    assert!(!output.exists());
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("# Pack Report"));
    assert!(stdout.contains("- **Input directory:**"));
    assert!(stdout.contains("- **Pattern file:**"));
    assert!(stdout.contains("- **Mode:** Dry run"));
    assert!(stdout.contains("| `main.lua` | 4 |"));
    assert!(stdout.contains("Success (dry run; archive not written)"));
    assert!(!stdout.contains("SHA-256"));
    Ok(())
}

#[test]
fn pack_sha256_prints_checksum() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .arg("--sha256")
        .output()?;

    assert!(result.status.success());
    let expected = sha256_hex(&output)?;
    let stdout = String::from_utf8(result.stdout)?;
    assert_eq!(stdout.matches("SHA-256:").count(), 1);
    assert!(stdout.contains(&format!("SHA-256: {expected}")));
    Ok(())
}

#[test]
fn pack_sha256_report_includes_checksum() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .arg("--sha256")
        .arg("--report")
        .output()?;

    assert!(result.status.success());
    let expected = sha256_hex(&output)?;
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains(&format!("- **SHA-256:** `{expected}`")));
    Ok(())
}

#[test]
fn pack_sha256_dry_run_report_reports_unavailable() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .arg("--dry-run")
        .arg("--sha256")
        .arg("--report")
        .output()?;

    assert!(result.status.success());
    assert!(!output.exists());
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("- **SHA-256:** not available (dry run)"));
    Ok(())
}

#[test]
fn pack_sha256_dry_run_preview_reports_unavailable() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .arg("--dry-run")
        .arg("--sha256")
        .output()?;

    assert!(result.status.success());
    assert!(!output.exists());
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("SHA-256: not available (dry run)"));
    Ok(())
}

#[test]
fn pack_reports_invalid_pattern() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "[\n")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .arg("--report")
        .output()?;

    assert!(!result.status.success());
    assert!(!output.exists());
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("- **Status:** Failed:"));
    assert!(stdout.contains("| `[` |"));
    Ok(())
}

#[test]
fn pack_empty_pattern_file_fails() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "# only comments\n")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    assert!(!output.exists());
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("pattern file contains no include patterns"));
    Ok(())
}

#[test]
fn pack_missing_pattern_file_fails() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    assert!(!output.exists());
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("failed to read pattern file"));
    Ok(())
}

#[test]
fn pack_default_output_uses_input_folder_name() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let folder_name = dir
        .path()
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("temporary directory has no name"))?
        .to_string_lossy()
        .into_owned();
    let output = dir.path().join(format!("{folder_name}.zip"));

    let result = trek()
        .current_dir(dir.path())
        .args(["pack", "--input", "."])
        .output()?;

    assert!(result.status.success());
    assert_eq!(zip_names(&output)?, vec!["a.txt"]);
    Ok(())
}

#[test]
fn pack_rerun_excludes_previous_output() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "**/*\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    for _ in 0..2 {
        let result = trek()
            .args(["pack", "--input"])
            .arg(dir.path())
            .args(["--output"])
            .arg(&output)
            .output()?;
        assert!(result.status.success());
    }

    assert_eq!(zip_names(&output)?, vec!["a.txt"]);
    Ok(())
}

#[test]
fn pack_refuses_to_overwrite_non_zip_output() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    let output = dir.path().join("a.txt");
    write_file(&output, "IMPORTANT")?;
    write_file(&dir.path().join("b.txt"), "second")?;

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(&output)?, "IMPORTANT");
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("not a ZIP archive"));
    Ok(())
}

#[test]
fn pack_dry_run_refuses_to_overwrite_non_zip_output() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    let output = dir.path().join("a.txt");
    write_file(&output, "IMPORTANT")?;
    write_file(&dir.path().join("b.txt"), "second")?;

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .arg("--dry-run")
        .arg("--report")
        .output()?;

    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(&output)?, "IMPORTANT");
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("- **Status:** Failed:"));
    assert!(stdout.contains("not a ZIP archive"));
    Ok(())
}

#[test]
fn pack_reports_no_matches() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    assert!(!output.exists());
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("no files matched the include patterns"));
    Ok(())
}

#[test]
fn pack_normalizes_leading_dot_slash_pattern() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "./*.lua\n")?;
    write_file(&dir.path().join("a.lua"), "a")?;
    let output = dir.path().join("out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(result.status.success());
    assert_eq!(zip_names(&output)?, vec!["a.lua"]);
    Ok(())
}

#[cfg(unix)]
#[test]
fn pack_excludes_symlink_output() -> Result<()> {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "**/*\n")?;
    write_file(&dir.path().join("target.bin"), "target")?;
    let output = dir.path().join("out.zip");
    symlink("target.bin", &output)?;

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(result.status.success());
    assert_eq!(zip_names(&output)?, vec!["target.bin"]);
    Ok(())
}

#[test]
fn pack_rejects_output_equal_to_pattern_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let pattern_file = dir.path().join(".pack");
    write_file(&pattern_file, "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&pattern_file)
        .output()?;

    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(&pattern_file)?, "*.txt\n");
    Ok(())
}

#[test]
fn pack_rejects_output_that_is_a_directory() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("output-directory");
    fs::create_dir(&output)?;

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("is a directory"));
    Ok(())
}

#[test]
fn pack_rejects_missing_output_parent() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.txt\n")?;
    write_file(&dir.path().join("a.txt"), "hello")?;
    let output = dir.path().join("missing/out.zip");

    let result = trek()
        .args(["pack", "--input"])
        .arg(dir.path())
        .args(["--output"])
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    assert!(!output.exists());
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("does not exist"));
    Ok(())
}
