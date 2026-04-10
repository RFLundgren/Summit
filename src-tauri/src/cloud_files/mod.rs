//! Windows Cloud Files API (CF API) provider.
//!
//! Implements Files On-Demand via the Win32 Cloud Filter API — the same
//! mechanism OneDrive uses.  Placeholders appear in Explorer; files are
//! downloaded from Immich the first time a user opens them.
//!
//! Key flow
//! ────────
//! 1. `ensure_connected()` — registers the sync root with Windows and starts
//!    the callback loop for a profile's download folder.
//! 2. `create_placeholders_batch()` — called by the sync engine each cycle to
//!    materialise new Immich assets as zero-byte placeholder files.
//! 3. On-demand hydration — when the user opens a placeholder, Windows fires
//!    `on_fetch_data`; we send a `HydrationRequest` through an unbounded
//!    channel, and an async worker downloads the real bytes and calls
//!    `CfExecute(TRANSFER_DATA)`.
//! 4. Dehydration — "Free up space" in the Explorer context menu fires
//!    `on_notify_dehydrate`; we acknowledge it and the file reverts to a
//!    placeholder.

#![allow(non_snake_case)]

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::NTSTATUS;
use windows::Win32::Storage::CloudFilters::{
    CfCreatePlaceholders, CfDisconnectSyncRoot, CfRegisterSyncRoot, CfUnregisterSyncRoot,
    CF_HARDLINK_POLICY_NONE, CF_HYDRATION_POLICY, CF_HYDRATION_POLICY_FULL,
    CF_HYDRATION_POLICY_MODIFIER_AUTO_DEHYDRATION_ALLOWED,
    CF_INSYNC_POLICY_NONE, CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT,
    CF_POPULATION_POLICY, CF_POPULATION_POLICY_ALWAYS_FULL,
    CF_POPULATION_POLICY_MODIFIER_NONE, CF_REGISTER_FLAG_UPDATE,
    CF_SYNC_POLICIES, CF_SYNC_REGISTRATION,
    CF_CALLBACK_PARAMETERS, CF_CALLBACK_TYPE, CF_CALLBACK_TYPE_CANCEL_FETCH_DATA,
    CF_CALLBACK_TYPE_FETCH_DATA, CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE,
    CF_CONNECT_FLAG_NONE, CF_CONNECTION_KEY, CF_CREATE_FLAG_NONE, CF_FS_METADATA,
    CF_OPERATION_PARAMETERS, CF_OPERATION_PARAMETERS_0, CF_OPERATION_PARAMETERS_0_1,
    CF_OPERATION_PARAMETERS_0_6, CF_OPERATION_TYPE, CF_OPERATION_TYPE_ACK_DEHYDRATE,
    CF_OPERATION_TYPE_TRANSFER_DATA, CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC,
    CF_PLACEHOLDER_CREATE_INFO,
};
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, FILE_ATTRIBUTE_NORMAL, FILE_BASIC_INFO};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED};
use windows::Storage::Provider::{
    StorageProviderHardlinkPolicy, StorageProviderHydrationPolicy,
    StorageProviderHydrationPolicyModifier, StorageProviderInSyncPolicy,
    StorageProviderPopulationPolicy, StorageProviderSyncRootInfo,
    StorageProviderSyncRootManager,
};
use windows::Storage::{IStorageFolder, StorageFolder};
use windows::core::{Interface, GUID, HRESULT, HSTRING, PCWSTR};

use crate::api::{types::AssetResponse, ImmichClient};

// ── Manual CF API types ───────────────────────────────────────────────────────
//
// CF_CALLBACK_INFO, CF_CALLBACK_REGISTRATION, CF_OPERATION_INFO, and
// CfConnectSyncRoot / CfExecute are unavailable in windows-rs 0.58 because
// they transitively depend on CORRELATION_VECTOR (whose feature flag cannot be
// resolved in this crate configuration).  We reproduce the structs manually
// with #[repr(C)] so the memory layout matches the Win32 ABI exactly.
//
// Padding is inserted automatically by #[repr(C)] (same rules as C structs).
// Opaque pointer fields (PCCORRELATION_VECTOR, *CF_PROCESS_INFO, *CF_SYNC_STATUS)
// are represented as *const c_void — we never read or write them.

/// Mirrors Win32 `CF_CALLBACK_INFO` (cfapi.h).
#[repr(C)]
struct CfCallbackInfo {
    StructSize: u32,                  // offset   0
    ConnectionKey: CF_CONNECTION_KEY, // offset   8  (4-byte pad before; align 8)
    CallbackContext: *mut c_void,     // offset  16
    VolumeDosName: PCWSTR,            // offset  24
    VolumeGuidName: PCWSTR,           // offset  32
    VolumeSerialNumber: u32,          // offset  40
    SyncRootFileId: i64,              // offset  48  (4-byte pad before; align 8)
    SyncRootIdentity: *const c_void,  // offset  56
    SyncRootIdentityLength: u32,      // offset  64
    FileId: i64,                      // offset  72  (4-byte pad before; align 8)
    FileSize: i64,                    // offset  80
    FileIdentity: *const c_void,      // offset  88
    FileIdentityLength: u32,          // offset  96
    NormalizedPath: PCWSTR,           // offset 104  (4-byte pad before; align 8)
    TransferKey: i64,                 // offset 112  (CF_TRANSFER_KEY.Internal)
    PriorityHint: u8,                 // offset 120
    CorrelationVector: *const c_void, // offset 128  (7-byte pad before; align 8)
    ProcessInfo: *const c_void,       // offset 136
    RequestKey: i64,                  // offset 144  (CF_REQUEST_KEY.Internal)
}

/// Mirrors Win32 `CF_OPERATION_INFO` (cfapi.h).
#[repr(C)]
struct CfOperationInfo {
    StructSize: u32,                  // offset  0
    Type: CF_OPERATION_TYPE,          // offset  4
    ConnectionKey: CF_CONNECTION_KEY, // offset  8  (align 8, no pad needed)
    TransferKey: i64,                 // offset 16  (CF_TRANSFER_KEY.Internal)
    CorrelationVector: *const c_void, // offset 24
    SyncStatus: *const c_void,        // offset 32
    RequestKey: i64,                  // offset 40  (CF_REQUEST_KEY.Internal)
}

/// Callback function pointer type matching Win32 `CF_CALLBACK`.
type CfCallbackFn = Option<
    unsafe extern "system" fn(*const CfCallbackInfo, *const CF_CALLBACK_PARAMETERS),
>;

/// Mirrors Win32 `CF_CALLBACK_REGISTRATION` (cfapi.h).
#[repr(C)]
struct CfCallbackRegistration {
    Type: CF_CALLBACK_TYPE, // offset 0  (4 bytes)
    Callback: CfCallbackFn, // offset 8  (4-byte pad before; fn ptr align 8)
}

// ── FFI declarations (cldapi.dll) ─────────────────────────────────────────────

#[link(name = "cldapi")]
extern "system" {
    fn CfConnectSyncRoot(
        SyncRootPath: *const u16,
        CallbackTable: *const CfCallbackRegistration,
        CallbackContext: *const c_void,
        ConnectFlags: i32,
        ConnectionKey: *mut CF_CONNECTION_KEY,
    ) -> HRESULT;

    fn CfExecute(
        OpInfo: *const CfOperationInfo,
        OpParams: *mut CF_OPERATION_PARAMETERS,
    ) -> HRESULT;
}

// ── Fixed provider GUID for Summit ───────────────────────────────────
const PROVIDER_ID: GUID = GUID {
    data1: 0xB3C4_A1D2,
    data2: 0x5F6E,
    data3: 0x7890,
    data4: [0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x90],
};

// ── Callback context ─────────────────────────────────────────────────────────
// Heap-allocated; a raw pointer to this is stored in Windows' callback table.
// Must NOT be moved after the pointer is handed to CfConnectSyncRoot.
struct CloudContext {
    client: Arc<ImmichClient>,
    hydration_tx: tokio::sync::mpsc::UnboundedSender<HydrationRequest>,
}

// ── Hydration request ────────────────────────────────────────────────────────
struct HydrationRequest {
    asset_id: String,
    transfer_key: i64,
    request_key: i64,
    connection_key: i64,
    client: Arc<ImmichClient>,
    /// Full normalized path of the placeholder file (for post-hydration diagnostics).
    file_path: String,
}

// ── Per-profile connection ───────────────────────────────────────────────────
struct ProfileConnection {
    connection_key: CF_CONNECTION_KEY,
    download_folder: PathBuf,
    /// Keeps the context allocation alive (raw pointer lives in Windows).
    _context: Box<CloudContext>,
    /// Keeps the callback table alive (Windows holds a raw pointer into it).
    _callbacks: Box<[CfCallbackRegistration; 4]>,
    /// Hydration async worker.
    _hydration_task: tauri::async_runtime::JoinHandle<()>,
    /// Current Immich client.
    client: Arc<ImmichClient>,
}

// ── CloudFilesProvider ───────────────────────────────────────────────────────

pub struct CloudFilesProvider {
    connections: Mutex<HashMap<String, ProfileConnection>>,
}

// CF_CONNECTION_KEY wraps i64; all our types are Send + Sync.
unsafe impl Send for CloudFilesProvider {}
unsafe impl Sync for CloudFilesProvider {}

impl CloudFilesProvider {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }

    /// Ensure a sync root is registered and connected for the given profile.
    /// Idempotent: if already connected to the same folder, returns immediately.
    pub fn ensure_connected(
        &self,
        profile_id: &str,
        download_folder: &PathBuf,
        client: Arc<ImmichClient>,
    ) -> Result<(), String> {
        let existing_folder = {
            let conns = self.connections.lock().unwrap();
            conns.get(profile_id).map(|c| c.download_folder.clone())
        };

        let expected_root = download_folder.clone();
        match existing_folder {
            Some(ref f) if *f == expected_root => {
                return Ok(());
            }
            Some(_) => {
                self.disconnect_profile(profile_id);
            }
            None => {}
        }

        self.connect_profile(profile_id, download_folder, client)
    }

    fn connect_profile(
        &self,
        profile_id: &str,
        download_folder: &PathBuf,
        client: Arc<ImmichClient>,
    ) -> Result<(), String> {
        // Cloud Files API requires a local NTFS/ReFS volume — network drives
        // (mapped or UNC) are not supported by the cldflt.sys filter driver.
        // GetDriveTypeW needs the root of the volume (e.g. "Z:\").
        let root = {
            let s = download_folder.to_string_lossy();
            // Extract drive root (e.g. "C:\") or UNC root (e.g. "\\server\share\").
            // For a UNC path the drive type will correctly return DRIVE_REMOTE.
            let mut root_str = if s.starts_with("\\\\") {
                // UNC: \\server\share\ — take up to and including the share component
                let after_slashes = &s[2..];
                let end = after_slashes.find('\\')
                    .and_then(|i| after_slashes[i+1..].find('\\').map(|j| 2 + i + 1 + j + 1))
                    .unwrap_or(s.len());
                s[..end].to_string() + "\\"
            } else {
                // Local or mapped: take "X:\"
                s.chars().take(3).collect::<String>()
            };
            root_str.push('\0');
            root_str.encode_utf16().collect::<Vec<u16>>()
        };
        let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
        const DRIVE_REMOTE: u32 = 4;
        if drive_type == DRIVE_REMOTE {
            return Err(
                "Files On-Demand requires a local NTFS drive. \
                 Mapped network drives and UNC paths are not supported by \
                 the Windows Cloud Files API."
                    .to_string(),
            );
        }

        // StorageProviderSyncRootManager refuses to register on Windows shell
        // known folders (Pictures, Documents, etc.) directly — the user must
        // select a dedicated subfolder themselves.
        let sync_root = download_folder.clone();
        if !sync_root.exists() {
            return Err(format!(
                "Sync folder \"{}\" does not exist. Please choose a folder in Settings.",
                sync_root.display()
            ));
        }

        let path_wide = HSTRING::from(sync_root.to_string_lossy().as_ref());

        // Unregister parent folder (may have been registered by older code).
        let parent_wide = HSTRING::from(download_folder.to_string_lossy().as_ref());
        unsafe { let _ = CfUnregisterSyncRoot(PCWSTR(parent_wide.as_ptr())); }

        // Resolve AppId and SID before any registration.
        let app_id = get_sync_provider_app_id();
        let sid = get_current_user_sid().unwrap_or_default();

        // Do NOT unregister the sync root before re-registering.  Each
        // CfUnregisterSyncRoot + CfRegisterSyncRoot cycle makes Windows assign
        // a fresh cloud-file reparse-tag slot (0x9000001A, 0x9000101A, …).
        // Existing placeholder files carry the tag from the PREVIOUS slot, so
        // they become orphaned from the new registration and Explorer can no
        // longer match them to our provider — meaning "Free up space" never
        // appears.  CfRegisterSyncRoot is idempotent: if the path is already
        // registered with the same identity it reuses the existing slot.

        // Step 2 — register with the CF filter driver.
        let identity_bytes = profile_id.as_bytes().to_vec();
        let provider_name_h = HSTRING::from("Summit");
        let provider_version_h = HSTRING::from("1.0");
        let registration = CF_SYNC_REGISTRATION {
            StructSize: std::mem::size_of::<CF_SYNC_REGISTRATION>() as u32,
            ProviderName: PCWSTR(provider_name_h.as_ptr()),
            ProviderVersion: PCWSTR(provider_version_h.as_ptr()),
            SyncRootIdentity: identity_bytes.as_ptr() as *const _,
            SyncRootIdentityLength: identity_bytes.len() as u32,
            FileIdentity: std::ptr::null(),
            FileIdentityLength: 0,
            ProviderId: PROVIDER_ID,
        };
        let policies = CF_SYNC_POLICIES {
            StructSize: std::mem::size_of::<CF_SYNC_POLICIES>() as u32,
            Hydration: CF_HYDRATION_POLICY {
                Primary: CF_HYDRATION_POLICY_FULL,
                Modifier: CF_HYDRATION_POLICY_MODIFIER_AUTO_DEHYDRATION_ALLOWED,
            },
            Population: CF_POPULATION_POLICY {
                Primary: CF_POPULATION_POLICY_ALWAYS_FULL,
                Modifier: CF_POPULATION_POLICY_MODIFIER_NONE,
            },
            InSync: CF_INSYNC_POLICY_NONE,
            HardLink: CF_HARDLINK_POLICY_NONE,
            PlaceholderManagement: CF_PLACEHOLDER_MANAGEMENT_POLICY_DEFAULT,
        };
        // CF_REGISTER_FLAG_UPDATE: if the sync root is already registered at
        // this path, update it in place (preserving the cloud-file reparse-tag
        // slot so existing placeholder files stay associated with this provider).
        // If it is not yet registered, this creates a fresh registration.
        unsafe {
            CfRegisterSyncRoot(
                PCWSTR(path_wide.as_ptr()),
                &registration,
                &policies,
                CF_REGISTER_FLAG_UPDATE,
            )
            .map_err(|e| format!("CfRegisterSyncRoot failed: {e}"))?;
        }
        log::info!("cloud_files: CfRegisterSyncRoot ok");

        // Step 3 — write SyncRootManager registry entries.
        if let Err(e) = register_shell_provider(&app_id, &sid, profile_id, &sync_root) {
            log::warn!("cloud_files: registry shell registration failed (non-fatal): {e}");
        }

        // Step 4 — call StorageProviderSyncRootManager::Register on a fire-and-forget
        // STA thread.  CfRegisterSyncRoot above creates a fresh CF registration;
        // Register() called after a fresh (WRT-less) registration should now succeed.
        {
            let sync_root_for_wrt = sync_root.clone();
            let app_id_for_wrt = app_id.clone();
            let sid_for_wrt = sid.clone();
            let profile_id_for_wrt = profile_id.to_string();
            std::thread::Builder::new()
                .name("spr-register".into())
                .spawn(move || {
                    // MTA is required: .get() on IAsyncOperation blocks using OS
                    // sync primitives on MTA threads.  On an STA thread without a
                    // running message loop .get() deadlocks, causing silent WRT failure.
                    unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); }

                    // Unconditionally clean up ALL stale HKLM SyncRootManager entries
                    // for this app prefix before attempting Register().  This MUST run
                    // outside the result closure so that a GetFolderFromPathAsync failure
                    // (e.g. sync folder not yet created) does not silently skip the
                    // cleanup, leaving old GUID-suffix entries that add duplicate Status
                    // columns in Explorer on every reinstall.
                    {
                        use windows::Win32::System::Registry::{
                            RegCloseKey, RegEnumKeyExW, RegOpenKeyExW,
                            HKEY, HKEY_LOCAL_MACHINE, KEY_ENUMERATE_SUB_KEYS,
                        };
                        let parent_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager";
                        let parent_wide: Vec<u16> = parent_path.encode_utf16().chain(std::iter::once(0)).collect();
                        let mut hk = HKEY::default();
                        if unsafe {
                            RegOpenKeyExW(
                                HKEY_LOCAL_MACHINE,
                                windows::core::PCWSTR(parent_wide.as_ptr()),
                                0, KEY_ENUMERATE_SUB_KEYS, &mut hk,
                            )
                        }.is_ok() {
                            let prefix = format!("{}!", app_id_for_wrt);
                            let mut entries: Vec<String> = Vec::new();
                            let mut i = 0u32;
                            loop {
                                let mut buf = vec![0u16; 512];
                                let mut len = 512u32;
                                match unsafe {
                                    RegEnumKeyExW(
                                        hk, i,
                                        windows::core::PWSTR(buf.as_mut_ptr()), &mut len,
                                        None, windows::core::PWSTR::null(), None, None,
                                    )
                                }.ok() {
                                    Ok(()) => {
                                        let name = String::from_utf16_lossy(&buf[..len as usize]);
                                        if name.starts_with(&prefix) { entries.push(name); }
                                        i += 1;
                                    }
                                    Err(_) => break,
                                }
                            }
                            unsafe { let _ = RegCloseKey(hk); }
                            for name in &entries {
                                let h = HSTRING::from(name.as_str());
                                match StorageProviderSyncRootManager::Unregister(&h) {
                                    Ok(()) => log::info!("cloud_files: WRT Unregister HKLM stale: {}", name),
                                    Err(e) => log::warn!("cloud_files: WRT Unregister HKLM stale {} failed: {e}", name),
                                }
                            }
                        }
                    }

                    let result = (|| -> Result<(), String> {
                        let path_wide = HSTRING::from(sync_root_for_wrt.to_string_lossy().as_ref());
                        let folder = StorageFolder::GetFolderFromPathAsync(&path_wide)
                            .map_err(|e| format!("GetFolderFromPathAsync: {e}"))?
                            .get()
                            .map_err(|e| format!("await StorageFolder: {e}"))?;
                        let i_folder: IStorageFolder = Interface::cast(&folder)
                            .map_err(|e| format!("IStorageFolder cast: {e}"))?;
                        // Static "Summit" suffix — must match the Id declared in
                        // the MSIX manifest's windows.cloudFiles extension.  Windows uses
                        // that manifest entry to wire up automatic shell integration
                        // ("Free up space", overlay icons, etc.) for the sync root.
                        let id = HSTRING::from(format!("{}!{}!Summit", app_id_for_wrt, sid_for_wrt));

                        let info = StorageProviderSyncRootInfo::new()
                            .map_err(|e| format!("new: {e}"))?;
                        info.SetId(&id).map_err(|e| format!("SetId: {e}"))?;
                        info.SetPath(&i_folder).map_err(|e| format!("SetPath: {e}"))?;
                        info.SetDisplayNameResource(&HSTRING::from("Summit"))
                            .map_err(|e| format!("SetDisplayNameResource: {e}"))?;
                        // IconResource must be set — Windows.FileExplorer.Common.dll crashes
                        // with an access violation if it is left null.
                        info.SetIconResource(&HSTRING::from("%SystemRoot%\\system32\\imageres.dll,-1023"))
                            .map_err(|e| format!("SetIconResource: {e}"))?;
                        info.SetHydrationPolicy(StorageProviderHydrationPolicy::Full)
                            .map_err(|e| format!("SetHydrationPolicy: {e}"))?;
                        info.SetHydrationPolicyModifier(
                            StorageProviderHydrationPolicyModifier::AutoDehydrationAllowed,
                        )
                        .map_err(|e| format!("SetHydrationPolicyModifier: {e}"))?;
                        // AlwaysFull: we proactively create all placeholders via
                        // create_placeholders_batch(); Windows never needs to fire
                        // FETCH_PLACEHOLDERS to ask us to enumerate a directory.
                        // Using Full instead would cause Windows to fire FETCH_PLACEHOLDERS
                        // on every directory navigation and hang Explorer waiting for our
                        // CfExecute(ACK_GET_FILE_LIST) response (which we never send).
                        info.SetPopulationPolicy(StorageProviderPopulationPolicy::AlwaysFull)
                            .map_err(|e| format!("SetPopulationPolicy: {e}"))?;
                        info.SetInSyncPolicy(StorageProviderInSyncPolicy::Default)
                            .map_err(|e| format!("SetInSyncPolicy: {e}"))?;
                        info.SetHardlinkPolicy(StorageProviderHardlinkPolicy::None)
                            .map_err(|e| format!("SetHardlinkPolicy: {e}"))?;
                        info.SetShowSiblingsAsGroup(false)
                            .map_err(|e| format!("SetShowSiblingsAsGroup: {e}"))?;
                        info.SetVersion(&HSTRING::from("1.0"))
                            .map_err(|e| format!("SetVersion: {e}"))?;
                        // Unregister the existing entry first so that Register()
                        // writes a fresh HKLM entry with correct Flags (including
                        // dehydration-support bits).  Re-registering over a stale
                        // entry written with wrong policies leaves the old Flags
                        // in place and "Free up space" never appears.
                        // This only affects the WRT/shell layer — the CF API
                        // reparse-tag slot is controlled by CfRegisterSyncRoot
                        // (with CF_REGISTER_FLAG_UPDATE) and is unaffected.
                        match StorageProviderSyncRootManager::Unregister(&id) {
                            Ok(()) => log::info!("cloud_files: WRT Unregister ok for {}", id),
                            Err(e) => log::info!("cloud_files: WRT Unregister (non-fatal, may not exist): {e}"),
                        }
                        StorageProviderSyncRootManager::Register(&info)
                            .map_err(|e| format!("Register: {e}"))
                    })();
                    unsafe { CoUninitialize(); }
                    match result {
                        Ok(()) => {
                            log::info!("cloud_files: StorageProviderSyncRootManager::Register succeeded");

                            // Write StorageProviderStatusUISourceFactory to the HKLM entry
                            // WRT just created.  WRT does not set this value automatically;
                            // without it Explorer shows a blank Status column.
                            {
                                use windows::Win32::System::Registry::{
                                    RegCloseKey, RegOpenKeyExW, RegSetValueExW,
                                    HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_SZ,
                                };
                                let hklm_path = format!(
                                    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager\{}!{}!Summit",
                                    app_id_for_wrt, sid_for_wrt
                                );
                                let path_w: Vec<u16> = hklm_path.encode_utf16().chain(std::iter::once(0)).collect();
                                let mut hk = HKEY::default();
                                let rc = unsafe {
                                    RegOpenKeyExW(HKEY_LOCAL_MACHINE, PCWSTR(path_w.as_ptr()), 0, KEY_SET_VALUE, &mut hk)
                                };
                                if rc.is_ok() {
                                    let name_w: Vec<u16> = "StorageProviderStatusUISourceFactory"
                                        .encode_utf16().chain(std::iter::once(0)).collect();
                                    let val = "{5A3B2C1D-4E5F-6070-8192-A3B4C5D6E7F8}";
                                    let val_bytes: Vec<u8> = val.encode_utf16()
                                        .chain(std::iter::once(0u16))
                                        .flat_map(|c| c.to_le_bytes())
                                        .collect();
                                    let rw = unsafe { RegSetValueExW(hk, PCWSTR(name_w.as_ptr()), 0, REG_SZ, Some(&val_bytes)) };
                                    if rw.is_ok() {
                                        log::info!("cloud_files: wrote StorageProviderStatusUISourceFactory to HKLM");
                                    } else {
                                        log::warn!("cloud_files: could not write StatusUIFactory to HKLM: {rw:?}");
                                    }
                                    unsafe { let _ = RegCloseKey(hk); }
                                } else {
                                    log::warn!("cloud_files: could not open HKLM SyncRootManager to write StatusUIFactory: {rc:?}");
                                }
                            }

                            // Delete the HKCU SyncRootManager key — HKLM now supersedes it.
                            // Having both HKCU and HKLM entries for the same sync root ID
                            // causes Explorer to hang when building its namespace.
                            {
                                use windows::Win32::System::Registry::{RegDeleteTreeW, HKEY_USERS};
                                let hkcu_subkey = format!(
                                    r"{}\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager\{}!{}!Summit",
                                    sid_for_wrt, app_id_for_wrt, sid_for_wrt
                                );
                                let subkey_w: Vec<u16> = hkcu_subkey.encode_utf16().chain(std::iter::once(0)).collect();
                                let rd = unsafe { RegDeleteTreeW(HKEY_USERS, PCWSTR(subkey_w.as_ptr())) };
                                if rd.is_ok() {
                                    log::info!("cloud_files: deleted HKCU SyncRootManager (HKLM supersedes)");
                                } else {
                                    log::warn!("cloud_files: could not delete HKCU SyncRootManager: {rd:?}");
                                }
                            }
                        }
                        Err(e) => {
                            if app_id_for_wrt == "Summit" {
                                log::warn!("cloud_files: Register failed (no package identity): {e}");
                            } else {
                                log::error!("cloud_files: Register failed with PFN={app_id_for_wrt}: {e}");
                            }
                        }
                    }
                })
                .ok();
        }

        let (hydration_tx, hydration_rx) =
            tokio::sync::mpsc::unbounded_channel::<HydrationRequest>();

        // Build context on heap — address is stable, passed as raw ptr to Windows.
        let ctx = Box::new(CloudContext {
            client: Arc::clone(&client),
            hydration_tx,
        });
        let ctx_raw: *const CloudContext = ctx.as_ref();

        // Build callback table on heap — Windows holds a pointer into it.
        let callbacks = Box::new([
            CfCallbackRegistration {
                Type: CF_CALLBACK_TYPE_FETCH_DATA,
                Callback: Some(on_fetch_data),
            },
            CfCallbackRegistration {
                Type: CF_CALLBACK_TYPE_CANCEL_FETCH_DATA,
                Callback: Some(on_cancel_fetch_data),
            },
            CfCallbackRegistration {
                Type: CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE,
                Callback: Some(on_notify_dehydrate),
            },
            // Null terminator: CF_CALLBACK_TYPE_NONE = 0xFFFFFFFF = -1 as i32.
            CfCallbackRegistration {
                Type: CF_CALLBACK_TYPE(-1),
                Callback: None,
            },
        ]);
        let callbacks_raw: *const CfCallbackRegistration = callbacks.as_ptr();

        let mut connection_key = CF_CONNECTION_KEY::default();
        unsafe {
            CfConnectSyncRoot(
                path_wide.as_ptr(),
                callbacks_raw,
                ctx_raw as *const c_void,
                CF_CONNECT_FLAG_NONE.0,
                &mut connection_key,
            )
            .ok()
            .map_err(|e| format!("CfConnectSyncRoot failed: {e}"))?;
        }

        let task = tauri::async_runtime::spawn(hydration_worker(hydration_rx));

        let mut conns = self.connections.lock().unwrap();
        conns.insert(
            profile_id.to_string(),
            ProfileConnection {
                connection_key,
                download_folder: sync_root.clone(),
                _context: ctx,
                _callbacks: callbacks,
                _hydration_task: task,
                client,
            },
        );

        Ok(())
    }

    pub fn disconnect_profile(&self, profile_id: &str) {
        if let Ok(mut conns) = self.connections.lock() {
            if let Some(conn) = conns.remove(profile_id) {
                unsafe {
                    let _ = CfDisconnectSyncRoot(conn.connection_key);
                }
                conn._hydration_task.abort();

                // Unregister the sync root from the CF filter driver so the old
                // folder does not auto-recreate itself if the user deletes it.
                // CfDisconnectSyncRoot above stops callbacks; CfUnregisterSyncRoot
                // removes the reparse point that makes cldflt.sys recreate the folder.
                let path_h = HSTRING::from(conn.download_folder.to_string_lossy().as_ref());
                unsafe {
                    let _ = CfUnregisterSyncRoot(PCWSTR(path_h.as_ptr()));
                }
                log::info!(
                    "disconnect_profile: CfUnregisterSyncRoot called for {}",
                    conn.download_folder.display()
                );
            }
        }
    }

    pub fn is_connected(&self, profile_id: &str) -> bool {
        self.connections
            .lock()
            .map(|c| c.contains_key(profile_id))
            .unwrap_or(false)
    }

    /// Create placeholder files for the given assets in the profile's sync root.
    pub fn create_placeholders_batch(
        &self,
        profile_id: &str,
        assets: &[AssetResponse],
    ) -> Result<u32, String> {
        let download_folder = {
            let conns = self.connections.lock().unwrap();
            conns
                .get(profile_id)
                .map(|c| c.download_folder.clone())
                .ok_or_else(|| "Profile not connected".to_string())?
        };

        let new_assets: Vec<&AssetResponse> = assets
            .iter()
            .filter(|a| !download_folder.join(&a.original_file_name).exists())
            .collect();

        if new_assets.is_empty() {
            return Ok(0);
        }

        let path_wide = HSTRING::from(download_folder.to_string_lossy().as_ref());

        // Build stable heap allocations for wide strings and identity bytes.
        let wide_names: Vec<HSTRING> = new_assets
            .iter()
            .map(|a| HSTRING::from(a.original_file_name.as_str()))
            .collect();
        let identities: Vec<Vec<u8>> = new_assets
            .iter()
            .map(|a| a.id.as_bytes().to_vec())
            .collect();

        let mut infos: Vec<CF_PLACEHOLDER_CREATE_INFO> = new_assets
            .iter()
            .enumerate()
            .map(|(i, asset)| {
                let file_size = asset
                    .exif_info
                    .as_ref()
                    .and_then(|e| e.file_size_in_byte)
                    .unwrap_or(0);
                let created = rfc3339_to_filetime(&asset.file_created_at);
                let modified = rfc3339_to_filetime(&asset.file_modified_at);

                CF_PLACEHOLDER_CREATE_INFO {
                    RelativeFileName: PCWSTR(wide_names[i].as_ptr()),
                    FsMetadata: CF_FS_METADATA {
                        BasicInfo: FILE_BASIC_INFO {
                            CreationTime: created,
                            LastAccessTime: modified,
                            LastWriteTime: modified,
                            ChangeTime: modified,
                            FileAttributes: FILE_ATTRIBUTE_NORMAL.0,
                        },
                        FileSize: file_size,
                    },
                    FileIdentity: identities[i].as_ptr() as *const _,
                    FileIdentityLength: identities[i].len() as u32,
                    Flags: CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC,
                    Result: windows::core::HRESULT(0),
                    CreateUsn: 0,
                }
            })
            .collect();

        let mut entries_processed: u32 = 0;
        unsafe {
            CfCreatePlaceholders(
                PCWSTR(path_wide.as_ptr()),
                &mut infos,
                CF_CREATE_FLAG_NONE,
                Some(&mut entries_processed),
            )
            .map_err(|e| format!("CfCreatePlaceholders failed: {e}"))?;
        }

        Ok(entries_processed)
    }
}

// ── Hydration worker ─────────────────────────────────────────────────────────

async fn hydration_worker(mut rx: tokio::sync::mpsc::UnboundedReceiver<HydrationRequest>) {
    while let Some(req) = rx.recv().await {
        tauri::async_runtime::spawn(hydrate_file(req));
    }
}

/// Log every relevant Win32 attribute bit for a file so we can see exactly
/// what state the placeholder is in before and after hydration.
#[cfg(windows)]
fn log_file_attrs(label: &str, path: &str) {
    use std::os::windows::fs::MetadataExt;
    const ATTR_READONLY:    u32 = 0x0000_0001;
    const ATTR_HIDDEN:      u32 = 0x0000_0002;
    const ATTR_ARCHIVE:     u32 = 0x0000_0020;
    const ATTR_NORMAL:      u32 = 0x0000_0080;
    const ATTR_REPARSE:     u32 = 0x0000_0400;
    const ATTR_COMPRESSED:  u32 = 0x0000_0800;
    const ATTR_OFFLINE:     u32 = 0x0000_1000;
    const ATTR_NOT_CONTENT_INDEXED: u32 = 0x0000_2000;
    const ATTR_ENCRYPTED:   u32 = 0x0000_4000;
    const ATTR_PINNED:      u32 = 0x0008_0000;
    const ATTR_UNPINNED:    u32 = 0x0010_0000;
    const ATTR_RECALL:      u32 = 0x0040_0000;
    const ATTR_STRICTLY_SEQUENTIAL: u32 = 0x0200_0000;
    const ATTR_NO_SCRUB:    u32 = 0x0002_0000;

    match std::fs::metadata(path) {
        Err(e) => log::info!("[attrs:{}] cannot read metadata for {:?}: {}", label, path, e),
        Ok(m) => {
            let a = m.file_attributes();
            let mut flags = Vec::new();
            if a & ATTR_READONLY    != 0 { flags.push("READONLY"); }
            if a & ATTR_HIDDEN      != 0 { flags.push("HIDDEN"); }
            if a & ATTR_ARCHIVE     != 0 { flags.push("ARCHIVE"); }
            if a & ATTR_NORMAL      != 0 { flags.push("NORMAL"); }
            if a & ATTR_REPARSE     != 0 { flags.push("REPARSE_POINT"); }
            if a & ATTR_COMPRESSED  != 0 { flags.push("COMPRESSED"); }
            if a & ATTR_OFFLINE     != 0 { flags.push("OFFLINE"); }
            if a & ATTR_NOT_CONTENT_INDEXED != 0 { flags.push("NOT_CONTENT_INDEXED"); }
            if a & ATTR_ENCRYPTED   != 0 { flags.push("ENCRYPTED"); }
            if a & ATTR_PINNED      != 0 { flags.push("PINNED"); }
            if a & ATTR_UNPINNED    != 0 { flags.push("UNPINNED"); }
            if a & ATTR_RECALL      != 0 { flags.push("RECALL_ON_DATA_ACCESS"); }
            if a & ATTR_STRICTLY_SEQUENTIAL != 0 { flags.push("STRICTLY_SEQUENTIAL"); }
            if a & ATTR_NO_SCRUB    != 0 { flags.push("NO_SCRUB_DATA"); }
            log::info!(
                "[attrs:{}] {:?}  raw=0x{:08x}  size={}  flags=[{}]",
                label, path, a, m.len(), flags.join(", ")
            );
        }
    }
}

async fn hydrate_file(req: HydrationRequest) {
    log::info!("hydrate_file: START asset={} path={:?}", req.asset_id, req.file_path);
    #[cfg(windows)]
    if !req.file_path.is_empty() {
        log_file_attrs("before-hydration", &req.file_path);
    }

    let url = format!(
        "{}/api/assets/{}/original",
        req.client.base_url, req.asset_id
    );

    let bytes = match req
        .client
        .client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                log::error!("Hydration stream failed for {}: {e}", req.asset_id);
                fail_hydration(&req);
                return;
            }
        },
        Err(e) => {
            log::error!("Hydration download failed for {}: {e}", req.asset_id);
            fail_hydration(&req);
            return;
        }
    };

    let len = bytes.len() as i64;

    unsafe {
        let op_info = CfOperationInfo {
            StructSize: std::mem::size_of::<CfOperationInfo>() as u32,
            Type: CF_OPERATION_TYPE_TRANSFER_DATA,
            ConnectionKey: CF_CONNECTION_KEY(req.connection_key),
            TransferKey: req.transfer_key,
            CorrelationVector: std::ptr::null(),
            SyncStatus: std::ptr::null(),
            RequestKey: req.request_key,
        };

        let mut op_params: CF_OPERATION_PARAMETERS = std::mem::zeroed();
        op_params.ParamSize = std::mem::size_of::<CF_OPERATION_PARAMETERS>() as u32;
        op_params.Anonymous = CF_OPERATION_PARAMETERS_0 {
            TransferData: CF_OPERATION_PARAMETERS_0_6 {
                Flags: windows::Win32::Storage::CloudFilters::CF_OPERATION_TRANSFER_DATA_FLAGS(0),
                CompletionStatus: NTSTATUS(0),
                Buffer: bytes.as_ptr() as *const _,
                Offset: 0,
                Length: len,
            },
        };

        match CfExecute(&op_info, &mut op_params).ok() {
            Err(e) => log::error!("CfExecute(TransferData) failed for {}: {e}", req.asset_id),
            Ok(()) => {
                log::info!("hydrate_file: CfExecute(TransferData) OK for {}", req.asset_id);
                #[cfg(windows)]
                if !req.file_path.is_empty() {
                    log_file_attrs("after-hydration", &req.file_path);
                }
            }
        }
    }
    log::info!("hydrate_file: DONE asset={}", req.asset_id);
}

fn fail_hydration(req: &HydrationRequest) {
    unsafe {
        let op_info = CfOperationInfo {
            StructSize: std::mem::size_of::<CfOperationInfo>() as u32,
            Type: CF_OPERATION_TYPE_TRANSFER_DATA,
            ConnectionKey: CF_CONNECTION_KEY(req.connection_key),
            TransferKey: req.transfer_key,
            CorrelationVector: std::ptr::null(),
            SyncStatus: std::ptr::null(),
            RequestKey: req.request_key,
        };

        let mut op_params: CF_OPERATION_PARAMETERS = std::mem::zeroed();
        op_params.ParamSize = std::mem::size_of::<CF_OPERATION_PARAMETERS>() as u32;
        op_params.Anonymous = CF_OPERATION_PARAMETERS_0 {
            TransferData: CF_OPERATION_PARAMETERS_0_6 {
                Flags: windows::Win32::Storage::CloudFilters::CF_OPERATION_TRANSFER_DATA_FLAGS(0),
                CompletionStatus: NTSTATUS(0xC000_0001u32 as i32), // STATUS_UNSUCCESSFUL
                Buffer: std::ptr::null(),
                Offset: 0,
                Length: 0,
            },
        };

        let _ = CfExecute(&op_info, &mut op_params);
    }
}

// ── Callbacks (extern "system" — called from Windows thread pool) ─────────────

unsafe extern "system" fn on_fetch_data(
    info: *const CfCallbackInfo,
    _params: *const CF_CALLBACK_PARAMETERS,
) {
    if info.is_null() {
        return;
    }
    let info = &*info;

    if info.CallbackContext.is_null() {
        return;
    }
    let ctx = &*(info.CallbackContext as *const CloudContext);

    if info.FileIdentityLength == 0 || info.FileIdentity.is_null() {
        log::warn!("on_fetch_data: placeholder has no file identity");
        return;
    }

    let asset_id = {
        let bytes = std::slice::from_raw_parts(
            info.FileIdentity as *const u8,
            info.FileIdentityLength as usize,
        );
        String::from_utf8_lossy(bytes).to_string()
    };

    let file_path = if !info.NormalizedPath.is_null() {
        let s = info.NormalizedPath.to_string().unwrap_or_default();
        s
    } else {
        String::new()
    };

    let req = HydrationRequest {
        asset_id,
        transfer_key: info.TransferKey,
        request_key: info.RequestKey,
        connection_key: info.ConnectionKey.0,
        client: Arc::clone(&ctx.client),
        file_path,
    };

    let _ = ctx.hydration_tx.send(req);
}

unsafe extern "system" fn on_cancel_fetch_data(
    _info: *const CfCallbackInfo,
    _params: *const CF_CALLBACK_PARAMETERS,
) {
    log::debug!("Cloud Files: cancel fetch requested (ignored)");
}

unsafe extern "system" fn on_notify_dehydrate(
    info: *const CfCallbackInfo,
    _params: *const CF_CALLBACK_PARAMETERS,
) {
    if info.is_null() {
        return;
    }
    let info = &*info;

    let op_info = CfOperationInfo {
        StructSize: std::mem::size_of::<CfOperationInfo>() as u32,
        Type: CF_OPERATION_TYPE_ACK_DEHYDRATE,
        ConnectionKey: CF_CONNECTION_KEY(info.ConnectionKey.0),
        TransferKey: info.TransferKey,
        CorrelationVector: std::ptr::null(),
        SyncStatus: std::ptr::null(),
        RequestKey: info.RequestKey,
    };

    let mut op_params: CF_OPERATION_PARAMETERS = std::mem::zeroed();
    op_params.ParamSize = std::mem::size_of::<CF_OPERATION_PARAMETERS>() as u32;
    op_params.Anonymous = CF_OPERATION_PARAMETERS_0 {
        AckDehydrate: CF_OPERATION_PARAMETERS_0_1 {
            Flags: windows::Win32::Storage::CloudFilters::CF_OPERATION_ACK_DEHYDRATE_FLAGS(0),
            CompletionStatus: NTSTATUS(0),
            FileIdentity: std::ptr::null(),
            FileIdentityLength: 0,
        },
    };

    let _ = CfExecute(&op_info, &mut op_params);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Write HKCU\...\SyncRootManager entries so the Windows Shell shows
/// "Free up space" for hydrated placeholder files.
///
/// `app_id` is the Package Family Name when running inside a sparse/MSIX
/// package, or "Summit" when unpackaged.
fn register_shell_provider(
    app_id: &str,
    sid: &str,
    profile_id: &str,
    sync_root: &std::path::Path,
) -> Result<(), String> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyW, RegDeleteTreeW, RegEnumKeyExW, RegOpenKeyExW,
        RegSetValueExW, HKEY, HKEY_USERS, KEY_ENUMERATE_SUB_KEYS, REG_SZ,
    };
    use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_DWORD};

    // Static "Summit" suffix — must match the MSIX manifest's
    // windows.cloudFiles SyncRoot Id so Windows wires up shell integration.
    let key_id = format!("{}!{}!Summit", app_id, sid);

    // Clean up ALL stale SyncRootManager entries for this app — not just the
    // one matching the current profile GUID.  Each reinstall generates a new
    // profile GUID; old entries accumulate and each one adds a duplicate
    // "Status" column in Explorer.  Enumerating and deleting all keys that
    // start with "{app_id}!" before writing the fresh entry ensures Explorer
    // always sees exactly one Status column.
    //
    // Write via HKEY_USERS\{SID} instead of HKEY_CURRENT_USER: MSIX
    // package identity redirects HKCU writes to a private per-package hive
    // invisible to Explorer.  HKEY_USERS\{SID} bypasses that virtualisation.
    {
        let syncroot_parent = format!(
            r"{}\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager",
            sid
        );
        let parent_wide: Vec<u16> = syncroot_parent.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hk_parent = HKEY::default();
        if unsafe {
            RegOpenKeyExW(
                HKEY_USERS,
                windows::core::PCWSTR(parent_wide.as_ptr()),
                0,
                KEY_ENUMERATE_SUB_KEYS,
                &mut hk_parent,
            )
        }.is_ok() {
            let prefix = format!("{}!", app_id);
            let mut stale: Vec<String> = Vec::new();
            let mut idx = 0u32;
            loop {
                let mut name_buf = vec![0u16; 512];
                let mut name_len = 512u32;
                match unsafe {
                    RegEnumKeyExW(
                        hk_parent, idx,
                        windows::core::PWSTR(name_buf.as_mut_ptr()),
                        &mut name_len,
                        None,
                        windows::core::PWSTR::null(),
                        None,
                        None,
                    )
                }.ok() {
                    Ok(()) => {
                        let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                        if name.starts_with(&prefix) {
                            stale.push(name);
                        }
                        idx += 1;
                    }
                    Err(_) => break,
                }
            }
            unsafe { let _ = RegCloseKey(hk_parent); }
            for name in &stale {
                let full = format!(
                    r"{}\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager\{}",
                    sid, name
                );
                let full_wide: Vec<u16> = full.encode_utf16().chain(std::iter::once(0)).collect();
                unsafe { let _ = RegDeleteTreeW(HKEY_USERS, windows::core::PCWSTR(full_wide.as_ptr())); }
                log::info!("cloud_files: deleted stale SyncRootManager entry: {}", name);
            }
        }
    }
    // Null-terminated UTF-16 string as Vec<u16>.
    let to_wide = |s: &str| -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    };
    // Same, but returned as raw bytes (REG_SZ includes null terminator).
    let to_wide_bytes = |s: &str| -> Vec<u8> {
        let w: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe { std::slice::from_raw_parts(w.as_ptr() as *const u8, w.len() * 2).to_vec() }
    };
    // Write a named REG_SZ value into an open key.
    let set_sz = |hk: HKEY, name: &str, value: &str| -> Result<(), String> {
        let name_w = to_wide(name);
        let data = to_wide_bytes(value);
        unsafe { RegSetValueExW(hk, PCWSTR(name_w.as_ptr()), 0, REG_SZ, Some(&data)) }
            .ok()
            .map_err(|e| format!("RegSetValueExW({name}): {e}"))
    };

    // CLSID of the companion COM shell-extension DLL that provides
    // "Free up space" and "Always keep on this device" verbs.
    // Must match CLSID_IMMICH_HANDLER in shell-ext/src/lib.rs.
    let handler_clsid = "{AA7F4C3E-2B48-4C9A-9E2F-1D8B5C4A7E6F}";

    // --- Register the companion COM shell-extension DLL in HKCU\SOFTWARE\Classes ---
    // The installer always drops summit_shell_ext.dll next to the exe.  We never
    // register that fixed name directly: Explorer locks the DLL it has loaded, so
    // the installer would be unable to overwrite it on update.
    //
    // Instead we copy it to summit_shell_ext_{version}.dll once per version and
    // register that versioned name.  The fixed-name file is never held open by
    // Explorer, so the installer can always replace it.  On startup after an update
    // the new versioned file is created and the registry is updated; Explorer picks
    // up the new DLL the next time it restarts.
    //
    // Old versioned files are deleted opportunistically (they may be locked on first
    // run after update, which is fine — the delete simply fails silently).
    let app_version = env!("CARGO_PKG_VERSION");
    let dll_path = if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        let base     = exe_dir.join("summit_shell_ext.dll");
        let versioned = exe_dir.join(format!("summit_shell_ext_{}.dll", app_version));

        // Copy base → versioned if the versioned file doesn't exist yet.
        if base.exists() && !versioned.exists() {
            match std::fs::copy(&base, &versioned) {
                Ok(_)  => log::info!("cloud_files: deployed {}", versioned.display()),
                Err(e) => log::warn!("cloud_files: failed to copy DLL to versioned name: {e}"),
            }
        }

        // Remove any old versioned DLLs that are no longer needed.
        if let Ok(entries) = std::fs::read_dir(&exe_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("summit_shell_ext_")
                    && name.ends_with(".dll")
                    && name != format!("summit_shell_ext_{}.dll", app_version).as_str()
                {
                    let _ = std::fs::remove_file(entry.path()); // silent failure is fine if locked
                }
            }
        }

        if versioned.exists() {
            versioned.to_string_lossy().into_owned()
        } else if base.exists() {
            // Fallback: versioned copy failed, register the base name.
            base.to_string_lossy().into_owned()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    if !dll_path.is_empty() {
        let inproc_key_path = format!(
            r"{}\SOFTWARE\Classes\CLSID\{}\InprocServer32",
            sid, handler_clsid
        );
        let mut hkey3 = HKEY::default();
        let inproc_wide = to_wide(&inproc_key_path);
        let r3 = unsafe {
            RegCreateKeyW(HKEY_USERS, windows::core::PCWSTR(inproc_wide.as_ptr()), &mut hkey3)
                .ok()
                .map_err(|e| format!("RegCreateKeyW(InprocServer32): {e}"))
        }
        .and_then(|_| set_sz(hkey3, "", &dll_path))
        .and_then(|_| set_sz(hkey3, "ThreadingModel", "Apartment"));
        unsafe { let _ = RegCloseKey(hkey3); }
        if let Err(e) = r3 {
            log::warn!("cloud_files: CLSID registration failed: {e}");
        } else {
            log::info!("cloud_files: registered CLSID {handler_clsid} -> {dll_path}");
        }

        // Register CLSID_STATUS_UI_FACTORY (IStorageProviderStatusUISourceFactory).
        // Explorer instantiates this to obtain Status column icons.  Same DLL.
        let status_factory_clsid = "{5A3B2C1D-4E5F-6070-8192-A3B4C5D6E7F8}";
        let sf_inproc_path = format!(
            r"{}\SOFTWARE\Classes\CLSID\{}\InprocServer32",
            sid, status_factory_clsid
        );
        let mut hkey_sf = HKEY::default();
        let sf_inproc_wide = to_wide(&sf_inproc_path);
        let r_sf = unsafe {
            RegCreateKeyW(HKEY_USERS, windows::core::PCWSTR(sf_inproc_wide.as_ptr()), &mut hkey_sf)
                .ok()
                .map_err(|e| format!("RegCreateKeyW(StatusUIFactory InprocServer32): {e}"))
        }
        .and_then(|_| set_sz(hkey_sf, "", &dll_path))
        .and_then(|_| set_sz(hkey_sf, "ThreadingModel", "Apartment"));
        unsafe { let _ = RegCloseKey(hkey_sf); }
        if let Err(e) = r_sf {
            log::warn!("cloud_files: StatusUIFactory CLSID registration failed: {e}");
        } else {
            log::info!("cloud_files: registered CLSID {status_factory_clsid} -> {dll_path}");
        }

        // Register the shell extension as a context menu handler for all files.
        // This is what actually causes Explorer to invoke our IContextMenu DLL.
        // Must be under SOFTWARE\Classes\*\shellex\ContextMenuHandlers\{name}.
        let cmh_key_path = format!(
            r"{}\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\Summit",
            sid
        );
        let mut hkey4 = HKEY::default();
        let cmh_wide = to_wide(&cmh_key_path);
        let r4 = unsafe {
            RegCreateKeyW(HKEY_USERS, windows::core::PCWSTR(cmh_wide.as_ptr()), &mut hkey4)
                .ok()
                .map_err(|e| format!("RegCreateKeyW(ContextMenuHandlers): {e}"))
        }
        .and_then(|_| set_sz(hkey4, "", handler_clsid));
        unsafe { let _ = RegCloseKey(hkey4); }
        if let Err(e) = r4 {
            log::warn!("cloud_files: ContextMenuHandlers registration failed: {e}");
        } else {
            log::info!("cloud_files: registered ContextMenuHandlers\\Summit");
        }
    }

    // Notify the shell to re-read cloud provider registrations.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_DWORD, None, None); }

    log::info!("cloud_files: shell provider registered ({})", key_id);
    Ok(())
}

/// Call `CfUnregisterSyncRoot` on a specific path so the Cloud Files filter
/// driver stops managing (and auto-recreating) that folder.
pub fn unregister_sync_root_path(path: &str) {
    use windows::core::HSTRING;
    // Ensure the folder exists — CfUnregisterSyncRoot needs the path present.
    let _ = std::fs::create_dir_all(path);
    let path_h = HSTRING::from(path);
    let result = unsafe { CfUnregisterSyncRoot(windows::core::PCWSTR(path_h.as_ptr())) };
    let msg = if result.is_ok() {
        format!("CfUnregisterSyncRoot succeeded for: {path}\n")
    } else {
        format!("CfUnregisterSyncRoot failed for {path}: {result:?}\n")
    };
    let log = std::env::temp_dir().join("immich_unreg.txt");
    let _ = std::fs::write(&log, &msg);
    // Also attempt to delete the folder now that it is unregistered.
    let _ = std::fs::remove_dir_all(path);
}

/// Remove the COM shell-extension registry entries written by
/// `register_shell_provider`.  Called by the NSIS uninstaller before files
/// are deleted, so Explorer stops trying to load a DLL that no longer exists.
///
/// Errors are logged but not propagated — the uninstaller must proceed even
/// if cleanup partially fails.
pub fn unregister_shell_extension() {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteTreeW, RegEnumKeyExW, RegOpenKeyExW,
        HKEY, HKEY_LOCAL_MACHINE, HKEY_USERS, KEY_ENUMERATE_SUB_KEYS,
    };
    use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_DWORD};

    // ── Step 0: CfUnregisterSyncRoot for every known download folder ───────────
    // The CF filter driver (cldflt.sys) keeps sync root registrations alive even
    // after uninstall.  If the folder is deleted the driver recreates it the next
    // time any process accesses the parent.  We MUST call CfUnregisterSyncRoot
    // before touching the registry so the driver stops managing those paths.
    {
        let settings_path = {
            // %APPDATA%\com.summit.app\settings.json
            let appdata = std::env::var("APPDATA").unwrap_or_default();
            std::path::PathBuf::from(appdata)
                .join("com.summit.app")
                .join("settings.json")
        };
        if let Ok(raw) = std::fs::read_to_string(&settings_path) {
            if let Ok(root) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(profiles) = root
                    .get("config")
                    .and_then(|c| c.get("profiles"))
                    .and_then(|p| p.as_array())
                {
                    for profile in profiles {
                        if let Some(folder) = profile
                            .get("downloadFolder")
                            .and_then(|f| f.as_str())
                            .filter(|f| !f.is_empty())
                        {
                            // Ensure the directory exists — CfUnregisterSyncRoot
                            // fails if the path is absent.
                            let _ = std::fs::create_dir_all(folder);
                            let path_h = windows::core::HSTRING::from(folder);
                            let result = unsafe {
                                CfUnregisterSyncRoot(windows::core::PCWSTR(path_h.as_ptr()))
                            };
                            log::info!(
                                "unregister_shell_extension: CfUnregisterSyncRoot({folder}) -> {result:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    let sid = match get_current_user_sid() {
        Ok(s)  => s,
        Err(e) => { log::warn!("unregister_shell_extension: cannot get SID: {e}"); return; }
    };
    let app_id = get_sync_provider_app_id();

    let to_wide = |s: &str| -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    };

    // Helper: enumerate and delete all SyncRootManager subkeys starting with
    // the app's prefix under a given open registry key.
    let delete_sync_root_entries = |hk_root: HKEY, parent_path: &str| {
        let parent_wide = to_wide(parent_path);
        let mut hk_parent = HKEY::default();
        if unsafe {
            RegOpenKeyExW(
                hk_root,
                windows::core::PCWSTR(parent_wide.as_ptr()),
                0,
                KEY_ENUMERATE_SUB_KEYS,
                &mut hk_parent,
            )
        }.is_ok() {
            let prefix = format!("{}!", app_id);
            let mut stale: Vec<String> = Vec::new();
            let mut idx = 0u32;
            loop {
                let mut name_buf = vec![0u16; 512];
                let mut name_len = 512u32;
                match unsafe {
                    RegEnumKeyExW(
                        hk_parent, idx,
                        windows::core::PWSTR(name_buf.as_mut_ptr()),
                        &mut name_len,
                        None,
                        windows::core::PWSTR::null(),
                        None,
                        None,
                    )
                }.ok() {
                    Ok(()) => {
                        let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                        if name.starts_with(&prefix) { stale.push(name); }
                        idx += 1;
                    }
                    Err(_) => break,
                }
            }
            unsafe { let _ = RegCloseKey(hk_parent); }
            for name in &stale {
                let full = format!(r"{}\{}", parent_path, name);
                let w = to_wide(&full);
                unsafe { let _ = RegDeleteTreeW(hk_root, windows::core::PCWSTR(w.as_ptr())); }
            }
            !stale.is_empty()
        } else {
            false
        }
    };

    // Delete HKCU SyncRootManager entries (our manual registration).
    let hkcu_syncroot_path = format!(
        r"{}\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager",
        sid
    );
    delete_sync_root_entries(HKEY_USERS, &hkcu_syncroot_path);

    // Delete HKLM SyncRootManager entries (written by StorageProviderSyncRootManager::Register).
    // The uninstaller runs elevated so this succeeds without a UAC prompt.
    let hklm_syncroot_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager";
    delete_sync_root_entries(HKEY_LOCAL_MACHINE, hklm_syncroot_path);

    // Delete all four HKCU COM CLSID registrations.
    for clsid in &[
        "{AA7F4C3E-2B48-4C9A-9E2F-1D8B5C4A7E6F}", // context menu handler
        "{5A3B2C1D-4E5F-6070-8192-A3B4C5D6E7F8}", // status UI factory
        "{3F8A9C1B-2D4E-5F60-A7B8-C9D0E1F23456}", // free-up IExplorerCommand
        "{4A9B0D2C-3E5F-6071-B8C9-D0E1F2345678}", // always-keep IExplorerCommand
    ] {
        let key = format!(r"{}\SOFTWARE\Classes\CLSID\{}", sid, clsid);
        let w = to_wide(&key);
        unsafe { let _ = RegDeleteTreeW(HKEY_USERS, windows::core::PCWSTR(w.as_ptr())); }
    }

    // Delete HKCU\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\Summit.
    let cmh_key = format!(
        r"{}\SOFTWARE\Classes\*\shellex\ContextMenuHandlers\Summit",
        sid
    );
    let w = to_wide(&cmh_key);
    unsafe { let _ = RegDeleteTreeW(HKEY_USERS, windows::core::PCWSTR(w.as_ptr())); }

    // Delete HKCU Desktop\NameSpace entries written by StorageProviderSyncRootManager::Register.
    // Windows creates one GUID-keyed subkey per sync root whose default value is the sync root ID.
    // Enumerate them all and remove any whose value starts with our app ID prefix.
    {
        use windows::Win32::System::Registry::{
            RegQueryValueExW, KEY_QUERY_VALUE, REG_SZ,
        };
        let ns_path = format!(
            r"{}\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace",
            sid
        );
        let ns_wide = to_wide(&ns_path);
        let mut hk_ns = HKEY::default();
        if unsafe {
            RegOpenKeyExW(
                HKEY_USERS,
                windows::core::PCWSTR(ns_wide.as_ptr()),
                0,
                KEY_ENUMERATE_SUB_KEYS | KEY_QUERY_VALUE,
                &mut hk_ns,
            )
        }.is_ok() {
            let prefix = format!("{}!", app_id);
            let mut to_delete: Vec<String> = Vec::new();
            let mut idx = 0u32;
            loop {
                let mut name_buf = vec![0u16; 256];
                let mut name_len = 256u32;
                match unsafe {
                    RegEnumKeyExW(
                        hk_ns, idx,
                        windows::core::PWSTR(name_buf.as_mut_ptr()),
                        &mut name_len,
                        None,
                        windows::core::PWSTR::null(),
                        None,
                        None,
                    )
                }.ok() {
                    Ok(()) => {
                        let subkey_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                        // Open the subkey and read its default value.
                        let sub_path = format!(r"{}\{}", ns_path, subkey_name);
                        let sub_wide = to_wide(&sub_path);
                        let mut hk_sub = HKEY::default();
                        if unsafe {
                            RegOpenKeyExW(
                                HKEY_USERS,
                                windows::core::PCWSTR(sub_wide.as_ptr()),
                                0,
                                KEY_QUERY_VALUE,
                                &mut hk_sub,
                            )
                        }.is_ok() {
                            let mut val_buf = vec![0u8; 512];
                            let mut val_len = 512u32;
                            let mut val_type = REG_SZ;
                            if unsafe {
                                RegQueryValueExW(
                                    hk_sub,
                                    windows::core::PCWSTR::null(),
                                    None,
                                    Some(&mut val_type),
                                    Some(val_buf.as_mut_ptr()),
                                    Some(&mut val_len),
                                )
                            }.is_ok() && val_type == REG_SZ {
                                let val_u16: Vec<u16> = val_buf[..val_len as usize]
                                    .chunks_exact(2)
                                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                                    .collect();
                                let val_str = String::from_utf16_lossy(&val_u16);
                                if val_str.starts_with(&prefix) {
                                    to_delete.push(subkey_name);
                                }
                            }
                            unsafe { let _ = RegCloseKey(hk_sub); }
                        }
                        idx += 1;
                    }
                    Err(_) => break,
                }
            }
            unsafe { let _ = RegCloseKey(hk_ns); }
            for name in &to_delete {
                let full = format!(
                    r"{}\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace\{}",
                    sid, name
                );
                let w = to_wide(&full);
                unsafe { let _ = RegDeleteTreeW(HKEY_USERS, windows::core::PCWSTR(w.as_ptr())); }
            }
        }
    }

    // Tell Explorer to refresh its shell extension cache.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_DWORD, None, None); }

    log::info!("cloud_files: shell extension unregistered");
}

/// Return the AppId to stamp into StorageProviderSyncRootManager IDs and
/// the SyncRootManager registry key name.
///
/// When the process has package identity (sparse or full MSIX package) this
/// returns the Package Family Name, e.g. `"Summit_8wekyb3d8bbwe"`.
/// Windows validates this against the installed-package list, which is what
/// makes `StorageProviderSyncRootManager::Register` and
/// `GetSyncRootInformationForFolder` work.
///
/// When unpackaged (plain `tauri dev` without the sparse package) it returns
/// `"Summit"` — CF API operations still work but WinRT shell
/// integration is unavailable.
fn get_sync_provider_app_id() -> String {
    // GetCurrentPackageFamilyName lives in kernel32.dll (always linked).
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentPackageFamilyName(
                packagefamilynamelength: *mut u32,
                packagefamilyname: *mut u16,
            ) -> u32;
        }
        const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
        // APPMODEL_ERROR_NO_PACKAGE = 15700 means we have no package identity.

        unsafe {
            let mut len: u32 = 0;
            // First call: get required buffer length.
            let rc = GetCurrentPackageFamilyName(&mut len, std::ptr::null_mut());
            if rc == ERROR_INSUFFICIENT_BUFFER && len > 0 {
                let mut buf = vec![0u16; len as usize];
                let rc2 = GetCurrentPackageFamilyName(&mut len, buf.as_mut_ptr());
                if rc2 == 0 && len > 0 {
                    let pfn = String::from_utf16_lossy(
                        &buf[..(len.saturating_sub(1)) as usize],
                    );
                    log::info!("cloud_files: package identity detected, PFN = {pfn}");
                    return pfn;
                }
            }
        }

        // Process identity not granted (sparse package ExternalLocation association
        // doesn't always propagate to dev builds). Fall back to querying the
        // installed package by name via PowerShell — this gives us the PFN so we
        // can write the registry key with a valid prefix even without process
        // identity. The shell only needs a valid PFN in the key name; it does NOT
        // require the writing process to hold that identity.
        if let Ok(output) = std::process::Command::new("powershell")
            .args([
                "-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command",
                "(Get-AppxPackage -Name 'Summit' -ErrorAction SilentlyContinue).PackageFamilyName",
            ])
            .output()
        {
            let pfn = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !pfn.is_empty() {
                log::info!("cloud_files: Summit package found via query, PFN = {pfn}");
                return pfn;
            }
        }

        log::info!("cloud_files: no sparse package found — shell integration unavailable");
    }
    "Summit".to_string()
}

/// Return the current user's SID string (e.g. `S-1-5-21-...-1001`).
fn get_current_user_sid() -> Result<String, String> {
    let output = std::process::Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .map_err(|e| format!("whoami failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output: "DOMAIN\username","S-1-5-21-..." — SID is the last CSV field.
    for field in stdout.split(',').rev() {
        let s = field.trim().trim_matches('"').trim();
        if s.starts_with("S-1-") {
            return Ok(s.to_string());
        }
    }
    Err(format!("Could not parse SID from whoami output: {stdout:?}"))
}

/// Convert an RFC 3339 timestamp to Windows FILETIME (100-ns intervals since
/// 1601-01-01).
fn rfc3339_to_filetime(s: &str) -> i64 {
    let unix_secs = chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp())
        .unwrap_or(0);
    const EPOCH_DIFF_SECS: i64 = 11_644_473_600;
    (unix_secs + EPOCH_DIFF_SECS) * 10_000_000
}
