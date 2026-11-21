use std::fs;
use std::process::Command;

use anyhow::Result;

mod common;

use common::{sha256_hex, write_file, zip_file_contents, zip_names};

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

#[test]
fn version_prints_current_without_modifying_manifest() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "version '1.2.3'\n")?;

    let result = trek().args(["version", "-m"]).arg(&manifest).output()?;

    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout)?,
        "Current version: 1.2.3\n"
    );
    assert_eq!(fs::read_to_string(&manifest)?, "version '1.2.3'\n");
    Ok(())
}

#[test]
fn version_ci_prints_raw_current_version() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "version '1.2.3'\n")?;

    let result = trek()
        .args(["version", "--ci", "-m"])
        .arg(&manifest)
        .output()?;

    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout)?, "1.2.3\n");
    Ok(())
}

#[test]
fn version_bumps_patch_and_preserves_syntax() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "version = \"1.2.3\"\n")?;

    let result = trek()
        .args(["version", "--patch", "-m"])
        .arg(&manifest)
        .output()?;

    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout)?,
        "Bumped version: 1.2.3 -> 1.2.4\n"
    );
    assert_eq!(fs::read_to_string(&manifest)?, "version = \"1.2.4\"\n");
    Ok(())
}

#[test]
fn version_ci_bumps_minor_and_preserves_syntax() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "version(\"1.2.3\")\n")?;

    let result = trek()
        .args(["version", "--minor", "--ci", "-m"])
        .arg(&manifest)
        .output()?;

    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout)?, "1.3.0\n");
    assert_eq!(fs::read_to_string(&manifest)?, "version(\"1.3.0\")\n");
    Ok(())
}

#[test]
fn version_bumps_major() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "version '1.2.3'\n")?;

    let result = trek()
        .args(["version", "--major", "-m"])
        .arg(&manifest)
        .output()?;

    assert!(result.status.success());
    assert_eq!(fs::read_to_string(&manifest)?, "version '2.0.0'\n");
    Ok(())
}

#[test]
fn version_uses_default_manifest_path() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.2.3'\n")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["version", "--ci"])
        .output()?;

    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout)?, "1.2.3\n");
    Ok(())
}

#[test]
fn version_rejects_multiple_bump_flags() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "version '1.2.3'\n")?;

    let result = trek()
        .args(["version", "--patch", "--minor", "-m"])
        .arg(&manifest)
        .output()?;

    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(&manifest)?, "version '1.2.3'\n");
    Ok(())
}

#[test]
fn version_missing_manifest_fails() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("missing.lua");

    let result = trek().args(["version", "-m"]).arg(&manifest).output()?;

    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("failed to read manifest"));
    Ok(())
}

#[test]
fn version_invalid_version_fails() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "version 'not-a-version'\n")?;

    let result = trek().args(["version", "-m"]).arg(&manifest).output()?;

    assert!(!result.status.success());
    assert_eq!(fs::read_to_string(&manifest)?, "version 'not-a-version'\n");
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("invalid version"));
    Ok(())
}

#[test]
fn version_missing_declaration_fails() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let manifest = dir.path().join("fxmanifest.lua");
    write_file(&manifest, "return {}\n")?;

    let result = trek().args(["version", "-m"]).arg(&manifest).output()?;

    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr)?;
    assert!(stderr.contains("no version declaration"));
    Ok(())
}

fn directory_name(path: &std::path::Path) -> Result<String> {
    Ok(path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("directory has no name"))?
        .to_string_lossy()
        .into_owned())
}

#[test]
fn release_packs_without_bumping_version() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let name = directory_name(dir.path())?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.2.3'\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;
    let output_dir = dir.path().join("dist");

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "-o"])
        .arg(&output_dir)
        .output()?;

    assert!(result.status.success());
    assert!(output_dir.is_dir());
    let archive = output_dir.join(format!("{name}.zip"));
    assert_eq!(zip_names(&archive)?, vec!["fxmanifest.lua", "main.lua"]);
    assert_eq!(
        fs::read_to_string(dir.path().join("fxmanifest.lua"))?,
        "version '1.2.3'\n"
    );
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("# Release Report"));
    assert!(stdout.contains("- **Version:** `1.2.3` (unchanged)"));
    assert!(stdout.contains("Success (archive written)"));
    Ok(())
}

#[test]
fn release_patch_bumps_before_packing() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let name = directory_name(dir.path())?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.2.3'\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "--patch"])
        .output()?;

    assert!(result.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("fxmanifest.lua"))?,
        "version '1.2.4'\n"
    );
    let archive = dir.path().join(format!("{name}.zip"));
    assert_eq!(
        zip_file_contents(&archive, "fxmanifest.lua")?,
        "version '1.2.4'\n"
    );
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("- **Version:** `1.2.3` -> `1.2.4`"));
    Ok(())
}

#[test]
fn release_sha256_is_included_in_the_report() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let name = directory_name(dir.path())?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.2.3'\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "--sha256"])
        .output()?;

    assert!(result.status.success());
    let checksum = sha256_hex(&dir.path().join(format!("{name}.zip")))?;
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains(&format!("- **SHA-256:** `{checksum}`")));
    Ok(())
}

#[test]
fn release_uses_manifest_parent_as_pack_input() -> Result<()> {
    let root = tempfile::tempdir()?;
    let resource = root.path().join("my-resource");
    write_file(&resource.join(".pack"), "*.lua\n")?;
    write_file(&resource.join("fxmanifest.lua"), "version = \"2.0.0\"\n")?;
    write_file(&resource.join("server.lua"), "server")?;
    let output_dir = root.path().join("dist");
    fs::create_dir_all(&output_dir)?;

    let result = trek()
        .current_dir(root.path())
        .args(["release", "--minor", "--output"])
        .arg(&output_dir)
        .args(["--manifest"])
        .arg(resource.join("fxmanifest.lua"))
        .output()?;

    assert!(result.status.success());
    let archive = output_dir.join("my-resource.zip");
    assert_eq!(zip_names(&archive)?, vec!["fxmanifest.lua", "server.lua"]);
    assert_eq!(
        fs::read_to_string(resource.join("fxmanifest.lua"))?,
        "version = \"2.1.0\"\n"
    );
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("- **Version:** `2.0.0` -> `2.1.0`"));
    assert!(stdout.contains("- **Output archive:**"));
    Ok(())
}

#[test]
fn release_rejects_conflicting_bump_flags() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let name = directory_name(dir.path())?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.2.3'\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "--patch", "--minor"])
        .output()?;

    assert!(!result.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("fxmanifest.lua"))?,
        "version '1.2.3'\n"
    );
    assert!(!dir.path().join(format!("{name}.zip")).exists());
    Ok(())
}

#[test]
fn release_bump_failure_prints_report_and_exits_nonzero() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let name = directory_name(dir.path())?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("main.lua"), "main")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "--patch"])
        .output()?;

    assert!(!result.status.success());
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("# Release Report"));
    assert!(stdout.contains("failed to read manifest"));
    assert!(!dir.path().join(format!("{name}.zip")).exists());
    Ok(())
}

#[test]
fn release_pack_failure_prints_report_and_exits_nonzero() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let name = directory_name(dir.path())?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.0.0'\n")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "--patch"])
        .output()?;

    assert!(!result.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("fxmanifest.lua"))?,
        "version '1.0.1'\n"
    );
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("- **Version:** `1.0.0` -> `1.0.1`"));
    assert!(stdout.contains("failed to read pattern file"));
    assert!(!dir.path().join(format!("{name}.zip")).exists());
    Ok(())
}

#[test]
fn release_pack_validation_failure_reports_problems() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let name = directory_name(dir.path())?;
    write_file(&dir.path().join(".pack"), "\n")?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.0.0'\n")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "--patch", "--sha256"])
        .output()?;

    assert!(!result.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("fxmanifest.lua"))?,
        "version '1.0.1'\n"
    );
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("- **Version:** `1.0.0` -> `1.0.1`"));
    assert!(stdout.contains("**Status:** Failed: pack validation failed"));
    assert!(stdout.contains("- **SHA-256:** not available"));
    assert!(stdout.contains("## Problems"));
    assert!(!dir.path().join(format!("{name}.zip")).exists());
    Ok(())
}

#[test]
fn release_fails_when_output_path_is_a_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    write_file(&dir.path().join(".pack"), "*.lua\n")?;
    write_file(&dir.path().join("fxmanifest.lua"), "version '1.0.0'\n")?;
    let output = dir.path().join("dist");
    write_file(&output, "not a directory")?;

    let result = trek()
        .current_dir(dir.path())
        .args(["release", "-o"])
        .arg(&output)
        .output()?;

    assert!(!result.status.success());
    let stdout = String::from_utf8(result.stdout)?;
    assert!(stdout.contains("failed to create output directory"));
    Ok(())
}
