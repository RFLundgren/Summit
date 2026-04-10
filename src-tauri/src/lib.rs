use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Listener, Manager, WindowEvent,
};
use tauri_plugin_updater::UpdaterExt;

// Holds a pending update between check and download.
pub struct PendingUpdate(pub tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>);

mod api;
mod cloud_files;
mod commands;
pub mod debug_log;
mod settings;
mod sync;

#[tauri::command]
fn show_window(app: tauri::AppHandle, label: String) {
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

// ── Updater helpers ───────────────────────────────────────────────────────────

fn emit_updater_status(
    app: &tauri::AppHandle,
    state: &str,
    version: Option<&str>,
    percent: Option<u32>,
    error: Option<&str>,
) {
    let mut payload = serde_json::json!({ "state": state });
    if let Some(v) = version { payload["version"] = serde_json::json!(v); }
    if let Some(p) = percent  { payload["percent"]  = serde_json::json!(p); }
    if let Some(e) = error    { payload["error"]    = serde_json::json!(e); }
    let _ = app.emit("updater:status", payload);
}

async fn perform_update_check(app: tauri::AppHandle) {
    emit_updater_status(&app, "checking", None, None, None);
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            emit_updater_status(&app, "error", None, None, Some(&e.to_string()));
            return;
        }
    };
    match updater.check().await {
        Err(e) => emit_updater_status(&app, "error", None, None, Some(&e.to_string())),
        Ok(None) => emit_updater_status(&app, "not-available", None, None, None),
        Ok(Some(update)) => {
            let version = update.version.clone();
            *app.state::<PendingUpdate>().0.lock().await = Some(update);
            emit_updater_status(&app, "available", Some(&version), None, None);
        }
    }
}

#[tauri::command]
async fn check_for_updates(app: tauri::AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn(async move { perform_update_check(app).await });
    Ok(())
}

#[tauri::command]
async fn download_update(
    app: tauri::AppHandle,
    pending: tauri::State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = pending.0.lock().await.take().ok_or("No update pending")?;
    let version = update.version.clone();
    let app_c = app.clone();

    tauri::async_runtime::spawn(async move {
        let downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let dl = downloaded.clone();
        let ver = version.clone();
        let app_progress = app_c.clone();

        let result = update
            .download_and_install(
                move |chunk, total| {
                    let so_far = dl.fetch_add(chunk as u64, std::sync::atomic::Ordering::Relaxed)
                        + chunk as u64;
                    let percent = total
                        .map(|t| ((so_far * 100) / t.max(1)) as u32)
                        .unwrap_or(0);
                    emit_updater_status(&app_progress, "downloading", Some(&ver), Some(percent), None);
                },
                || {},
            )
            .await;

        if let Err(e) = result {
            emit_updater_status(&app_c, "error", None, None, Some(&e.to_string()));
        }
        // On success the NSIS installer launches and the app exits automatically.
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Handle installer/uninstaller flags before starting the Tauri runtime.
    // The NSIS uninstaller calls `--unregister-shell` to clean up registry
    // entries before deleting files.  We exit immediately after the operation
    // so no UI is shown.
    let args: Vec<String> = std::env::args().collect();
    dlog!("=== Summit starting, args: {:?}", args);

    // Log the raw contents of settings.json so we know exactly what's on disk.
    {
        let settings_path = {
            let appdata = std::env::var("APPDATA").unwrap_or_default();
            std::path::PathBuf::from(&appdata)
                .join("com.summit.app")
                .join("settings.json")
        };
        dlog!("settings.json path: {:?}", settings_path);
        match std::fs::read_to_string(&settings_path) {
            Ok(raw) => {
                dlog!("settings.json content: {}", raw);
                // Parse and log every profile's download folder + existence.
                if let Ok(root) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(profiles) = root
                        .get("config")
                        .and_then(|c| c.get("profiles"))
                        .and_then(|p| p.as_array())
                    {
                        for (i, profile) in profiles.iter().enumerate() {
                            let name = profile.get("displayName").and_then(|v| v.as_str()).unwrap_or("?");
                            let folder = profile.get("downloadFolder").and_then(|v| v.as_str()).unwrap_or("");
                            let exists = std::path::Path::new(folder).exists();
                            dlog!("  profile[{i}] name={name:?} downloadFolder={folder:?} exists={exists}");
                        }
                    } else {
                        dlog!("  settings.json: no profiles array found");
                    }
                }
            }
            Err(e) => dlog!("settings.json not found or unreadable: {e}"),
        }
    }

    if args.iter().any(|a| a == "--unregister-shell") {
        cloud_files::unregister_shell_extension();
        return;
    }
    // --unregister-path <path>: call CfUnregisterSyncRoot on the given path so
    // the CF filter driver stops managing (and recreating) that folder.
    if let Some(pos) = args.iter().position(|a| a == "--unregister-path") {
        if let Some(path) = args.get(pos + 1) {
            cloud_files::unregister_sync_root_path(path);
        }
        return;
    }

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("app".into()),
                    },
                ))
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .manage(PendingUpdate(tokio::sync::Mutex::new(None)))
        .setup(|app| {
            // Spawn background update check 10 s after startup.
            let app_for_update = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                perform_update_check(app_for_update).await;
            });

            // Build tray menu
            let open_item =
                MenuItem::with_id(app, "open", "Open Dashboard", true, None::<&str>)?;
            let pause_item =
                MenuItem::with_id(app, "pause", "Pause Sync", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit_item =
                MenuItem::with_id(app, "quit", "Quit Summit", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &open_item,
                    &pause_item,
                    &sep,
                    &quit_item,
                ],
            )?;

            // Clone so the event handler can toggle the label at runtime.
            let pause_item_for_handler = pause_item.clone();

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Summit")
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("dashboard") {
                            let _ = w.center();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "pause" => {
                        if let Some(engine) = app.try_state::<std::sync::Arc<crate::sync::SyncEngine>>() {
                            if engine.is_paused() {
                                engine.resume();
                                let _ = pause_item_for_handler.set_text("Pause Sync");
                            } else {
                                engine.pause();
                                let _ = pause_item_for_handler.set_text("Resume Sync");
                            }
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("dashboard") {
                            let _ = w.center();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Clone handle for the tooltip listener (TrayIcon is Arc-backed, cheap clone).
            let tray_for_tooltip = tray.clone();
            // Keep tray bound so it isn't dropped before the clone is made.
            let _ = tray;

            // Update tray tooltip whenever sync phase changes.
            app.listen("sync://status", move |event: tauri::Event| {
                if let Ok(status) =
                    serde_json::from_str::<crate::sync::SyncStatus>(event.payload())
                {
                    let tip = match status.phase.as_str() {
                        "uploading"   => "Summit - Uploading",
                        "downloading" => "Summit - Downloading",
                        "paused"      => "Summit - Paused",
                        "error"       => "Summit - Error",
                        _             => "Summit",
                    };
                    let _ = tray_for_tooltip.set_tooltip(Some(tip));
                }
            });

            // Snapshot which folders exist RIGHT NOW, so we can detect
            // anything created by our own startup code.
            let snapshot_folders = {
                let appdata = std::env::var("APPDATA").unwrap_or_default();
                if let Ok(raw) = std::fs::read_to_string(
                    std::path::Path::new(&appdata)
                        .join("com.summit.app")
                        .join("settings.json"),
                ) {
                    if let Ok(root) = serde_json::from_str::<serde_json::Value>(&raw) {
                        root.get("config")
                            .and_then(|c| c.get("profiles"))
                            .and_then(|p| p.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|p| {
                                        p.get("downloadFolder")
                                            .and_then(|f| f.as_str())
                                            .filter(|f| !f.is_empty())
                                            .map(|f| (f.to_string(), std::path::Path::new(f).exists()))
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    } else { vec![] }
                } else { vec![] }
            };
            dlog!("folder existence BEFORE startup: {:?}", snapshot_folders);

            // Initialise Cloud Files provider (Files On-Demand)
            dlog!("Initialising CloudFilesProvider");
            let cf_provider = std::sync::Arc::new(crate::cloud_files::CloudFilesProvider::new());
            app.manage(cf_provider);
            dlog!("folder existence AFTER CloudFilesProvider: {:?}",
                snapshot_folders.iter().map(|(p, _)| (p.as_str(), std::path::Path::new(p).exists())).collect::<Vec<_>>());

            // Initialise sync engine
            let data_dir = app.path().app_data_dir()?;
            dlog!("app data_dir: {:?}", data_dir);
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("sync.db");
            match crate::sync::SyncEngine::init(app.handle().clone(), db_path) {
                Ok(engine) => {
                    dlog!("SyncEngine initialised OK");
                    dlog!("folder existence AFTER SyncEngine init: {:?}",
                        snapshot_folders.iter().map(|(p, _)| (p.as_str(), std::path::Path::new(p).exists())).collect::<Vec<_>>());
                    app.manage(engine);
                }
                Err(e) => {
                    dlog!("SyncEngine init FAILED: {e}");
                    log::error!("Sync engine init failed: {e}");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            show_window,
            check_for_updates,
            download_update,
            commands::connection_commands::test_connection,
            commands::auth_commands::login_account,
            commands::auth_commands::get_config,
            commands::auth_commands::set_active_profile,
            commands::auth_commands::delete_profile,
            commands::auth_commands::save_app_config,
            commands::auth_commands::get_active_profile_status,
            commands::auth_commands::update_profile,
            commands::auth_commands::update_sync_folders,
            commands::auth_commands::discover_servers,
            commands::sync_commands::get_sync_status,
            commands::sync_commands::trigger_sync,
            commands::sync_commands::pause_sync,
            commands::sync_commands::resume_sync,
            commands::sync_commands::get_recent_activity,
            commands::sync_commands::check_cloud_file_state,
            commands::sync_commands::check_shell_registration,
            commands::sync_commands::check_wrt_registration,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
