use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;

#[allow(unused_imports)]
use crate::dlog;

use crate::api::{auth, discovery, types::ConnectionTestResult, ImmichClient};
use crate::settings::{AccountProfile, AppConfig};

const STORE_FILE: &str = "settings.json";
const CONFIG_KEY: &str = "config";

fn load_config(app: &AppHandle) -> Result<AppConfig, String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    Ok(store
        .get(CONFIG_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}

fn save_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let store = app.store(STORE_FILE).map_err(|e| e.to_string())?;
    store.set(
        CONFIG_KEY,
        serde_json::to_value(config).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())
}

/// Login with email + password. Creates an API key on the server and saves the profile.
#[tauri::command]
pub async fn login_account(
    app: AppHandle,
    local_url: String,
    remote_url: String,
    email: String,
    password: String,
) -> Result<AccountProfile, String> {
    // Determine which URL to use for login
    let local_trimmed = local_url.trim_end_matches('/').to_string();
    let remote_trimmed = remote_url.trim_end_matches('/').to_string();

    let login_url = if !local_trimmed.is_empty() {
        let probe = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|e| e.to_string())?;
        let ping = format!("{}/api/server/ping", local_trimmed);
        if probe.get(&ping).send().await.map(|r| r.status().is_success()).unwrap_or(false) {
            local_trimmed.clone()
        } else {
            remote_trimmed.clone()
        }
    } else {
        remote_trimmed.clone()
    };

    if login_url.is_empty() {
        return Err("Please enter at least a remote server URL.".to_string());
    }

    // Authenticate
    let login_resp = auth::login(&login_url, &email, &password)
        .await
        .map_err(|e| e.to_string())?;

    // Create a named API key so the user can see/revoke it in Immich
    let machine = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Desktop".to_string());
    let key_name = format!("Immich Desktop — {}", machine);
    let api_key = auth::create_api_key(&login_url, &login_resp.access_token, &key_name)
        .await
        .map_err(|e| e.to_string())?;

    let profile = AccountProfile {
        id: uuid::Uuid::new_v4().to_string(),
        display_name: login_resp.name.clone(),
        email: login_resp.user_email,
        local_url: local_trimmed,
        remote_url: remote_trimmed,
        api_key,
        upload_folders: vec![],
        download_folder: String::new(),
        default_sync_mode: "cloud_and_local".to_string(),
        duplicate_handling: "rename".to_string(),
        enabled: true,
    };

    let mut config = load_config(&app)?;
    let is_first = config.profiles.is_empty();
    config.profiles.push(profile.clone());
    if is_first {
        config.active_profile_id = profile.id.clone();
    }
    save_config(&app, &config)?;
    let _ = app.emit("config://changed", ());

    Ok(profile)
}

#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    load_config(&app)
}

#[tauri::command]
pub async fn set_active_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let mut config = load_config(&app)?;
    if config.profiles.iter().any(|p| p.id == profile_id) {
        config.active_profile_id = profile_id;
        save_config(&app, &config)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_profile(app: AppHandle, profile_id: String) -> Result<(), String> {
    let mut config = load_config(&app)?;
    config.profiles.retain(|p| p.id != profile_id);
    if config.active_profile_id == profile_id {
        config.active_profile_id =
            config.profiles.first().map(|p| p.id.clone()).unwrap_or_default();
    }
    save_config(&app, &config)?;
    let _ = app.emit("config://changed", ());
    Ok(())
}

#[tauri::command]
pub async fn save_app_config(
    app: AppHandle,
    sync_interval_secs: u64,
    autostart: bool,
    notifications_enabled: bool,
) -> Result<(), String> {
    let mut config = load_config(&app)?;
    config.sync_interval_secs = sync_interval_secs;
    config.autostart = autostart;
    config.notifications_enabled = notifications_enabled;
    save_config(&app, &config)
}

/// Get the current connection status for the active profile,
/// including which URL mode (Local / Remote / Direct) is in use.
#[tauri::command]
pub async fn get_active_profile_status(app: AppHandle) -> Result<ConnectionTestResult, String> {
    let config = load_config(&app)?;

    let profile = match config.profiles.iter().find(|p| p.id == config.active_profile_id) {
        Some(p) => p.clone(),
        None => {
            return Ok(ConnectionTestResult {
                success: false,
                message: "No account configured.".to_string(),
                version: None,
                url_mode: None,
            })
        }
    };

    if profile.api_key.is_empty() {
        return Ok(ConnectionTestResult {
            success: false,
            message: "Account not authenticated.".to_string(),
            version: None,
            url_mode: None,
        });
    }

    let client = ImmichClient::for_profile(
        Some(profile.local_url),
        profile.remote_url,
        profile.api_key,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(client.test_connection().await)
}

/// Update editable fields on an existing profile (URLs and display name).
#[tauri::command]
pub async fn update_profile(
    app: AppHandle,
    profile_id: String,
    display_name: String,
    local_url: String,
    remote_url: String,
) -> Result<(), String> {
    let mut config = load_config(&app)?;
    if let Some(profile) = config.profiles.iter_mut().find(|p| p.id == profile_id) {
        profile.display_name = display_name;
        profile.local_url = local_url.trim_end_matches('/').to_string();
        profile.remote_url = remote_url.trim_end_matches('/').to_string();
    } else {
        return Err("Profile not found".to_string());
    }
    save_config(&app, &config)?;
    let _ = app.emit("config://changed", ());
    Ok(())
}

/// Update the upload and download folders for a profile.
#[tauri::command]
pub async fn update_sync_folders(
    app: AppHandle,
    profile_id: String,
    upload_folders: Vec<String>,
    download_folder: String,
    sync_mode: String,
    duplicate_handling: String,
) -> Result<(), String> {
    dlog!(
        "update_sync_folders: profile_id={} download_folder={:?} sync_mode={:?}",
        profile_id, download_folder, sync_mode
    );
    let mut config = load_config(&app)?;
    if let Some(profile) = config.profiles.iter_mut().find(|p| p.id == profile_id) {
        profile.upload_folders = upload_folders;
        profile.download_folder = download_folder.clone();
        profile.default_sync_mode = sync_mode;
        profile.duplicate_handling = duplicate_handling;
    } else {
        dlog!("update_sync_folders: profile_id={profile_id} NOT FOUND");
        return Err("Profile not found".to_string());
    }
    // Validate the sync root folder exists — we never create it automatically.
    if !download_folder.is_empty() && !std::path::Path::new(&download_folder).exists() {
        dlog!("update_sync_folders: folder does not exist — rejecting save");
        return Err(format!(
            "Sync folder \"{download_folder}\" does not exist. Please choose an existing folder."
        ));
    }
    dlog!("update_sync_folders: saved OK");
    save_config(&app, &config)?;
    let _ = app.emit("config://changed", ());
    Ok(())
}

/// Scan the local network for Immich servers.
#[tauri::command]
pub async fn discover_servers() -> Vec<String> {
    discovery::discover_immich_servers().await
}
