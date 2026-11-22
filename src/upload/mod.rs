//! Optional CFX asset upload support.
//!
//! This ports the original `asset-upload` TypeScript scripts: `trek upload`
//! publishes `dist/*.zip` to the CFX portal, and `trek upload --refresh-cookie`
//! validates the forum cookie without uploading anything.

mod api;
mod browser;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use tokio::io::AsyncReadExt;
use tokio::time::sleep;

use self::api::{Api, AssetVersion, ReuploadRequest};
use self::browser::BrowserSession;
use crate::cli::UploadArgs;

const CHUNK_SIZE: u64 = 2 * 1024 * 1024;
const MAX_VERSIONS: usize = 5;
const PACK_POLL_ATTEMPTS: u32 = 30;
const PACK_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Execute the `upload` subcommand.
///
/// The command reads `CFX_FORUM_COOKIE`, `ASSET_NAME`, and the optional
/// `ZIP_FILE` and `VERSION` environment variables, matching the original
/// scripts. With `--refresh-cookie`, only `CFX_FORUM_COOKIE` is required.
///
/// # Errors
///
/// Returns an error if required environment variables are missing, the browser
/// or CFX portal cannot be reached, the ZIP cannot be read, the upload fails,
/// or the generated pack cannot be downloaded.
pub fn run(args: &UploadArgs) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start async runtime")?;

    if args.refresh_cookie {
        let cookie = required_env("CFX_FORUM_COOKIE", "CFX_FORUM_COOKIE not set")?;
        return runtime.block_on(refresh_cookie(&cookie));
    }

    let config = UploadConfig::from_env()?;
    runtime.block_on(upload(config))
}

struct UploadConfig {
    forum_cookie: String,
    asset_name: String,
    zip_file: Option<String>,
    version: Option<String>,
}

impl UploadConfig {
    fn from_env() -> Result<Self> {
        Ok(Self {
            forum_cookie: required_env("CFX_FORUM_COOKIE", "CFX_FORUM_COOKIE not set")?,
            asset_name: required_env("ASSET_NAME", "ASSET_NAME not set (e.g. trek-core)")?,
            zip_file: optional_env("ZIP_FILE"),
            version: optional_env("VERSION"),
        })
    }
}

fn required_env(name: &str, message: &str) -> Result<String> {
    optional_env(name).ok_or_else(|| anyhow!(message.to_string()))
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

async fn refresh_cookie(forum_cookie: &str) -> Result<()> {
    let session = BrowserSession::launch().await?;
    let result = session.authenticate(forum_cookie).await;
    let close = session.close().await;
    result?;
    close
}

async fn upload(config: UploadConfig) -> Result<()> {
    let dist = std::env::current_dir()
        .context("failed to resolve current directory")?
        .join("dist");
    let zip = SelectedZip::select(&dist, config.zip_file.as_deref())?;

    let session = BrowserSession::launch().await?;
    let result = upload_with_session(&session, &config, &zip, &dist).await;
    let close = session.close().await;
    result?;
    close
}

async fn upload_with_session(
    session: &BrowserSession,
    config: &UploadConfig,
    zip: &SelectedZip,
    dist: &Path,
) -> Result<()> {
    let cookie = session.login(&config.forum_cookie).await?;
    let api = Api::new(&cookie)?;
    let asset_id = api.find_asset_id(&config.asset_name).await?;
    let metadata = fs::metadata(&zip.path)
        .with_context(|| format!("failed to read {}", zip.path.display()))?;
    let total_size = metadata.len();
    let original_file_name = file_name(&zip.name);
    let chunk_count = total_size.div_ceil(CHUNK_SIZE);
    let version = config
        .version
        .clone()
        .unwrap_or_else(|| derive_version(&zip.name));

    println!("Uploading {original_file_name} ({total_size} bytes, {chunk_count} chunks)");

    let mut versions = api.asset_versions(asset_id).await?;
    prune_versions(&api, asset_id, &mut versions, &version).await?;

    let response = api
        .reupload(
            asset_id,
            &ReuploadRequest {
                chunk_count,
                chunk_size: CHUNK_SIZE,
                name: &original_file_name,
                original_file_name: &original_file_name,
                total_size,
                release_candidate: false,
                version: &version,
                changelog: "",
            },
        )
        .await?;

    if let Some(errors) = response.errors {
        bail!("Re-upload failed: {errors}");
    }
    let version_id = response.version_id;

    upload_chunks(&api, asset_id, version_id, &zip.path, chunk_count).await?;
    api.complete_upload(asset_id, version_id).await?;
    println!("Upload completed! Waiting for pack generation...");

    let pack_id = wait_for_pack(&api, asset_id, version_id).await?;
    println!("Pack ID: {pack_id}");

    let download_url = api.pack_download_url(asset_id, version_id, pack_id).await?;
    let output = dist.join(pack_file_name(&zip.name));
    println!("Downloading pack...");
    api.download(&download_url, &output).await?;
    println!("Pack saved to: {}", output.display());

    Ok(())
}

async fn prune_versions(
    api: &Api,
    asset_id: i64,
    versions: &mut Vec<AssetVersion>,
    version: &str,
) -> Result<()> {
    if let Some(index) = duplicate_version_index(versions, version) {
        let existing = &versions[index];
        println!(
            "Deleting existing version {} ({})...",
            existing.id,
            existing.version.as_deref().unwrap_or("")
        );
        api.delete_version(asset_id, existing.id).await?;
        versions.remove(index);
    }

    while versions.len() >= MAX_VERSIONS {
        let Some(index) = oldest_version_index(versions) else {
            break;
        };
        let oldest = &versions[index];
        let id = oldest.id;
        let version_text = oldest.version.clone().unwrap_or_default();
        println!("Version limit {MAX_VERSIONS} reached, deleting oldest {id} ({version_text})...");
        api.delete_version(asset_id, id).await?;
        versions.remove(index);
    }

    Ok(())
}

fn duplicate_version_index(versions: &[AssetVersion], version: &str) -> Option<usize> {
    versions
        .iter()
        .position(|candidate| candidate.version.as_deref() == Some(version))
}

fn oldest_version_index(versions: &[AssetVersion]) -> Option<usize> {
    versions
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| candidate.id)
        .map(|(index, _)| index)
}

async fn upload_chunks(
    api: &Api,
    asset_id: i64,
    version_id: i64,
    path: &Path,
    chunk_count: u64,
) -> Result<()> {
    let mut file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("failed to open {}", path.display()))?;
    let buffer_size = usize::try_from(CHUNK_SIZE).context("chunk size exceeds platform limits")?;
    let mut buffer = vec![0_u8; buffer_size];
    let mut index = 0_u64;

    loop {
        let read = read_chunk(&mut file, &mut buffer)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        api.upload_chunk(asset_id, version_id, index, &buffer[..read])
            .await
            .with_context(|| format!("failed to upload chunk {}/{}", index + 1, chunk_count))?;
        println!("Uploaded chunk {}/{}", index + 1, chunk_count);
        index += 1;
    }

    Ok(())
}

async fn read_chunk(file: &mut tokio::fs::File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        let read = file.read(&mut buffer[filled..]).await?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    Ok(filled)
}

async fn wait_for_pack(api: &Api, asset_id: i64, version_id: i64) -> Result<i64> {
    for attempt in 0..PACK_POLL_ATTEMPTS {
        sleep(PACK_POLL_INTERVAL).await;
        if let Some(pack_id) = api.pack_id(asset_id, version_id).await? {
            return Ok(pack_id);
        }
        if attempt % 5 == 0 {
            println!("  waiting for pack...");
        }
    }

    bail!("Pack not generated after 60s")
}

struct SelectedZip {
    name: String,
    path: PathBuf,
}

impl SelectedZip {
    fn select(dist: &Path, requested: Option<&str>) -> Result<Self> {
        let name = match requested {
            Some(name) => name.to_string(),
            None => first_zip(dist)?,
        };
        Ok(Self {
            path: dist.join(&name),
            name,
        })
    }
}

#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "the original script matches lowercase `.zip` only"
)]
fn first_zip(dist: &Path) -> Result<String> {
    let entries = fs::read_dir(dist)
        .with_context(|| format!("failed to read directory {}", dist.display()))?;

    for entry in entries {
        let entry =
            entry.with_context(|| format!("failed to read directory {}", dist.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".zip") {
            return Ok(name);
        }
    }

    bail!("No .zip file found in dist/")
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map_or_else(
        || path.to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn strip_zip_suffix(name: &str) -> &str {
    name.strip_suffix(".zip").unwrap_or(name)
}

fn pack_file_name(zip_name: &str) -> String {
    format!("{}.pack.zip", strip_zip_suffix(&file_name(zip_name)))
}

/// Derive the version from a ZIP name using the original `index.ts` regex.
///
/// The pattern is `^.*?[-](\d[\w.]+\d)`, followed by removing a trailing
/// `.zip`. The first hyphen followed by an ASCII digit wins, and the capture
/// ends at the last digit in the following word/dot run.
fn derive_version(zip_file: &str) -> String {
    let replaced = find_version_capture(zip_file).map_or_else(
        || zip_file.to_string(),
        |(start, end)| format!("{}{}", &zip_file[start..end], &zip_file[end..]),
    );
    strip_zip_suffix(&replaced).to_string()
}

fn find_version_capture(value: &str) -> Option<(usize, usize)> {
    let bytes = value.as_bytes();
    let search_end = value
        .char_indices()
        .find(|(_, character)| matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
        .map_or(value.len(), |(index, _)| index);

    for index in 0..search_end.saturating_sub(1) {
        if bytes[index] == b'-' && bytes[index + 1].is_ascii_digit() {
            let start = index + 1;
            if let Some(end) = version_capture_end(bytes, start) {
                return Some((start, end));
            }
        }
    }

    None
}

fn version_capture_end(bytes: &[u8], start: usize) -> Option<usize> {
    if !bytes.get(start)?.is_ascii_digit() {
        return None;
    }

    let mut run_end = start;
    while run_end < bytes.len() && is_word_or_dot(bytes[run_end]) {
        run_end += 1;
    }

    let last_digit = (start..run_end)
        .rev()
        .find(|&index| bytes[index].is_ascii_digit())?;
    let end = last_digit + 1;

    (end - start >= 3).then_some(end)
}

fn is_word_or_dot(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::write_file;

    fn version(id: i64, version: &str) -> AssetVersion {
        AssetVersion {
            id,
            version: Some(version.to_string()),
            packs: None,
        }
    }

    #[test]
    fn finds_duplicate_versions_by_exact_name() {
        let versions = vec![version(1, "1.2.3"), version(2, "1.2.4")];

        assert_eq!(duplicate_version_index(&versions, "1.2.4"), Some(1));
        assert_eq!(duplicate_version_index(&versions, "1.2.5"), None);
    }

    #[test]
    fn finds_the_oldest_version_by_id() {
        let versions = vec![
            version(9, "2.0.0"),
            version(3, "1.0.0"),
            version(7, "1.1.0"),
        ];

        assert_eq!(oldest_version_index(&versions), Some(1));
        assert_eq!(oldest_version_index(&[]), None);
    }

    #[test]
    fn selects_the_first_zip_in_dist() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("notes.txt"), "notes")?;
        write_file(&dir.path().join("trek-core-1.2.3.zip"), "zip")?;

        assert_eq!(first_zip(dir.path())?, "trek-core-1.2.3.zip");
        Ok(())
    }

    #[test]
    fn reports_when_dist_has_no_zip() -> Result<()> {
        let dir = tempfile::tempdir()?;
        write_file(&dir.path().join("notes.txt"), "notes")?;

        let error = first_zip(dir.path()).expect_err("expected a missing ZIP error");
        assert_eq!(error.to_string(), "No .zip file found in dist/");
        Ok(())
    }

    #[test]
    fn selected_zip_uses_the_requested_name() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let selected = SelectedZip::select(dir.path(), Some("custom.zip"))?;

        assert_eq!(selected.name, "custom.zip");
        assert_eq!(selected.path, dir.path().join("custom.zip"));
        Ok(())
    }

    #[test]
    fn derives_version_like_the_typescript_script() {
        let cases = [
            ("trek-core-1.2.3.zip", "1.2.3"),
            ("trek-core-1.0.0.zip", "1.0.0"),
            ("foo-12.zip", "foo-12"),
            ("foo-1.2.zip", "1.2"),
            ("foo-1.2.3-bar.zip", "1.2.3-bar"),
            ("foo-1.2.3abc.zip", "1.2.3abc"),
            ("foo-abc.zip", "foo-abc"),
            ("foo-1.2.3", "1.2.3"),
            ("foo-bar-1.2.3.zip", "1.2.3"),
            ("1.2.3.zip", "1.2.3"),
            ("build\n-1.2.3.zip", "build\n-1.2.3"),
        ];

        for (input, expected) in cases {
            assert_eq!(derive_version(input), expected, "input: {input}");
        }
    }

    #[test]
    fn strips_only_the_final_zip_suffix() {
        assert_eq!(strip_zip_suffix("foo.zip"), "foo");
        assert_eq!(strip_zip_suffix("foo.zip.zip"), "foo.zip");
        assert_eq!(strip_zip_suffix("foo"), "foo");
    }

    #[test]
    fn reads_file_name_from_a_path() {
        assert_eq!(file_name("/tmp/dist/trek-core.zip"), "trek-core.zip");
        assert_eq!(file_name("trek-core.zip"), "trek-core.zip");
    }

    #[test]
    fn pack_output_name_replaces_the_zip_suffix() {
        assert_eq!(
            pack_file_name("dist/trek-core-1.2.3.zip"),
            "trek-core-1.2.3.pack.zip"
        );
        assert_eq!(pack_file_name("custom.zip"), "custom.pack.zip");
    }

    #[tokio::test]
    async fn read_chunk_fills_two_mib_and_returns_the_tail() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("archive.zip");
        let chunk = usize::try_from(CHUNK_SIZE)?;
        std::fs::write(&path, vec![0_u8; chunk + 5])?;

        let mut file = tokio::fs::File::open(&path).await?;
        let mut buffer = vec![0_u8; chunk];
        assert_eq!(read_chunk(&mut file, &mut buffer).await?, chunk);
        assert_eq!(read_chunk(&mut file, &mut buffer).await?, 5);
        assert_eq!(read_chunk(&mut file, &mut buffer).await?, 0);
        Ok(())
    }
}
