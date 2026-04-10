use std::path::Path;
use std::sync::Arc;
use tauri::State;
use crate::sync::{db::ActivityEntry, SyncEngine, SyncStatus};

#[tauri::command]
pub async fn get_sync_status(engine: State<'_, Arc<SyncEngine>>) -> Result<SyncStatus, String> {
    Ok(engine.status.read().await.clone())
}

#[tauri::command]
pub async fn trigger_sync(engine: State<'_, Arc<SyncEngine>>) -> Result<(), String> {
    engine.trigger().await;
    Ok(())
}

#[tauri::command]
pub fn pause_sync(engine: State<'_, Arc<SyncEngine>>) -> Result<(), String> {
    engine.pause();
    Ok(())
}

#[tauri::command]
pub fn resume_sync(engine: State<'_, Arc<SyncEngine>>) -> Result<(), String> {
    engine.resume();
    Ok(())
}

/// Diagnostic: returns the cloud-file state of up to 20 files in `folder`.
/// Tells you whether each file is a dehydrated placeholder, a hydrated cloud
/// file, or a plain local file (which will never show "Free up space").
#[tauri::command]
pub fn check_cloud_file_state(folder: String) -> String {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        // Windows file-attribute bit constants
        const ATTR_REPARSE: u32 = 0x0000_0400; // cloud file (has CF reparse point)
        const ATTR_OFFLINE: u32 = 0x0000_1000; // content not locally available
        const ATTR_RECALL: u32 = 0x0040_0000; // RECALL_ON_DATA_ACCESS — dehydrated placeholder
        const ATTR_PINNED: u32 = 0x0008_0000; // always keep locally (no "Free up space")
        const ATTR_UNPINNED: u32 = 0x0010_0000; // prefer online-only

        let dir = Path::new(&folder);
        let mut lines = Vec::new();
        lines.push(format!("Folder: {}", folder));

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => return format!("Cannot read folder: {e}"),
        };

        let mut hydrated = Vec::new();
        let mut total = 0u32;
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() { continue; }
            total += 1;

            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
            match std::fs::metadata(&p) {
                Err(_) => {}
                Ok(meta) => {
                    let a = meta.file_attributes();
                    let is_hydrated_cloud =
                        a & ATTR_REPARSE != 0 && a & ATTR_RECALL == 0 && a & ATTR_OFFLINE == 0;
                    if is_hydrated_cloud {
                        let pin = if a & ATTR_PINNED != 0 { " [PINNED]" }
                                  else if a & ATTR_UNPINNED != 0 { " [unpinned]" }
                                  else { "" };
                        hydrated.push(format!("  {name}{pin}  (attrs=0x{a:08x})"));
                    }
                }
            }
        }

        lines.push(format!("Scanned {} files — {} hydrated cloud file(s):", total, hydrated.len()));
        if hydrated.is_empty() {
            lines.push("  (none — no files have been hydrated via the Cloud Files API yet)".into());
        } else {
            lines.extend(hydrated);
        }

        lines.join("\n")
    }
    #[cfg(not(windows))]
    format!("Only supported on Windows")
}

/// Diagnostic: calls StorageProviderSyncRootManager::GetSyncRootInformationForFolder
/// on the given path. Returns what Windows WinRT layer knows about the registration.
#[tauri::command]
pub fn check_wrt_registration(folder: String) -> String {
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let folder_clone = folder.clone();

        let handle = std::thread::Builder::new()
            .name("wrt-diag".into())
            .spawn(move || {
                use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
                use windows::Storage::Provider::StorageProviderSyncRootManager;
                use windows::Storage::{IStorageFolder, StorageFolder};
                use windows::core::{Interface, HSTRING};

                unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }

                let output = (|| -> String {
                    let path_h = HSTRING::from(folder_clone.as_str());
                    let folder = match StorageFolder::GetFolderFromPathAsync(&path_h)
                        .and_then(|op| op.get())
                    {
                        Ok(f) => f,
                        Err(e) => return format!("GetFolderFromPathAsync({folder_clone}): {e}"),
                    };
                    let i_folder: IStorageFolder = match Interface::cast(&folder) {
                        Ok(f) => f,
                        Err(e) => return format!("IStorageFolder cast: {e}"),
                    };
                    match StorageProviderSyncRootManager::GetSyncRootInformationForFolder(&i_folder) {
                        Err(e) => format!("GetSyncRootInformationForFolder FAILED: {e}\n\
                            → WinRT layer cannot find a provider for this folder.\n\
                            → Registry entries may be missing or in wrong format."),
                        Ok(info) => {
                            let id = info.Id().unwrap_or_default();
                            let disp = info.DisplayNameResource().unwrap_or_default();
                            let policy = info.HydrationPolicy()
                                .map(|p| format!("{p:?}"))
                                .unwrap_or_else(|_| "?".into());
                            format!("GetSyncRootInformationForFolder SUCCEEDED\nId = {id}\nDisplayNameResource = {disp}\nHydrationPolicy = {policy}\n\u{2192} WinRT layer recognises this folder as a cloud sync root.")
                        }
                    }
                })();

                unsafe { CoUninitialize(); }
                let _ = tx.send(output);
            });

        match handle {
            Err(e) => return format!("Failed to spawn diagnostic thread: {e}"),
            Ok(h) => { let _ = h.join(); }
        }

        rx.recv().unwrap_or_else(|_| "No result from diagnostic thread".to_string())
    }
    #[cfg(not(windows))]
    "Only supported on Windows".to_string()
}

/// Diagnostic: dumps the SyncRootManager registry entries for Summit.
/// Returns a multi-line string describing what's registered (or any errors).
#[tauri::command]
pub fn check_shell_registration() -> String {
    #[cfg(windows)]
    {
        use std::process::Command;

        // Get user SID via whoami
        let sid = match Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
        {
            Err(e) => return format!("whoami failed: {e}"),
            Ok(o) => {
                let s = String::from_utf8_lossy(&o.stdout);
                let found = s.split(',').rev()
                    .map(|f| f.trim().trim_matches('"').trim().to_string())
                    .find(|f| f.starts_with("S-1-"));
                match found {
                    Some(sid) => sid,
                    None => return format!("Could not parse SID from: {s:?}"),
                }
            }
        };

        // Use reg.exe to query all Summit keys (covers any profile_id suffix
        // and both unpackaged "Summit!{SID}!..." and packaged
        // "Summit_{pfn}!{SID}!..." key name formats).
        let parent = r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager";

        let mut lines = vec![format!("SID: {sid}"), format!("Looking for Summit keys containing SID: {sid}")];

        // List all keys under SyncRootManager then filter for ours
        let list = Command::new("reg")
            .args(["query", parent])
            .output();

        let found_keys: Vec<String> = match list {
            Err(e) => { lines.push(format!("reg query failed: {e}")); vec![] }
            Ok(o) => {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter(|l| l.contains(&sid) && l.contains("Summit"))
                    .map(|l| l.trim().to_string())
                    .collect()
            }
        };

        if found_keys.is_empty() {
            lines.push("(no Summit SyncRootManager keys found — registration not written yet, or sync profile not yet connected)".to_string());
        } else {
            for key in &found_keys {
                lines.push(format!("\nKey: {key}"));
                let full_key = key.replace("HKEY_CURRENT_USER", "HKCU");
                match Command::new("reg").args(["query", &full_key, "/s"]).output() {
                    Err(e) => lines.push(format!("  query failed: {e}")),
                    Ok(o) => lines.push(String::from_utf8_lossy(&o.stdout).to_string()),
                }
            }
        }

        lines.join("\n")
    }
    #[cfg(not(windows))]
    "Only supported on Windows".to_string()
}

#[tauri::command]
pub fn get_recent_activity(
    engine: State<'_, Arc<SyncEngine>>,
    profile_id: String,
    limit: Option<i64>,
) -> Result<Vec<ActivityEntry>, String> {
    let db = engine.db.lock().map_err(|e| e.to_string())?;
    crate::sync::db::get_recent_activity(&db, &profile_id, limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}
