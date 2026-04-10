use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub id: String,
    pub display_name: String,
    pub email: String,
    /// Local network URL e.g. "http://192.168.1.50:2283" — empty if not configured
    pub local_url: String,
    /// Public/remote URL e.g. "https://photos.example.com"
    pub remote_url: String,
    pub api_key: String,
    pub upload_folders: Vec<String>,
    pub download_folder: String,
    pub default_sync_mode: String, // "cloud_and_local" | "cloud_only" | "cloud_browse"
    pub duplicate_handling: String, // "overwrite" | "rename" | "skip"
    pub enabled: bool,
}

impl Default for AccountProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            display_name: String::new(),
            email: String::new(),
            local_url: String::new(),
            remote_url: String::new(),
            api_key: String::new(),
            upload_folders: vec![],
            download_folder: String::new(),
            default_sync_mode: "cloud_and_local".to_string(),
            duplicate_handling: "rename".to_string(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub active_profile_id: String,
    pub profiles: Vec<AccountProfile>,
    pub sync_interval_secs: u64,
    pub autostart: bool,
    pub notifications_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            active_profile_id: String::new(),
            profiles: vec![],
            sync_interval_secs: 300,
            autostart: false,
            notifications_enabled: true,
        }
    }
}
