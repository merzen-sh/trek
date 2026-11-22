use std::path::Path;

use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use reqwest::header::{COOKIE, HeaderValue};
use reqwest::multipart::{Form, Part};
use reqwest::{Client, Method, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncWriteExt, BufWriter};

const API_BASE: &str = "https://portal-api.cfx.re";

pub(crate) struct Api {
    client: Client,
    cookie: HeaderValue,
}

impl Api {
    pub(crate) fn new(cookie: &str) -> Result<Self> {
        let cookie = HeaderValue::from_str(cookie)
            .context("browser cookie string contains invalid header characters")?;
        let client = Client::builder()
            .build()
            .context("failed to create HTTP client")?;

        Ok(Self { client, cookie })
    }

    pub(crate) async fn find_asset_id(&self, name: &str) -> Result<i64> {
        let response: SearchResponse = self
            .send_json(
                self.request(Method::GET, format!("{API_BASE}/v1/me/assets"))
                    .query(&[
                        ("search", name),
                        ("sort", "asset.name"),
                        ("direction", "asc"),
                    ]),
                "searching assets",
            )
            .await?;

        response
            .items
            .first()
            .map(|asset| asset.id)
            .ok_or_else(|| anyhow!("Failed to find asset id for \"{name}\"."))
    }

    pub(crate) async fn asset_versions(&self, asset_id: i64) -> Result<Vec<AssetVersion>> {
        let details: AssetDetails = self
            .send_json(
                self.request(Method::GET, format!("{API_BASE}/v1/assets/{asset_id}")),
                "loading asset",
            )
            .await?;

        Ok(details.versions.unwrap_or_default())
    }

    pub(crate) async fn delete_version(&self, asset_id: i64, version_id: i64) -> Result<()> {
        self.send_empty(
            self.request(
                Method::DELETE,
                format!("{API_BASE}/v1/assets/{asset_id}/versions/{version_id}"),
            ),
            "deleting asset version",
        )
        .await
    }

    pub(crate) async fn reupload(
        &self,
        asset_id: i64,
        request: &ReuploadRequest<'_>,
    ) -> Result<ReuploadResponse> {
        self.send_json(
            self.request(
                Method::POST,
                format!("{API_BASE}/v1/assets/{asset_id}/re-upload"),
            )
            .json(request),
            "starting re-upload",
        )
        .await
    }

    pub(crate) async fn upload_chunk(
        &self,
        asset_id: i64,
        version_id: i64,
        index: u64,
        chunk: &[u8],
    ) -> Result<()> {
        let part = Part::bytes(chunk.to_vec())
            .file_name("blob")
            .mime_str("application/octet-stream")
            .context("failed to build upload chunk")?;
        let form = Form::new()
            .text("chunk_id", index.to_string())
            .part("chunk", part);

        self.send_empty(
            self.request(
                Method::POST,
                format!("{API_BASE}/v1/assets/{asset_id}/versions/{version_id}/upload-chunk"),
            )
            .multipart(form),
            "uploading chunk",
        )
        .await
    }

    pub(crate) async fn complete_upload(&self, asset_id: i64, version_id: i64) -> Result<()> {
        self.send_empty(
            self.request(
                Method::POST,
                format!("{API_BASE}/v1/assets/{asset_id}/versions/{version_id}/complete-upload"),
            )
            .json(&serde_json::json!({})),
            "completing upload",
        )
        .await
    }

    pub(crate) async fn pack_id(&self, asset_id: i64, version_id: i64) -> Result<Option<i64>> {
        let details: AssetDetails = self
            .send_json(
                self.request(Method::GET, format!("{API_BASE}/v1/assets/{asset_id}")),
                "checking pack generation",
            )
            .await?;

        let pack_id = details
            .versions
            .unwrap_or_default()
            .into_iter()
            .find(|version| version.id == version_id)
            .and_then(|version| version.packs)
            .and_then(|packs| packs.into_iter().next())
            .map(|pack| pack.id)
            .filter(|id| *id != 0);

        Ok(pack_id)
    }

    pub(crate) async fn pack_download_url(
        &self,
        asset_id: i64,
        version_id: i64,
        pack_id: i64,
    ) -> Result<String> {
        let response: DownloadResponse = self
            .send_json(
                self.request(
                    Method::GET,
                    format!(
                        "{API_BASE}/v1/assets/{asset_id}/versions/{version_id}/packs/{pack_id}/download"
                    ),
                ),
                "loading pack download URL",
            )
            .await?;

        Ok(response.url)
    }

    pub(crate) async fn download(&self, url: &str, path: &Path) -> Result<()> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("failed to download pack")?;
        let response = response
            .error_for_status()
            .context("pack download failed")?;

        let file = tokio::fs::File::create(path)
            .await
            .with_context(|| format!("failed to create {}", path.display()))?;
        let mut file = BufWriter::new(file);
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("failed to read pack download")?;
            file.write_all(&chunk)
                .await
                .with_context(|| format!("failed to write {}", path.display()))?;
        }

        file.flush()
            .await
            .with_context(|| format!("failed to flush {}", path.display()))?;

        Ok(())
    }

    fn request(&self, method: Method, url: impl AsRef<str>) -> RequestBuilder {
        self.client
            .request(method, url.as_ref())
            .header(COOKIE, self.cookie.clone())
    }

    async fn send_json<T: DeserializeOwned>(
        &self,
        request: RequestBuilder,
        action: &str,
    ) -> Result<T> {
        let response = request
            .send()
            .await
            .with_context(|| format!("{action}: request failed"))?;
        let response = response
            .error_for_status()
            .with_context(|| format!("{action}: request failed"))?;

        response
            .json()
            .await
            .with_context(|| format!("{action}: failed to parse response"))
    }

    async fn send_empty(&self, request: RequestBuilder, action: &str) -> Result<()> {
        let response = request
            .send()
            .await
            .with_context(|| format!("{action}: request failed"))?;
        response
            .error_for_status()
            .with_context(|| format!("{action}: request failed"))?;

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Vec<AssetSummary>,
}

#[derive(Debug, Deserialize)]
struct AssetSummary {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct AssetDetails {
    versions: Option<Vec<AssetVersion>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AssetVersion {
    pub(crate) id: i64,
    pub(crate) version: Option<String>,
    pub(crate) packs: Option<Vec<Pack>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Pack {
    pub(crate) id: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReuploadRequest<'a> {
    pub(crate) chunk_count: u64,
    pub(crate) chunk_size: u64,
    pub(crate) name: &'a str,
    pub(crate) original_file_name: &'a str,
    pub(crate) total_size: u64,
    pub(crate) release_candidate: bool,
    pub(crate) version: &'a str,
    pub(crate) changelog: &'a str,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReuploadResponse {
    pub(crate) version_id: i64,
    #[serde(default)]
    pub(crate) errors: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct DownloadResponse {
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reupload_request_matches_the_typescript_payload() -> Result<()> {
        let request = ReuploadRequest {
            chunk_count: 2,
            chunk_size: 2 * 1024 * 1024,
            name: "trek-core.zip",
            original_file_name: "trek-core.zip",
            total_size: 3_000_000,
            release_candidate: false,
            version: "1.2.3",
            changelog: "",
        };

        assert_eq!(
            serde_json::to_value(request)?,
            serde_json::json!({
                "chunk_count": 2,
                "chunk_size": 2 * 1024 * 1024,
                "name": "trek-core.zip",
                "original_file_name": "trek-core.zip",
                "total_size": 3_000_000,
                "release_candidate": false,
                "version": "1.2.3",
                "changelog": ""
            })
        );
        Ok(())
    }

    #[test]
    fn reupload_errors_are_nullable() -> Result<()> {
        let response: ReuploadResponse =
            serde_json::from_value(serde_json::json!({"version_id": 7, "errors": null}))?;
        assert_eq!(response.version_id, 7);
        assert!(response.errors.is_none());

        let response: ReuploadResponse = serde_json::from_value(
            serde_json::json!({"version_id": 8, "errors": {"chunk": "invalid"}}),
        )?;
        assert_eq!(
            response.errors,
            Some(serde_json::json!({"chunk": "invalid"}))
        );

        let response: ReuploadResponse =
            serde_json::from_value(serde_json::json!({"version_id": 9}))?;
        assert!(response.errors.is_none());
        Ok(())
    }
}
