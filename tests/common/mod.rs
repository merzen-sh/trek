use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::Result;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

pub fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

pub fn zip_names(path: &Path) -> Result<Vec<String>> {
    let mut archive = ZipArchive::new(fs::File::open(path)?)?;
    let mut names = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        names.push(file.name()?.to_string());
    }
    Ok(names)
}

pub fn sha256_hex(path: &Path) -> Result<String> {
    let digest = Sha256::digest(fs::read(path)?);
    let mut checksum = String::with_capacity(digest.len() * 2);
    for &byte in &digest {
        let _ = write!(checksum, "{byte:02x}");
    }
    Ok(checksum)
}
