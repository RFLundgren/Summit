use std::path::Path;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::{TimeZone, Utc};
use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;

use super::{
    types::{
        AssetResponse, ConnectionTestResult, MetadataSearchResponse, ServerAboutResponse,
        UploadResponse, UserResponse,
    },
    ImmichClient,
};

impl ImmichClient {
    pub async fn test_connection(&self) -> ConnectionTestResult {
        let fail = |msg: String| ConnectionTestResult {
            success: false,
            message: msg,
            version: None,
            url_mode: None,
        };

        // Step 1: ping the server (no auth required)
        let ping_url = format!("{}/api/server/ping", self.base_url);
        match self.client.get(&ping_url).send().await {
            Err(e) => return fail(format!("Cannot reach server: {}", e)),
            Ok(resp) if !resp.status().is_success() => {
                return fail(format!("Server returned unexpected status: {}", resp.status()))
            }
            Ok(_) => {}
        }

        // Step 2: verify API key via /api/users/me
        let me_url = format!("{}/api/users/me", self.base_url);
        let me_resp = match self.client.get(&me_url).send().await {
            Err(e) => return fail(format!("Error verifying API key: {}", e)),
            Ok(r) => r,
        };

        match me_resp.status().as_u16() {
            401 | 403 => return fail("Invalid API key — please check and try again.".to_string()),
            s if s >= 400 => {
                return fail(format!("Unexpected response ({})", me_resp.status()))
            }
            _ => {}
        }

        let user: UserResponse = match me_resp.json().await {
            Ok(u) => u,
            Err(e) => return fail(format!("Failed to parse server response: {}", e)),
        };

        let version = self.get_server_version().await.ok();

        ConnectionTestResult {
            success: true,
            message: format!("Connected as {} ({})", user.name, user.email),
            version,
            url_mode: Some(self.url_mode.as_str().to_string()),
        }
    }

    async fn get_server_version(&self) -> Result<String> {
        let url = format!("{}/api/server/about", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let about: ServerAboutResponse = resp.json().await?;
        Ok(about.version)
    }

    pub async fn list_assets_page(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<AssetResponse>> {
        // POST /api/search/metadata is the stable paginated listing endpoint
        // in modern Immich (1.50+).  The old GET /api/assets?page=N was removed.
        let url = format!("{}/api/search/metadata", self.base_url);
        let body = serde_json::json!({
            "page": page,
            "size": page_size,
            "withExif": true,
        });
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("list_assets failed: {}", resp.status()));
        }
        let result: MetadataSearchResponse = resp.json().await?;
        Ok(result.assets.items)
    }

    pub async fn upload_asset(
        &self,
        path: &Path,
        device_asset_id: &str,
    ) -> Result<UploadResponse> {
        let bytes = tokio::fs::read(path).await?;
        let (resp, _sha1) = self.upload_asset_with_bytes(path, bytes, device_asset_id).await?;
        Ok(resp)
    }

    pub async fn upload_asset_with_bytes(
        &self,
        path: &Path,
        bytes: Vec<u8>,
        device_asset_id: &str,
    ) -> Result<(UploadResponse, String)> {
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let metadata = tokio::fs::metadata(path).await?;

        let created = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).single())
            .unwrap_or_else(Utc::now)
            .to_rfc3339();

        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).single())
            .unwrap_or_else(Utc::now)
            .to_rfc3339();

        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        let digest = hasher.finalize();
        let sha1_hex = format!("{:x}", digest);
        let checksum_b64 = general_purpose::STANDARD.encode(&digest);

        let mime = mime_from_ext(path);

        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(file_name)
            .mime_str(&mime)?;

        let form = reqwest::multipart::Form::new()
            .part("assetData", part)
            .text("deviceAssetId", device_asset_id.to_string())
            .text("deviceId", "summit")
            .text("fileCreatedAt", created)
            .text("fileModifiedAt", modified)
            .text("isFavorite", "false");

        let url = format!("{}/api/assets", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("x-immich-checksum", checksum_b64)
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("upload failed {}: {}", status, body));
        }

        Ok((resp.json::<UploadResponse>().await?, sha1_hex))
    }

    pub async fn download_asset(&self, asset_id: &str, dest: &Path) -> Result<u64> {
        let url = format!("{}/api/assets/{}/original", self.base_url, asset_id);
        let resp = self.client.get(&url).send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("download failed: {}", resp.status()));
        }

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let ext = dest.extension().and_then(|e| e.to_str()).unwrap_or("bin");
        let part_path = dest.with_extension(format!("{}.part", ext));

        let mut file = tokio::fs::File::create(&part_path).await?;
        let mut stream = resp.bytes_stream();
        let mut total: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            total += chunk.len() as u64;
        }
        file.flush().await?;
        drop(file);

        tokio::fs::rename(&part_path, dest).await?;
        Ok(total)
    }
}

fn mime_from_ext(path: &Path) -> String {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" | "heif" => "image/heic",
        "tiff" | "tif" => "image/tiff",
        _ => "application/octet-stream",
    }
    .to_string()
}
