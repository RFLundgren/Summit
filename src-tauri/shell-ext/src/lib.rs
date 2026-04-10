//! Summit cloud-file shell extension.
//!
//! Implements IShellExtInit + IContextMenu (classic context menu) and
//! IExplorerCommand (Windows 11 modern context menu) for files in an
//! Summit sync root.
//!
//! Also implements IStorageProviderStatusUISourceFactory so Explorer can show
//! cloud/checkmark icons in the Status column for our sync root.
//!
//! CLSIDs:
//!   CLSID_IMMICH_HANDLER:      {AA7F4C3E-2B48-4C9A-9E2F-1D8B5C4A7E6F}  — IContextMenu
//!   CLSID_FREE_UP_CMD:         {3F8A9C1B-2D4E-5F60-A7B8-C9D0E1F23456}  — IExplorerCommand
//!   CLSID_ALWAYS_KEEP_CMD:     {4A9B0D2C-3E5F-6071-B8C9-D0E1F2345678}  — IExplorerCommand
//!   CLSID_STATUS_UI_FACTORY:   {5A3B2C1D-4E5F-6070-8192-A3B4C5D6E7F8}  — IStorageProviderStatusUISourceFactory

#![allow(non_snake_case, clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::sync::Mutex;

use windows::Win32::Foundation::{
    BOOL, CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, CloseHandle, E_INVALIDARG,
    E_NOINTERFACE, E_NOTIMPL, E_POINTER, HANDLE, S_FALSE, S_OK,
};
use windows::Win32::Storage::CloudFilters::{
    CF_DEHYDRATE_FLAG_NONE, CF_PIN_STATE, CF_SET_PIN_FLAG_NONE, CfDehydratePlaceholder,
    CfSetPinState,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_ATTRIBUTES, OPEN_EXISTING,
};
use windows::Win32::System::Com::{
    CoTaskMemAlloc, CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl, IDataObject,
    DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL,
};
use windows::Win32::System::Ole::{CF_HDROP, ReleaseStgMedium};
use windows::Win32::System::Registry::HKEY;
use windows::Win32::UI::Shell::{
    DragQueryFileW, HDROP, IContextMenu, IContextMenu_Impl,
    IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl, IShellExtInit,
    IShellExtInit_Impl, IShellItemArray, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::WindowsAndMessaging::{InsertMenuW, HMENU, MF_BYPOSITION, MF_STRING};
use windows::core::{implement, Interface, GUID, HRESULT, HSTRING, PCWSTR, PSTR, PWSTR};
use windows::Foundation::{EventRegistrationToken, TypedEventHandler};
use windows::Storage::Provider::{
    IStorageProviderStatusUISource,
    IStorageProviderStatusUISourceFactory,
    IStorageProviderStatusUISourceFactory_Impl,
    IStorageProviderStatusUISource_Impl,
    StorageProviderState,
    StorageProviderStatusUI,
};

// ── CLSIDs ────────────────────────────────────────────────────────────────────

/// {AA7F4C3E-2B48-4C9A-9E2F-1D8B5C4A7E6F}
pub const CLSID_IMMICH_HANDLER: GUID = GUID {
    data1: 0xAA7F_4C3E, data2: 0x2B48, data3: 0x4C9A,
    data4: [0x9E, 0x2F, 0x1D, 0x8B, 0x5C, 0x4A, 0x7E, 0x6F],
};

/// {3F8A9C1B-2D4E-5F60-A7B8-C9D0E1F23456}
pub const CLSID_FREE_UP_CMD: GUID = GUID {
    data1: 0x3F8A_9C1B, data2: 0x2D4E, data3: 0x5F60,
    data4: [0xA7, 0xB8, 0xC9, 0xD0, 0xE1, 0xF2, 0x34, 0x56],
};

/// {4A9B0D2C-3E5F-6071-B8C9-D0E1F2345678}
pub const CLSID_ALWAYS_KEEP_CMD: GUID = GUID {
    data1: 0x4A9B_0D2C, data2: 0x3E5F, data3: 0x6071,
    data4: [0xB8, 0xC9, 0xD0, 0xE1, 0xF2, 0x34, 0x56, 0x78],
};

/// {5A3B2C1D-4E5F-6070-8192-A3B4C5D6E7F8}
/// Registered in SyncRootManager as `StorageProviderStatusUISourceFactory`.
/// Explorer instantiates this to obtain the cloud/checkmark Status column icons.
pub const CLSID_STATUS_UI_FACTORY: GUID = GUID {
    data1: 0x5A3B_2C1D, data2: 0x4E5F, data3: 0x6070,
    data4: [0x81, 0x92, 0xA3, 0xB4, 0xC5, 0xD6, 0xE7, 0xF8],
};

const IID_IUNKNOWN: GUID = GUID {
    data1: 0x0000_0000, data2: 0x0000, data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// ── File-attribute bit constants ─────────────────────────────────────────────

const ATTR_REPARSE: u32 = 0x0000_0400;
const ATTR_OFFLINE: u32 = 0x0000_1000;
const ATTR_RECALL:  u32 = 0x0040_0000;

// ── Explorer command state / flags (u32 in windows-rs 0.58) ──────────────────

const ECS_ENABLED: u32 = 0;
const ECS_HIDDEN:  u32 = 2;
const ECF_DEFAULT: u32 = 0;

// ── DLL exports ──────────────────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid:   *const GUID,
    ppv:    *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() { return E_POINTER; }
    *ppv = std::ptr::null_mut();

    let clsid = *rclsid;
    if clsid != CLSID_IMMICH_HANDLER
        && clsid != CLSID_FREE_UP_CMD
        && clsid != CLSID_ALWAYS_KEEP_CMD
        && clsid != CLSID_STATUS_UI_FACTORY
    {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    if *riid != <IClassFactory as Interface>::IID && *riid != IID_IUNKNOWN {
        return E_NOINTERFACE;
    }

    let factory: IClassFactory = ImmichClassFactory { clsid }.into();
    *ppv = factory.into_raw() as *mut c_void;
    S_OK
}

/// Shell extensions are loaded into Explorer for the duration of the process;
/// returning S_FALSE is safe and prevents the DLL from unloading prematurely.
#[no_mangle]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

// ── Class factory ─────────────────────────────────────────────────────────────

#[implement(IClassFactory)]
struct ImmichClassFactory {
    clsid: GUID,
}

impl IClassFactory_Impl for ImmichClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&windows_core::IUnknown>,
        riid:      *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> windows_core::Result<()> {
        unsafe { *ppvobject = std::ptr::null_mut(); }
        if punkouter.is_some() {
            return Err(windows_core::Error::from(CLASS_E_NOAGGREGATION));
        }

        unsafe {
            let iid = &*riid;
            if self.clsid == CLSID_IMMICH_HANDLER {
                let handler: IShellExtInit = ImmichHandler {
                    files: Mutex::new(Vec::new()),
                }.into();
                if *iid == IID_IUNKNOWN || *iid == <IShellExtInit as Interface>::IID {
                    *ppvobject = handler.into_raw() as *mut c_void;
                    Ok(())
                } else if *iid == <IContextMenu as Interface>::IID {
                    let ctx = handler.cast::<IContextMenu>()
                        .map_err(|_| windows_core::Error::from(E_NOINTERFACE))?;
                    *ppvobject = ctx.into_raw() as *mut c_void;
                    Ok(())
                } else {
                    Err(windows_core::Error::from(E_NOINTERFACE))
                }
            } else if self.clsid == CLSID_FREE_UP_CMD {
                let cmd: IExplorerCommand = FreeUpSpaceCommand.into();
                if *iid == IID_IUNKNOWN || *iid == <IExplorerCommand as Interface>::IID {
                    *ppvobject = cmd.into_raw() as *mut c_void;
                    Ok(())
                } else {
                    Err(windows_core::Error::from(E_NOINTERFACE))
                }
            } else if self.clsid == CLSID_ALWAYS_KEEP_CMD {
                let cmd: IExplorerCommand = AlwaysKeepCommand.into();
                if *iid == IID_IUNKNOWN || *iid == <IExplorerCommand as Interface>::IID {
                    *ppvobject = cmd.into_raw() as *mut c_void;
                    Ok(())
                } else {
                    Err(windows_core::Error::from(E_NOINTERFACE))
                }
            } else if self.clsid == CLSID_STATUS_UI_FACTORY {
                let factory: IStorageProviderStatusUISourceFactory = ImmichStatusFactory.into();
                if *iid == IID_IUNKNOWN
                    || *iid == <IStorageProviderStatusUISourceFactory as Interface>::IID
                {
                    *ppvobject = factory.into_raw() as *mut c_void;
                    Ok(())
                } else {
                    Err(windows_core::Error::from(E_NOINTERFACE))
                }
            } else {
                Err(windows_core::Error::from(CLASS_E_CLASSNOTAVAILABLE))
            }
        }
    }

    fn LockServer(&self, _flock: BOOL) -> windows_core::Result<()> {
        Ok(())
    }
}

// ── Shell extension handler (classic context menu) ───────────────────────────

#[implement(IShellExtInit, IContextMenu)]
struct ImmichHandler {
    files: Mutex<Vec<String>>,
}

// ── IShellExtInit ─────────────────────────────────────────────────────────────

impl IShellExtInit_Impl for ImmichHandler_Impl {
    fn Initialize(
        &self,
        _pidlfolder: *const ITEMIDLIST,
        pdtobj:      Option<&IDataObject>,
        _hkeyprogid: HKEY,
    ) -> windows_core::Result<()> {
        let mut files = self.files.lock().unwrap();
        files.clear();

        let dobj = match pdtobj {
            Some(d) => d,
            None    => return Ok(()),
        };

        let fmt = FORMATETC {
            cfFormat: CF_HDROP.0,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };

        let mut stgm = unsafe { dobj.GetData(&fmt)? };

        unsafe {
            let hdrop = HDROP(stgm.u.hGlobal.0);
            let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);

            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None) as usize + 1;
                let mut buf = vec![0u16; len];
                DragQueryFileW(hdrop, i, Some(&mut buf));
                buf.pop();
                if let Ok(s) = String::from_utf16(&buf) {
                    files.push(s);
                }
            }

            ReleaseStgMedium(&mut stgm);
        }

        Ok(())
    }
}

// ── IContextMenu ──────────────────────────────────────────────────────────────

const CMD_FREE_UP_SPACE: u32 = 0;
const CMD_ALWAYS_KEEP:   u32 = 1;

impl IContextMenu_Impl for ImmichHandler_Impl {
    fn QueryContextMenu(
        &self,
        hmenu:      HMENU,
        indexmenu:  u32,
        idcmdfirst: u32,
        _idcmdlast: u32,
        uflags:     u32,
    ) -> windows_core::Result<()> {
        // CMF_DEFAULTONLY = 0x1
        if uflags & 0x0000_0001 != 0 {
            return Ok(());
        }

        let files      = self.files.lock().unwrap();
        // Windows does not invoke shell extension handlers for offline/dehydrated
        // files (to prevent accidental recalls). Show both items for any hydrated
        // placeholder so the user can either free space or pin the file in place.
        let hydrated   = files.iter().any(|f| is_hydrated_placeholder(f));
        let show_pin   = hydrated; // pin option always shown alongside free-up-space

        let mut highest: i32 = -1;
        let mut idx = indexmenu;

        unsafe {
            if hydrated {
                let t: Vec<u16> = "Free up space\0".encode_utf16().collect();
                let _ = InsertMenuW(
                    hmenu, idx,
                    MF_BYPOSITION | MF_STRING,
                    (idcmdfirst + CMD_FREE_UP_SPACE) as usize,
                    PCWSTR(t.as_ptr()),
                );
                idx += 1;
                highest = highest.max(CMD_FREE_UP_SPACE as i32);
            }
            if show_pin {
                let t: Vec<u16> = "Always keep on this device\0".encode_utf16().collect();
                let _ = InsertMenuW(
                    hmenu, idx,
                    MF_BYPOSITION | MF_STRING,
                    (idcmdfirst + CMD_ALWAYS_KEEP) as usize,
                    PCWSTR(t.as_ptr()),
                );
                highest = highest.max(CMD_ALWAYS_KEEP as i32);
            }
        }

        if highest < 0 { return Ok(()); }
        // Return count in low 16 bits of a success HRESULT.
        Err(windows_core::Error::from(HRESULT(highest + 1)))
    }

    fn InvokeCommand(
        &self,
        pici: *const windows::Win32::UI::Shell::CMINVOKECOMMANDINFO,
    ) -> windows_core::Result<()> {
        let verb_raw = unsafe { (*pici).lpVerb.0 as usize };
        if verb_raw >> 16 != 0 {
            return Err(windows_core::Error::from(E_INVALIDARG));
        }
        let cmd = (verb_raw & 0xFFFF) as u32;

        let files = self.files.lock().unwrap();
        match cmd {
            CMD_FREE_UP_SPACE => {
                for path in files.iter().filter(|f| is_hydrated_placeholder(f)) {
                    dehydrate_file(path);
                }
            }
            CMD_ALWAYS_KEEP => {
                for path in files.iter().filter(|f| is_hydrated_placeholder(f)) {
                    pin_file(path);
                }
            }
            _ => return Err(windows_core::Error::from(E_INVALIDARG)),
        }
        Ok(())
    }

    fn GetCommandString(
        &self,
        idcmd:     usize,
        utype:     u32,
        _reserved: *const u32,
        pszname:   PSTR,
        cchmax:    u32,
    ) -> windows_core::Result<()> {
        let text: &str = match (idcmd as u32, utype) {
            (CMD_FREE_UP_SPACE, 4) => "ImmichFreeUpSpace\0",
            (CMD_FREE_UP_SPACE, 5) => "Free up local space — the file stays available in Immich\0",
            (CMD_ALWAYS_KEEP,   4) => "ImmichAlwaysKeep\0",
            (CMD_ALWAYS_KEEP,   5) => "Always keep a full local copy from Immich\0",
            _ => return Err(windows_core::Error::from(E_INVALIDARG)),
        };
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().collect();
            let dst = std::slice::from_raw_parts_mut(pszname.0 as *mut u16, cchmax as usize);
            let n = wide.len().min(dst.len());
            dst[..n].copy_from_slice(&wide[..n]);
            if n < dst.len() { dst[n] = 0; }
        }
        Ok(())
    }
}

// ── IExplorerCommand: "Free up space" ────────────────────────────────────────

#[implement(IExplorerCommand)]
struct FreeUpSpaceCommand;

impl IExplorerCommand_Impl for FreeUpSpaceCommand_Impl {
    fn GetTitle(&self, _items: Option<&IShellItemArray>) -> windows_core::Result<PWSTR> {
        Ok(unsafe { alloc_pwstr("Free up space") })
    }

    fn GetIcon(&self, _items: Option<&IShellItemArray>) -> windows_core::Result<PWSTR> {
        Err(windows_core::Error::from(E_NOTIMPL))
    }

    fn GetToolTip(&self, _items: Option<&IShellItemArray>) -> windows_core::Result<PWSTR> {
        Err(windows_core::Error::from(E_NOTIMPL))
    }

    fn GetCanonicalName(&self) -> windows_core::Result<GUID> {
        Ok(CLSID_FREE_UP_CMD)
    }

    fn GetState(
        &self,
        items: Option<&IShellItemArray>,
        _ok_to_be_slow: BOOL,
    ) -> windows_core::Result<u32> {
        let paths = unsafe { paths_from_item_array(items) };
        if paths.iter().any(|p| is_hydrated_placeholder(p)) {
            Ok(ECS_ENABLED)
        } else {
            Ok(ECS_HIDDEN)
        }
    }

    fn Invoke(
        &self,
        items: Option<&IShellItemArray>,
        _pbc: Option<&IBindCtx>,
    ) -> windows_core::Result<()> {
        let paths = unsafe { paths_from_item_array(items) };
        for path in paths.iter().filter(|p| is_hydrated_placeholder(p)) {
            dehydrate_file(path);
        }
        Ok(())
    }

    fn GetFlags(&self) -> windows_core::Result<u32> {
        Ok(ECF_DEFAULT)
    }

    fn EnumSubCommands(&self) -> windows_core::Result<IEnumExplorerCommand> {
        Err(windows_core::Error::from(E_NOTIMPL))
    }
}

// ── IExplorerCommand: "Always keep on this device" ───────────────────────────

#[implement(IExplorerCommand)]
struct AlwaysKeepCommand;

impl IExplorerCommand_Impl for AlwaysKeepCommand_Impl {
    fn GetTitle(&self, _items: Option<&IShellItemArray>) -> windows_core::Result<PWSTR> {
        Ok(unsafe { alloc_pwstr("Always keep on this device") })
    }

    fn GetIcon(&self, _items: Option<&IShellItemArray>) -> windows_core::Result<PWSTR> {
        Err(windows_core::Error::from(E_NOTIMPL))
    }

    fn GetToolTip(&self, _items: Option<&IShellItemArray>) -> windows_core::Result<PWSTR> {
        Err(windows_core::Error::from(E_NOTIMPL))
    }

    fn GetCanonicalName(&self) -> windows_core::Result<GUID> {
        Ok(CLSID_ALWAYS_KEEP_CMD)
    }

    fn GetState(
        &self,
        items: Option<&IShellItemArray>,
        _ok_to_be_slow: BOOL,
    ) -> windows_core::Result<u32> {
        let paths = unsafe { paths_from_item_array(items) };
        if paths.iter().any(|p| is_dehydrated_placeholder(p)) {
            Ok(ECS_ENABLED)
        } else {
            Ok(ECS_HIDDEN)
        }
    }

    fn Invoke(
        &self,
        items: Option<&IShellItemArray>,
        _pbc: Option<&IBindCtx>,
    ) -> windows_core::Result<()> {
        let paths = unsafe { paths_from_item_array(items) };
        for path in paths.iter().filter(|p| is_dehydrated_placeholder(p)) {
            pin_file(path);
        }
        Ok(())
    }

    fn GetFlags(&self) -> windows_core::Result<u32> {
        Ok(ECF_DEFAULT)
    }

    fn EnumSubCommands(&self) -> windows_core::Result<IEnumExplorerCommand> {
        Err(windows_core::Error::from(E_NOTIMPL))
    }
}

// ── IExplorerCommand helpers ──────────────────────────────────────────────────

/// Allocate a null-terminated UTF-16 string with CoTaskMemAlloc.
/// The Windows shell is responsible for freeing it.
unsafe fn alloc_pwstr(s: &str) -> PWSTR {
    let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * 2;
    let ptr = CoTaskMemAlloc(bytes) as *mut u16;
    if !ptr.is_null() {
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
    }
    PWSTR(ptr)
}

/// Extract filesystem paths from an IShellItemArray.
unsafe fn paths_from_item_array(items: Option<&IShellItemArray>) -> Vec<String> {
    let items = match items { Some(i) => i, None => return vec![] };
    let count = match items.GetCount() { Ok(n) => n, Err(_) => return vec![] };
    let mut result = Vec::new();
    for i in 0..count {
        if let Ok(item) = items.GetItemAt(i) {
            if let Ok(name) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                if let Ok(s) = name.to_string() {
                    result.push(s);
                }
                CoTaskMemFree(Some(name.0 as *const c_void));
            }
        }
    }
    result
}

// ── File-state helpers ────────────────────────────────────────────────────────

fn file_attrs(path: &str) -> Option<u32> {
    // Use GetFileAttributesW rather than std::fs::metadata: the latter opens a
    // file handle which can trigger cloud-file recall on a dehydrated placeholder.
    // GetFileAttributesW reads only the directory entry and never causes recall.
    let w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    let attrs = unsafe {
        windows::Win32::Storage::FileSystem::GetFileAttributesW(
            windows::core::PCWSTR(w.as_ptr()),
        )
    };
    if attrs == u32::MAX { None } else { Some(attrs) }
}

fn is_hydrated_placeholder(path: &str) -> bool {
    file_attrs(path).map_or(false, |a| {
        a & ATTR_REPARSE != 0 && a & ATTR_RECALL == 0 && a & ATTR_OFFLINE == 0
    })
}

fn is_dehydrated_placeholder(path: &str) -> bool {
    file_attrs(path).map_or(false, |a| {
        a & ATTR_REPARSE != 0 && a & ATTR_RECALL != 0
    })
}

// ── Cloud file operations ─────────────────────────────────────────────────────

fn open_for_cf_ops(path: &str) -> Option<HANDLE> {
    let w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    // FILE_FLAG_OPEN_REPARSE_POINT (0x00200000): open the placeholder directly
    // without triggering hydration.
    let flags = FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_BACKUP_SEMANTICS.0 | 0x0020_0000);
    unsafe {
        CreateFileW(
            PCWSTR(w.as_ptr()),
            FILE_WRITE_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            flags,
            HANDLE::default(),
        ).ok()
    }
}

fn dehydrate_file(path: &str) {
    if let Some(h) = open_for_cf_ops(path) {
        unsafe {
            let _ = CfDehydratePlaceholder(h, 0, -1, CF_DEHYDRATE_FLAG_NONE, None);
            let _ = CloseHandle(h);
        }
    }
}

fn pin_file(path: &str) {
    if let Some(h) = open_for_cf_ops(path) {
        unsafe {
            let _ = CfSetPinState(h, CF_PIN_STATE(1), CF_SET_PIN_FLAG_NONE, None);
            let _ = CloseHandle(h);
        }
    }
}

// ── IStorageProviderStatusUISourceFactory ─────────────────────────────────────
//
// Explorer reads the CLSID from
//   HKCU\...\SyncRootManager\{key}\StorageProviderStatusUISourceFactory
// and CoCreateInstance's it.  Without this factory the Status column is always
// empty regardless of the placeholder CF attributes.
//
// Implementation chain:
//   ImmichStatusFactory  (IStorageProviderStatusUISourceFactory)
//     └─ GetStatusUISource() → ImmichStatusSource  (IStorageProviderStatusUISource)
//          └─ StatusUI() → StorageProviderStatusUI (WinRT system class, ProviderUIStatus = Synced)

#[implement(IStorageProviderStatusUISourceFactory)]
struct ImmichStatusFactory;

impl IStorageProviderStatusUISourceFactory_Impl for ImmichStatusFactory_Impl {
    fn GetStatusUISource(
        &self,
        _syncrootid: &HSTRING,
    ) -> windows_core::Result<IStorageProviderStatusUISource> {
        let source: IStorageProviderStatusUISource = ImmichStatusSource.into();
        Ok(source)
    }
}

#[implement(IStorageProviderStatusUISource)]
struct ImmichStatusSource;

impl IStorageProviderStatusUISource_Impl for ImmichStatusSource_Impl {
    fn GetStatusUI(&self) -> windows_core::Result<StorageProviderStatusUI> {
        // Create the WinRT system class, set ProviderState = InSync.
        // Explorer reads the UIStatus (computed from ProviderState) to decide
        // which icon to show in the Status column.
        let ui = StorageProviderStatusUI::new()?;
        ui.SetProviderState(StorageProviderState::InSync)?;
        Ok(ui)
    }

    fn StatusUIChanged(
        &self,
        _handler: Option<&TypedEventHandler<IStorageProviderStatusUISource, windows_core::IInspectable>>,
    ) -> windows_core::Result<EventRegistrationToken> {
        // Stub: we never fire status-change events; return a dummy token.
        Ok(EventRegistrationToken { Value: 0 })
    }

    fn RemoveStatusUIChanged(
        &self,
        _token: &EventRegistrationToken,
    ) -> windows_core::Result<()> {
        Ok(())
    }
}
