use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::manifest::Manifest;
use crate::pack::{Plan, build_plan};

/// Shared filesystem helpers for unit tests.
pub(crate) fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

/// Build a manifest rooted at `root` without touching the filesystem.
pub(crate) fn manifest_for(root: &Path, patterns: &[&str]) -> Manifest {
    Manifest {
        path: root.join(".pack"),
        root: root.to_path_buf(),
        patterns: patterns
            .iter()
            .map(|pattern| (*pattern).to_string())
            .collect(),
    }
}

/// Build a plan for `patterns` rooted at `root`.
pub(crate) fn plan_for(root: &Path, patterns: &[&str], output: &Path) -> Plan {
    build_plan(&manifest_for(root, patterns), output)
}
