pub mod db;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::{mpsc, RwLock};

#[allow(unused_imports)]
use crate::dlog;
use crate::api::ImmichClient;
use crate::cloud_files::CloudFilesProvider;
use crate::settings::{AccountProfile, AppConfig};

const STORE_FILE: &str = "settings.json";
const CONFIG_KEY: &str = "config";

pub struct SyncEngine {
    app: AppHandle,
    pub(crate) db: Arc<Mutex<rusqlite::Connection>>,
    paused: Arc<AtomicBool>,
    pub status: Arc<RwLock<SyncStatus>>,
    trigger_tx: mpsc::Sender<()>,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub phase: String,
    pub uploaded: u32,
    pub downloaded: u32,
    pub skipped: u32,
    pub errors: u32,
    pub last_sync_at: Option<String>,
    pub message: String,
}

impl SyncStatus {
    fn idle() -> Self {
        Self {
            phase: "idle".to_string(),
            message: "Ready".to_string(),
            ..Default::default()
        }
    }
}

impl SyncEngine {
    pub fn init(app: AppHandle, db_path: PathBuf) -> Result<Arc<Self>, String> {
        let conn = db::open(&db_path).map_err(|e| e.to_string())?;
        let (trigger_tx, trigger_rx) = mpsc::channel::<()>(1);

        // Spawn file-watcher: triggers an immediate sync when new images appear
        // in any upload folder without waiting for the next interval.
        spawn_watcher(app.clone(), trigger_tx.clone());

        let engine = Arc::new(Self {
            app,
            db: Arc::new(Mutex::new(conn)),
            paused: Arc::new(AtomicBool::new(false)),
            status: Arc::new(RwLock::new(SyncStatus::idle())),
            trigger_tx,
        });

        let engine_clone = Arc::clone(&engine);
        tauri::async_runtime::spawn(async move {
            engine_clone.run_loop(trigger_rx).await;
        });

        Ok(engine)
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub async fn trigger(&self) {
        let _ = self.trigger_tx.try_send(());
    }

    // ── internals ─────────────────────────────────────────────────────────────

    /// Run `f` with a locked DB connection and immediately release the lock.
    /// Never holds the guard across an `.await`, so the future stays `Send`.
    fn with_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&rusqlite::Connection) -> R,
    {
        let db = self.db.lock().unwrap();
        f(&db)
    }

    fn load_config(&self) -> AppConfig {
        self.app
            .store(STORE_FILE)
            .ok()
            .and_then(|s| s.get(CONFIG_KEY))
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    async fn set_phase(&self, phase: &str, message: &str) {
        let mut s = self.status.write().await;
        s.phase = phase.to_string();
        s.message = message.to_string();
        drop(s);
        self.emit_status().await;
    }

    async fn emit_status(&self) {
        let s = self.status.read().await.clone();
        let _ = self.app.emit("sync://status", s);
    }

    async fn run_loop(&self, mut trigger_rx: mpsc::Receiver<()>) {
        // Small startup delay so the app can finish initialising, then run
        // one sync immediately rather than waiting for the full interval.
        tokio::time::sleep(Duration::from_secs(5)).await;
        self.run_sync_cycle().await;

        loop {
            let interval_secs = self.load_config().sync_interval_secs.max(60);

            tokio::select! {
                _ = trigger_rx.recv() => {}
                _ = tokio::time::sleep(Duration::from_secs(interval_secs)) => {}
            }

            if self.paused.load(Ordering::Relaxed) {
                self.set_phase("paused", "Sync paused").await;
                continue;
            }

            self.run_sync_cycle().await;
        }
    }

    async fn run_sync_cycle(&self) {
        let config = self.load_config();

        dlog!(
            "run_sync_cycle: {} total profiles, active={}",
            config.profiles.len(),
            config.active_profile_id
        );
        for p in &config.profiles {
            dlog!(
                "  profile id={} name={:?} enabled={} api_key_set={} sync_mode={:?} download_folder={:?}",
                p.id, p.display_name, p.enabled, !p.api_key.is_empty(),
                p.default_sync_mode, p.download_folder
            );
        }

        // Sync ALL enabled profiles, not just the active one
        let profiles: Vec<_> = config
            .profiles
            .into_iter()
            .filter(|p| p.enabled && !p.api_key.is_empty())
            .collect();

        if profiles.is_empty() {
            dlog!("run_sync_cycle: no enabled profiles with API keys — skipping");
            return;
        }

        // Reset per-cycle counters
        {
            let mut s = self.status.write().await;
            s.uploaded = 0;
            s.downloaded = 0;
            s.skipped = 0;
            s.errors = 0;
        }

        for profile in &profiles {
            if self.paused.load(Ordering::Relaxed) {
                self.set_phase("paused", "Sync paused").await;
                return;
            }
            self.sync_profile(profile).await;
        }

        let n = profiles.len();
        let mut s = self.status.write().await;
        s.phase = "idle".to_string();
        s.last_sync_at = Some(chrono::Utc::now().to_rfc3339());
        s.message = format!(
            "Last sync: ↑{} ↓{} skip:{} err:{} ({} account{})",
            s.uploaded, s.downloaded, s.skipped, s.errors,
            n, if n == 1 { "" } else { "s" }
        );
        drop(s);
        self.emit_status().await;
    }

    async fn sync_profile(&self, profile: &AccountProfile) {
        dlog!(
            "sync_profile: name={:?} mode={:?} download_folder={:?} folder_exists={}",
            profile.display_name,
            profile.default_sync_mode,
            profile.download_folder,
            std::path::Path::new(&profile.download_folder).exists()
        );
        let client = match ImmichClient::for_profile(
            Some(profile.local_url.clone()),
            profile.remote_url.clone(),
            profile.api_key.clone(),
        )
        .await
        {
            Ok(c) => c,
            Err(e) => {
                self.set_phase(
                    "error",
                    &format!("{}: connection failed: {e}", profile.display_name),
                )
                .await;
                return;
            }
        };

        self.run_upload(profile, &client).await;

        if self.paused.load(Ordering::Relaxed) {
            return;
        }

        match profile.default_sync_mode.as_str() {
            "cloud_and_local" if !profile.download_folder.is_empty() => {
                self.run_download(profile, &client).await;
            }
            "cloud_browse" if !profile.download_folder.is_empty() => {
                self.run_cloud_browse(profile, client).await;
            }
            _ => {}
        }
    }

    // ── cloud browse (Files On-Demand) ────────────────────────────────────────

    async fn run_cloud_browse(&self, profile: &AccountProfile, client: ImmichClient) {
        self.set_phase("downloading", "Checking for new placeholders…").await;

        let download_dir = PathBuf::from(&profile.download_folder);
        log::info!(
            "cloud_browse: profile={} sync_root={}",
            profile.display_name, profile.download_folder
        );

        // Ensure the sync root is registered and connected.
        let cf = match self.app.try_state::<std::sync::Arc<CloudFilesProvider>>() {
            Some(p) => p,
            None => {
                log::error!("CloudFilesProvider not found in app state");
                return;
            }
        };

        let client_arc = std::sync::Arc::new(client);

        {
            let cf2 = std::sync::Arc::clone(&*cf);
            let profile_id2 = profile.id.clone();
            let download_dir2 = download_dir.clone();
            let client2 = std::sync::Arc::clone(&client_arc);
            let result = tokio::task::spawn_blocking(move || {
                cf2.ensure_connected(&profile_id2, &download_dir2, client2)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r);
            if let Err(e) = result {
                log::error!("cloud_browse connect failed for {}: {e}", profile.display_name);
                return;
            }
        }
        log::info!("cloud_browse: sync root connected OK");

        // Paginate Immich assets and create placeholders for unknown ones.
        let mut page = 1u32;
        let mut new_placeholders: u32 = 0;

        loop {
            if self.paused.load(Ordering::Relaxed) {
                return;
            }

            let assets = match client_arc.list_assets_page(page, 100).await {
                Ok(a) => a,
                Err(e) => {
                    log::error!("cloud_browse: list_assets_page(page={page}) failed: {e}");
                    break;
                }
            };
            log::info!(
                "cloud_browse: page={page} returned {} assets",
                assets.len()
            );
            if assets.is_empty() {
                break;
            }

            // Filter: images only, not trashed, not already in DB.
            let new_assets: Vec<_> = assets
                .iter()
                .filter(|a| !a.is_trashed && a.r#type == "IMAGE")
                .filter(|a| {
                    let known = self.with_db(|db| {
                        db::has_asset(db, &profile.id, &a.id).unwrap_or(false)
                    });
                    !known
                })
                .cloned()
                .collect();
            log::info!(
                "cloud_browse: page={page} {} new IMAGE assets after filtering",
                new_assets.len()
            );

            if !new_assets.is_empty() {
                match cf.create_placeholders_batch(&profile.id, &new_assets) {
                    Ok(n) => {
                        // Record each placeholder in the DB as a "placeholder" download.
                        for asset in &new_assets {
                            let dest = download_dir.join(&asset.original_file_name);
                            let dest_str = dest.to_string_lossy().to_string();
                            self.with_db(|db| {
                                let _ = db::upsert_download(
                                    db,
                                    &profile.id,
                                    &asset.id,
                                    &dest_str,
                                    &asset.original_file_name,
                                    0, // size unknown until hydrated
                                );
                                let _ = db::log_activity(
                                    db,
                                    &profile.id,
                                    "download",
                                    &asset.original_file_name,
                                    Some(&dest_str),
                                    Some(&asset.id),
                                    None,
                                    "Placeholder created",
                                );
                            });
                        }
                        new_placeholders += n;
                        if n > 0 {
                            let mut s = self.status.write().await;
                            s.downloaded += n;
                            s.message = format!("Created {new_placeholders} placeholder(s)");
                            drop(s);
                            self.emit_status().await;
                        }
                    }
                    Err(e) => {
                        log::error!("create_placeholders_batch failed: {e}");
                    }
                }
            }

            page += 1;
        }
    }

    // ── upload ────────────────────────────────────────────────────────────────

    async fn run_upload(&self, profile: &AccountProfile, client: &ImmichClient) {
        self.set_phase("uploading", "Scanning upload folders…").await;

        for folder in &profile.upload_folders {
            let dir = PathBuf::from(folder);
            if !dir.exists() {
                continue;
            }

            let files = walk_images(&dir);

            for file_path in files {
                if self.paused.load(Ordering::Relaxed) {
                    return;
                }

                let bytes = match tokio::fs::read(&file_path).await {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                let hash = sha1_hex(&bytes);
                let file_name = file_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let local_path = file_path.to_string_lossy().to_string();

                // Check DB: already uploaded with this hash?
                let already = self.with_db(|db| {
                    db::is_uploaded(db, &profile.id, &hash).unwrap_or(false)
                });
                if already {
                    let mut s = self.status.write().await;
                    s.skipped += 1;
                    continue;
                }

                match client
                    .upload_asset_with_bytes(&file_path, bytes, &hash)
                    .await
                {
                    Ok((resp, _)) => {
                        let file_size = tokio::fs::metadata(&file_path)
                            .await
                            .map(|m| m.len())
                            .unwrap_or(0);

                        let is_dup = resp.status == "duplicate";
                        let msg = if is_dup {
                            "Already on server".to_string()
                        } else {
                            format!("Uploaded ({file_size} bytes)")
                        };
                        self.with_db(|db| {
                            let _ = db::upsert_upload(
                                db, &profile.id, &local_path, &resp.id,
                                &hash, file_size, &file_name,
                            );
                            let _ = db::log_activity(
                                db, &profile.id,
                                if is_dup { "skip" } else { "upload" },
                                &file_name, Some(&local_path),
                                Some(&resp.id), Some(file_size as i64), &msg,
                            );
                        });

                        let mut s = self.status.write().await;
                        if is_dup { s.skipped += 1; } else { s.uploaded += 1; }
                        s.message = format!("Uploading: {file_name}");
                        drop(s);
                        self.emit_status().await;
                    }
                    Err(e) => {
                        self.with_db(|db| {
                            let _ = db::log_activity(
                                db, &profile.id, "error", &file_name,
                                Some(&local_path), None, None,
                                &format!("Upload failed: {e}"),
                            );
                        });
                        let mut s = self.status.write().await;
                        s.errors += 1;
                    }
                }
            }
        }
    }

    // ── download ──────────────────────────────────────────────────────────────

    async fn run_download(&self, profile: &AccountProfile, client: &ImmichClient) {
        self.set_phase("downloading", "Checking for new photos…").await;

        let download_dir = PathBuf::from(&profile.download_folder);
        dlog!("run_download: folder={:?} exists={}", download_dir, download_dir.exists());
        if !download_dir.exists() {
            dlog!("run_download: ABORTING — folder does not exist");
            log::error!(
                "Sync folder \"{}\" does not exist. Please choose a folder in Settings.",
                download_dir.display()
            );
            return;
        }

        let mut page = 1u32;
        loop {
            if self.paused.load(Ordering::Relaxed) {
                return;
            }

            let assets = match client.list_assets_page(page, 100).await {
                Ok(a) => a,
                Err(_) => break,
            };
            if assets.is_empty() {
                break;
            }

            for asset in &assets {
                if asset.is_trashed || asset.r#type != "IMAGE" {
                    continue;
                }

                let known = self.with_db(|db| {
                    db::has_asset(db, &profile.id, &asset.id).unwrap_or(false)
                });
                if known {
                    continue;
                }

                let file_name = &asset.original_file_name;
                let candidate = download_dir.join(file_name);

                let final_dest = if candidate.exists() {
                    match profile.duplicate_handling.as_str() {
                        "overwrite" => candidate,
                        "skip" => {
                            // Mark known so we don't check every cycle, then skip
                            let path_str = candidate.to_string_lossy().to_string();
                            self.with_db(|db| {
                                let _ = db::upsert_download(
                                    db, &profile.id, &asset.id,
                                    &path_str, file_name, 0,
                                );
                            });
                            let mut s = self.status.write().await;
                            s.skipped += 1;
                            drop(s);
                            continue;
                        }
                        _ => {
                            // "rename" — add conflict timestamp suffix
                            let stem = Path::new(file_name)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or(file_name);
                            let ext = Path::new(file_name)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("");
                            let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                            let new_name = if ext.is_empty() {
                                format!("{stem}.conflict-{ts}")
                            } else {
                                format!("{stem}.conflict-{ts}.{ext}")
                            };
                            download_dir.join(new_name)
                        }
                    }
                } else {
                    candidate
                };

                match client.download_asset(&asset.id, &final_dest).await {
                    Ok(size) => {
                        let dest_str = final_dest.to_string_lossy().to_string();
                        self.with_db(|db| {
                            let _ = db::upsert_download(
                                db, &profile.id, &asset.id,
                                &dest_str, file_name, size as i64,
                            );
                            let _ = db::log_activity(
                                db, &profile.id, "download", file_name,
                                Some(&dest_str), Some(&asset.id),
                                Some(size as i64), "Downloaded",
                            );
                        });

                        let mut s = self.status.write().await;
                        s.downloaded += 1;
                        s.message = format!("Downloading: {file_name}");
                        drop(s);
                        self.emit_status().await;
                    }
                    Err(e) => {
                        self.with_db(|db| {
                            let _ = db::log_activity(
                                db, &profile.id, "error", file_name,
                                None, Some(&asset.id), None,
                                &format!("Download failed: {e}"),
                            );
                        });
                        let mut s = self.status.write().await;
                        s.errors += 1;
                    }
                }
            }

            page += 1;
        }
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn sha1_hex(bytes: &[u8]) -> String {
    let mut h = Sha1::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn is_image_ext(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str(),
        "jpg" | "jpeg" | "png" | "gif" | "heic" | "heif" | "webp" | "tiff" | "tif"
            | "raw" | "arw" | "cr2" | "cr3" | "nef" | "orf" | "rw2" | "dng"
    )
}

fn walk_images(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk_images(&path));
            } else if is_image_ext(&path) {
                files.push(path);
            }
        }
    }
    files
}

// ── file watcher ──────────────────────────────────────────────────────────────

/// Spawns a background OS thread that watches all configured upload folders.
/// When a new image file is created or modified, it waits for 2 s of silence
/// (debounce) and then sends a trigger to kick off an immediate sync cycle.
/// Watched paths are refreshed every 30 s to pick up newly added folders.
fn spawn_watcher(app: tauri::AppHandle, trigger_tx: mpsc::Sender<()>) {
    let _ = std::thread::Builder::new()
        .name("file-watcher".into())
        .spawn(move || {
            use notify::Watcher; // bring .watch() / .unwatch() into scope
            let (event_tx, event_rx) =
                std::sync::mpsc::channel::<notify::Result<notify::Event>>();

            let mut watcher = match notify::recommended_watcher(move |res| {
                let _ = event_tx.send(res);
            }) {
                Ok(w) => w,
                Err(e) => {
                    log::error!("file-watcher: failed to create watcher: {e}");
                    return;
                }
            };

            let mut watched: HashSet<PathBuf> = HashSet::new();

            loop {
                // Refresh the set of watched folders from current config.
                let desired = upload_folders_from_config(&app);

                let to_unwatch: Vec<PathBuf> =
                    watched.difference(&desired).cloned().collect();
                let to_watch: Vec<PathBuf> =
                    desired.difference(&watched).cloned().collect();

                for path in &to_unwatch {
                    let _ = watcher.unwatch(path);
                    log::info!("file-watcher: stopped watching {}", path.display());
                }
                for path in &to_watch {
                    match watcher.watch(path, notify::RecursiveMode::Recursive) {
                        Ok(()) => log::info!("file-watcher: watching {}", path.display()),
                        Err(e) => log::warn!("file-watcher: cannot watch {}: {e}", path.display()),
                    }
                }
                watched = desired;

                // Block until an event arrives, or 30 s passes (config refresh).
                let first = match event_rx.recv_timeout(Duration::from_secs(30)) {
                    Ok(Ok(ev)) => ev,
                    Ok(Err(e)) => {
                        log::warn!("file-watcher: notify error: {e}");
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                };

                // Ignore events we don't care about (deletions, metadata, etc.).
                if !is_watcher_event_relevant(&first) {
                    continue;
                }

                // Debounce: wait until 2 s of silence before triggering sync.
                // Resets on every new event so a burst of file copies fires once.
                let mut last_event = std::time::Instant::now();
                loop {
                    let elapsed = last_event.elapsed();
                    if elapsed >= Duration::from_secs(2) {
                        break;
                    }
                    match event_rx.recv_timeout(Duration::from_secs(2) - elapsed) {
                        Ok(_) => last_event = std::time::Instant::now(),
                        Err(_) => break,
                    }
                }

                log::info!("file-watcher: new image files detected, triggering sync");
                let _ = trigger_tx.try_send(());
            }
        });
}

/// Returns true for Create/Modify events that involve at least one image file.
fn is_watcher_event_relevant(event: &notify::Event) -> bool {
    matches!(
        event.kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_)
    ) && event.paths.iter().any(|p| is_image_ext(p))
}

/// Reads all enabled upload folders from the persisted config.
fn upload_folders_from_config(app: &tauri::AppHandle) -> HashSet<PathBuf> {
    app.store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(CONFIG_KEY))
        .and_then(|v| serde_json::from_value::<AppConfig>(v).ok())
        .map(|c| {
            c.profiles
                .into_iter()
                .filter(|p| p.enabled && !p.api_key.is_empty())
                .flat_map(|p| p.upload_folders.into_iter().map(PathBuf::from))
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}
