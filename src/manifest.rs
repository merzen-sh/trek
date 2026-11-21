use std::error::Error;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

/// A parsed `.pack` include-pattern file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Path to the `.pack` pattern file.
    pub path: PathBuf,
    /// Input directory that patterns are matched against.
    pub root: PathBuf,
    /// Raw, non-empty, non-comment include patterns.
    pub patterns: Vec<String>,
}

impl Manifest {
    /// Read and parse a `.pack` pattern file from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or is not valid UTF-8.
    pub fn read(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read pattern file {}", path.display()))?;
        Ok(Self::parse(path, &contents))
    }

    /// Parse include patterns from `.pack` file contents.
    ///
    /// Blank lines and lines whose first non-whitespace character is `#` are
    /// ignored. Surrounding whitespace is trimmed, so patterns with leading or
    /// trailing whitespace, and paths beginning with `#`, cannot be
    /// represented. Patterns are matched relative to [`Self::root`].
    #[must_use]
    pub fn parse(path: &Path, contents: &str) -> Self {
        let root = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let patterns = contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_string)
            .collect();

        Self {
            path: path.to_path_buf(),
            root,
            patterns,
        }
    }
}

/// Why an include pattern is not a valid relative glob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError {
    /// The pattern contains a NUL byte.
    NulByte,
    /// The pattern is absolute.
    Absolute,
    /// The pattern climbs above the input directory.
    EscapesRoot,
}

impl fmt::Display for PatternError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NulByte => "pattern contains a NUL byte",
            Self::Absolute => "absolute patterns are not supported",
            Self::EscapesRoot => "pattern escapes the input directory",
        })
    }
}

impl Error for PatternError {}

/// Validate that an include pattern is relative to the input directory.
///
/// # Errors
///
/// Returns [`PatternError::Absolute`] for absolute patterns,
/// [`PatternError::EscapesRoot`] when the pattern contains a `..` component,
/// and [`PatternError::NulByte`] for patterns containing a NUL byte.
pub fn validate_pattern(pattern: &str) -> std::result::Result<(), PatternError> {
    if pattern.contains('\0') {
        return Err(PatternError::NulByte);
    }

    let path = Path::new(pattern);
    if path.is_absolute() {
        return Err(PatternError::Absolute);
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PatternError::EscapesRoot);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_patterns_and_ignores_comments_and_blank_lines() {
        let manifest = Manifest::parse(
            Path::new("/tmp/.pack"),
            "\n# comment\n  *.lua  \n**/*.lua\n",
        );

        assert_eq!(manifest.root, Path::new("/tmp"));
        assert_eq!(manifest.patterns, vec!["*.lua", "**/*.lua"]);
    }

    #[test]
    fn uses_current_directory_for_bare_pattern_file_name() {
        let manifest = Manifest::parse(Path::new(".pack"), "*.lua\n");

        assert_eq!(manifest.root, Path::new("."));
        assert_eq!(manifest.patterns, vec!["*.lua"]);
    }

    #[test]
    fn accepts_relative_patterns() {
        assert_eq!(validate_pattern("*.lua"), Ok(()));
        assert_eq!(validate_pattern("**/*.lua"), Ok(()));
        assert_eq!(validate_pattern("assets/**"), Ok(()));
    }

    #[test]
    fn rejects_absolute_patterns() {
        assert_eq!(validate_pattern("/etc/passwd"), Err(PatternError::Absolute));
    }

    #[test]
    fn rejects_patterns_that_escape_the_input_directory() {
        assert_eq!(
            validate_pattern("../secret.lua"),
            Err(PatternError::EscapesRoot)
        );
    }

    #[test]
    fn rejects_nul_bytes() {
        assert_eq!(validate_pattern("a\0b"), Err(PatternError::NulByte));
    }

    #[test]
    fn reads_pattern_file_from_disk() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join(".pack");
        std::fs::write(&path, "*.lua\n")?;

        let manifest = Manifest::read(&path)?;

        assert_eq!(manifest.patterns, vec!["*.lua"]);
        assert_eq!(manifest.root, dir.path());
        Ok(())
    }

    #[test]
    fn missing_pattern_file_is_an_error() {
        assert!(Manifest::read(Path::new("/definitely/not/here/.pack")).is_err());
    }
}
