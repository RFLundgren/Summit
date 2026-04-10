use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct PingResponse {
    pub res: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerAboutResponse {
    pub version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectionTestResult {
    pub success: bool,
    pub message: String,
    pub version: Option<String>,
    pub url_mode: Option<String>, // "Local" | "Remote" | "Direct"
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExifInfo {
    pub file_size_in_byte: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetResponse {
    pub id: String,
    pub checksum: String,
    pub original_file_name: String,
    pub file_created_at: String,
    pub file_modified_at: String,
    pub r#type: String,
    pub is_trashed: bool,
    #[serde(default)]
    pub exif_info: Option<ExifInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UploadResponse {
    pub id: String,
    pub status: String, // "created" | "duplicate"
}

/// Wrapper returned by POST /api/search/metadata (modern Immich).
#[derive(Debug, Deserialize)]
pub struct MetadataSearchResponse {
    pub assets: MetadataSearchPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSearchPage {
    pub items: Vec<AssetResponse>,
    /// Next page number as a string, or null when there are no more pages.
    pub next_page: Option<String>,
}
