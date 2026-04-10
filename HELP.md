# Summit — Help Guide

## Table of Contents
1. [What's New in Version 1.0.0](#whats-new-in-version-100)
2. [Resolved Issues & Fixes — Version 1.0.0](#resolved-issues--fixes--version-100)
3. [Getting Started](#getting-started)
4. [Adding Your First Account](#adding-your-first-account)
5. [The System Tray](#the-system-tray)
6. [The Dashboard](#the-dashboard)
7. [Sync Status](#sync-status)
8. [Connection Modes: Local vs Remote](#connection-modes-local-vs-remote)
9. [Local Network Discovery](#local-network-discovery)
10. [Managing Multiple Accounts](#managing-multiple-accounts)
11. [Folders & Sync Mode](#folders--sync-mode)
12. [Files On-Demand (Cloud Placeholders)](#files-on-demand-cloud-placeholders)
13. [Explorer Status Column Icons](#explorer-status-column-icons)
14. [Duplicate Handling](#duplicate-handling)
15. [Sync Settings](#sync-settings)
16. [App Settings](#app-settings)
17. [Editing or Removing an Account](#editing-or-removing-an-account)
18. [Diagnostics](#diagnostics)
19. [Troubleshooting](#troubleshooting)

---

## What's New in Version 1.0.0

### Files On-Demand — OneDrive-style cloud placeholders

Summit now supports **Files On-Demand** via the Windows Cloud Files API — the same mechanism used by OneDrive. Every photo in your Immich library appears as a placeholder file in Windows Explorer. Files occupy almost no disk space until you open them, at which point they download from Immich automatically. You can free up space or pin files using the Explorer right-click menu.

### Explorer Status column icons

When using Files On-Demand, Windows Explorer shows cloud status icons in the **Status** column — a cloud icon for online-only placeholders and a green checkmark for locally available files — exactly like OneDrive.

### Context menu integration — "Free up space" and "Always keep on this device"

Right-clicking a hydrated cloud file now shows **"Free up space"** (revert to placeholder) and **"Always keep on this device"** (pin locally) directly in the Windows Explorer classic context menu (Show more options).

### Local network auto-discovery

When adding or editing an account, the **Find** button automatically scans your local network for Immich servers on port 2283. Discovered servers can be selected with a single click.

### Local / Remote URL fallback

Each account can have both a **local network URL** and a **remote internet URL**. Summit automatically uses the local URL when you are on your home or office network and falls back to the remote URL when away — no manual switching required.

### Multiple account support

Summit supports multiple Immich accounts simultaneously. Each account has independent upload folders, download folder, sync mode, and duplicate handling settings. All enabled accounts sync in sequence each cycle.

### Pause / Resume sync

Sync can be paused and resumed from both the tray icon right-click menu and the Dashboard Sync Status page. The tray tooltip updates to reflect the current sync phase.

### Detailed diagnostics

The Sync Status page includes diagnostics for Files On-Demand: per-file state inspection, shell registration checks, and WRT registration checks — useful for verifying that cloud icons and context menu items are correctly wired up.

### Reliable debug logging

All key decisions are logged to `%LOCALAPPDATA%\Temp\summit_debug.log` in real time, independently of the application log system. This file is always written even during early startup and is the first place to check when investigating issues.

---

## Resolved Issues & Fixes — Version 1.0.0

### Explorer freezes / "Not Responding" when navigating into the sync folder

**Root cause:** The Windows Cloud Files API was configured with `CF_POPULATION_POLICY_FULL`, which causes Windows to send a `FETCH_PLACEHOLDERS` callback every time Explorer navigates into the folder and wait for the app to respond before completing the navigation. The callback was not handled, so Explorer waited indefinitely.

**Fix:** Changed to `CF_POPULATION_POLICY_ALWAYS_FULL`, which tells Windows that all placeholders are always created proactively and it should never fire the population callback. This matches the app's actual behaviour (all placeholders are created at the end of each sync cycle) and eliminates the hang entirely.

### Sync root folder being recreated after deletion

**Root cause:** When a folder is registered with the Windows Cloud Files filter driver (`CfRegisterSyncRoot`), the driver attaches a reparse point to it. If the folder is then deleted while this reparse point is present, the driver automatically recreates the folder the next time any process accesses the parent directory.

**Fix:** `CfUnregisterSyncRoot` must be called while the folder still exists (i.e. while the reparse point is present) to instruct the driver to stop managing the path. The uninstaller now does this automatically for all configured sync folders before removing any files. A `--unregister-path` CLI flag is also available for manual recovery.

### Multiple "Status" columns appearing in Explorer

**Root cause:** Each installation generated a new profile GUID, leaving behind a stale `SyncRootManager` registry entry from the previous installation. Each stale entry adds an additional Status column in Explorer.

**Fix:** Before writing a new `SyncRootManager` entry, the app now enumerates and removes all existing entries whose key name starts with the app's provider prefix. Only one entry is ever present at a time.

### Explorer hanging after duplicate registry entries

**Root cause:** Both a manual `HKCU` SyncRootManager entry (written by the app) and an `HKLM` entry (written by the Windows Runtime `StorageProviderSyncRootManager::Register()`) existed simultaneously for the same sync root ID. Having both causes Windows Explorer's namespace builder to deadlock.

**Fix:** After `StorageProviderSyncRootManager::Register()` succeeds and writes its HKLM entry, the app immediately deletes the HKCU entry. HKLM supersedes HKCU; only one entry is kept.

### Sync root folder being created automatically without user consent

**Root cause:** Earlier code called `create_dir_all` on the configured download folder path before checking whether it existed, creating folders the user had never selected.

**Fix:** All automatic folder creation has been removed. The app validates that a folder exists before saving it as a sync target and aborts the sync cycle if the folder is absent. Folders must be created manually by the user before being selected in Settings.

### Status column icons not appearing (blank Status column)

**Root cause:** The `StorageProviderStatusUISourceFactory` registry value pointing to the shell extension DLL was not being written to the HKLM `SyncRootManager` entry created by `StorageProviderSyncRootManager::Register()`. Without it, Explorer cannot find the DLL that supplies the cloud/checkmark icons.

**Fix:** After `Register()` succeeds, the app manually writes the `StorageProviderStatusUISourceFactory` value (containing the CLSID of the status icon DLL) to the HKLM entry.

### Changing the sync root folder left the old folder in a broken state

**Root cause:** When the user changed to a different sync root folder, the app called `CfDisconnectSyncRoot` (which stops callback processing) but not `CfUnregisterSyncRoot` (which removes the CF filter driver's reparse point). The old folder retained its reparse point and would auto-recreate itself if deleted, exactly like the first-run folder recreation bug.

**Fix:** `disconnect_profile` now calls `CfUnregisterSyncRoot` on the old folder path immediately after disconnecting. The old folder's reparse point is removed, so it can be deleted cleanly. Old placeholder files are left in the old folder as ordinary files and can be deleted manually.

### Shell extension DLL locked during installer updates

**Root cause:** Explorer loads `summit_shell_ext.dll` and holds it open. A subsequent installer run could not overwrite the locked file, causing updates to fail silently.

**Fix:** The installer ships the DLL with its base name (`summit_shell_ext.dll`). On startup the app copies it to a versioned filename (`summit_shell_ext_1.0.0.dll`) and registers that versioned copy. The base-named file is never held open by Explorer, so the installer can always replace it. Old versioned files are cleaned up automatically.

---

## Getting Started

Summit runs silently in the background as a **system tray application** — there is no persistent window on your taskbar. After launching, look for the Summit icon in your system tray (bottom-right corner of the taskbar, near the clock).

The app starts hidden. Click the tray icon to open the Dashboard, or right-click for the menu.

> **First run:** No folders are created automatically. Nothing happens until you add an account and configure your sync folders.

---

## Adding Your First Account

1. Click the tray icon (or right-click → **Open Dashboard**).
2. Click **Add Account** on the welcome screen, or go to **Accounts** in the sidebar.
3. Click **Add Account** and fill in the form:
   - **Server URL** — the address of your Immich server (e.g. `https://photos.yourdomain.com` or `http://192.168.1.50:2283`)
   - **Email** — your Immich login email
   - **Password** — your Immich password
4. Click **Sign In**.

Summit authenticates with your server and automatically creates a named API key called **"Summit — *YourPCName*"** on your Immich account. Your password is never stored — only the API key is saved locally.

---

## The System Tray

Right-click the Summit tray icon for these options:

| Option | Description |
|---|---|
| **Open Dashboard** | Shows the main window |
| **Pause Sync** | Temporarily pauses all sync activity |
| **Resume Sync** | Appears when sync is paused — resumes normal operation |
| **Quit Summit** | Exits the application completely |

Left-clicking the tray icon opens the Dashboard directly. The tray tooltip shows the current sync phase (Idle, Uploading, Downloading, Paused, or Error).

Closing the Dashboard **hides** it — Summit continues running in the background.

---

## The Dashboard

The Dashboard is a single window with a sidebar for navigation:

| Sidebar item | What it does |
|---|---|
| **Sync Status** | Live sync activity, counters, pause/resume/sync-now |
| **Folders** | Choose upload and download folders, sync mode, duplicate handling |
| **Accounts** | Add, edit, and manage Immich accounts |
| **Sync** | Configure how often the sync cycle runs |
| **App Settings** | Launch at startup, notifications |
| **Help** | Opens this help window |
| **About** | App version and info |

---

## Sync Status

The Sync Status page shows:

- **Connection status** — green dot (connected), red dot (error), with server version and Local/Remote badge
- **Current phase** — Idle, Uploading, Downloading, Paused, or Error
- **Counters** — files uploaded ↑, downloaded ↓, skipped, and errors for the current cycle
- **Last sync time** — when the most recent cycle completed
- **Pause / Resume** button — temporarily stops the sync engine
- **Sync Now** button — triggers an immediate sync cycle without waiting for the interval

All enabled accounts are synced in sequence each cycle. The counters reflect totals across all accounts.

---

## Connection Modes: Local vs Remote

If you have configured a **Local network URL** for your account, Summit automatically tries to connect via your local network first (faster). If the local connection fails or is unavailable, it falls back to your remote/internet URL automatically.

The Dashboard shows which mode is active:
- **Local** badge — connected via your home/office network
- **Remote** badge — connected via the internet

No action is required — switching happens automatically each time the app connects.

---

## Local Network Discovery

When adding or editing an account, you can use the **Find** button to automatically scan your local network for Immich servers.

1. In the Add Account or Edit Account form, expand the **Local network URL** section.
2. Click **Find**.
3. Summit scans your local `/24` subnet on port 2283 and lists any Immich servers found.
4. Click a result to fill in the URL automatically.

> **Note:** Discovery may take up to 10 seconds. It also tries `immich.local` and `immich` hostnames automatically. If your Immich server runs on a non-standard port, enter the address manually.

---

## Managing Multiple Accounts

Summit supports multiple Immich accounts (e.g. personal + family servers). **All enabled accounts sync simultaneously** — you do not need to switch between them.

- **Add accounts** via the Accounts sidebar page.
- **Active account** — the account shown in the Dashboard title bar and Sync Status page. Switch it using the checkmark button next to any account in the list.
- **Enabled / disabled** — all accounts with a configured API key are synced automatically. Delete an account to stop syncing it entirely.

Each account has its own upload folders, download folder, sync mode, and duplicate handling settings, configured independently on the **Folders** page.

---

## Folders & Sync Mode

Go to **Folders** in the sidebar to configure sync behaviour for the active account.

### Sync Mode

| Mode | Behaviour |
|---|---|
| **Cloud + Local** | Upload local photos to Immich **and** download Immich photos to your device as regular files |
| **Cloud Only** | Upload local photos to Immich only — nothing is downloaded to this device |
| **Files On-Demand** | Placeholder files appear in Explorer for every Immich photo. Files download automatically when you open them. "Free up space" and "Always keep on this device" appear in the right-click menu. |

### Upload Folders

Add one or more local folders. Any image or video file found in these folders (including subfolders) will be uploaded to Immich. Supported formats: JPG, PNG, GIF, HEIC, WEBP, TIFF, RAW and common camera RAW formats.

Files are deduplicated by SHA-1 hash on the Immich server — re-uploading the same file does nothing.

### Download Folder (Cloud + Local mode)

Only shown in **Cloud + Local** mode. New photos from Immich that are not already on your device will be downloaded here as regular files.

**Requirements:**
- The folder must already exist — Summit **never creates folders automatically**.
- Works with any location Windows can write to, including mapped network drives and UNC paths (`\\server\share`).

### Sync Root Folder (Files On-Demand mode)

See the **Files On-Demand** section below for full details and requirements.

---

## Files On-Demand (Cloud Placeholders)

Files On-Demand is Summit's most powerful sync mode. It works like OneDrive Files On-Demand: placeholder files appear in Explorer for every photo in your Immich library. Files occupy almost no disk space until you open them, at which point they are downloaded from Immich automatically.

### How it works

1. After each sync cycle, Summit creates **placeholder files** in your sync root folder — one per photo in your Immich library.
2. Placeholders look like real files in Explorer and show a **cloud icon** in the Status column.
3. When you open a placeholder (double-click), Windows automatically asks Summit to download the file. A progress indicator appears; the file opens once downloaded.
4. After hydration, the file shows a **green checkmark** icon and is fully available offline.
5. Right-clicking a hydrated file reveals **"Free up space"** (revert to placeholder) and **"Always keep on this device"** (pin so it is never dehydrated automatically).

### Sync Root Folder requirements

- Must be on a **local NTFS drive** (e.g. `C:\`, `D:\`). Mapped network drives and UNC paths are not supported by the Windows Cloud Files API.
- Must **not** be a Windows known folder directly (e.g. do not use `C:\Users\You\Pictures` itself — use a subfolder such as `C:\Users\You\Pictures\Immich`).
- Must **already exist** before you save the setting. Summit will not create it for you. Create the folder in Explorer first, then select it.
- You can change this folder at any time in Settings — see **Changing your sync root folder** below.

### First-time setup

1. Create your desired sync folder in Explorer (e.g. `D:\Immich Photos`).
2. In Summit → **Folders**, set Sync Mode to **Files On-Demand** and select the folder.
3. Click **Save**.
4. Click **Sync Now** on the Sync Status page. Placeholder files will appear in the folder after the cycle completes.
5. Navigate to the folder in Explorer. You should see your Immich photos as placeholder files with cloud icons.

### Changing your sync root folder

You can change the sync root folder at any time in **Settings → Folders**. Here is exactly what happens:

1. The new folder is validated (must already exist on a local NTFS drive) and saved.
2. On the next sync cycle, the app detects the folder has changed.
3. The **old folder** is safely disconnected — the Windows Cloud Files driver releases its reparse point so the old folder will not auto-recreate itself if you delete it.
4. The **new folder** is registered as the sync root and connected.
5. Placeholder files are created in the new folder at the end of that sync cycle.

**What happens to the old folder's placeholder files?**
The old placeholder files are left behind — they are not automatically deleted or moved. Once the old folder is unregistered they become ordinary zero-byte files with no cloud association. You can delete the old folder (and its contents) safely after the next sync cycle completes.

**What if I want to rename the folder rather than change it?**
1. In Settings → Folders, select a different (temporary) folder and save. This unregisters the original path.
2. Rename the original folder in Explorer.
3. In Settings → Folders, select the renamed folder and save.
4. Click Sync Now — placeholders will be recreated in the renamed folder.

### Context menu options

Both options appear under **Show more options** (the classic context menu) when right-clicking a **hydrated** (fully downloaded) cloud file:

- **Free up space** — removes the local copy, leaving a placeholder. The file remains in Immich and will re-download when opened.
- **Always keep on this device** — pins the file so it is always kept local and never dehydrated automatically.

These options do **not** appear on dehydrated (online-only) placeholders or on regular local files.

---

## Explorer Status Column Icons

When your sync root folder is registered, Explorer shows cloud status icons in the **Status** column. If the Status column is not visible, right-click any column header in Explorer and enable it.

| Icon | Meaning |
|---|---|
| ☁️ Cloud / offline icon | **Dehydrated placeholder** — file exists in Immich but is not downloaded locally. Double-click to download. |
| ✓ Green checkmark | **Hydrated cloud file** — fully downloaded locally. "Free up space" is available via right-click. |
| 📌 Checkmark + pin | **Pinned cloud file** — always kept local. "Always keep on this device" was used. |
| ↕ Sync arrows | **Syncing** — file is currently being uploaded or downloaded. |

If the Status column shows blank entries or no icons, see the [Troubleshooting](#troubleshooting) section.

---

## Duplicate Handling

When a file being downloaded already exists in your download folder, Summit follows your chosen policy:

| Option | Behaviour |
|---|---|
| **Keep both** | Renames the downloaded copy with a timestamp suffix, e.g. `photo.conflict-20260322-153000.jpg` |
| **Overwrite** | Replaces the existing local file with the version from Immich |
| **Skip** | Does not download the file; marks it as skipped so it is not checked again this cycle |

> This setting applies to **downloads only**. Uploads use SHA-1 checksums for deduplication — the Immich server rejects exact duplicates automatically regardless of this setting.

---

## Sync Settings

Go to **Sync** in the sidebar to set the sync interval:

| Interval | Use case |
|---|---|
| 1 minute | Near real-time (higher resource usage) |
| 5 minutes | Default — good balance |
| 15 minutes | Light usage |
| 30 minutes | Infrequent sync |
| 1 hour | Minimal background activity |

The interval controls how often a full scan cycle runs. Each cycle: scans all upload folders → uploads new images → downloads new Immich assets (if enabled) → creates/updates placeholders (if Files On-Demand).

New files are also detected immediately via a file system watcher — when a new image appears in a watched upload folder, it uploads within a few seconds without waiting for the next interval.

---

## App Settings

Go to **App Settings** in the sidebar:

- **Launch at startup** — Summit starts automatically when you log in to Windows. Uses the Windows Task Scheduler (not a registry Run key), so it works reliably for standard and administrator accounts.
- **Show notifications** — desktop notifications for sync events (conflicts, errors, completions).

---

## Editing or Removing an Account

In **Accounts**, each account row has two action buttons:

- **Pencil icon** — opens the Edit Account form to change the display name, remote URL, or local URL.
- **Trash icon** — removes the account from Summit. Nothing is deleted from your Immich server or from your local files.

> The API key created at login remains on your Immich server. To revoke it, go to Immich → Account Settings → API Keys and delete the key named **"Summit — *YourPCName*"**.

---

## Diagnostics

### Files On-Demand Diagnostics

On the **Sync Status** page, when your active profile is in Files On-Demand mode, a **Files On-Demand Diagnostics** card appears. Click **"Check file states"** to inspect every file in your sync folder:

| State shown | Meaning |
|---|---|
| `PLACEHOLDER (dehydrated)` | Online-only placeholder — open it to hydrate |
| `HYDRATED CLOUD FILE` | Fully downloaded — "Free up space" available |
| `PINNED cloud file` | Pinned locally — won't auto-dehydrate |
| `regular local file` | Not managed by cloud sync — was there before FOD was set up |

### Shell Registration Check

On the **Sync Status** page, the **"Check shell registration"** button reports whether the Windows Cloud Files API sync root and the Explorer shell extension (context menu DLL) are correctly registered. Use this if cloud icons or context menu items are missing.

### WRT Registration Check

The **"Check WRT registration"** button reports whether the Windows Runtime `StorageProviderSyncRootManager` entry exists in the system registry. This entry is responsible for the Status column icons appearing in Explorer. If it is absent, cloud icons will not show.

---

## Troubleshooting

### "No connection" shown in the Dashboard
- Check that your Immich server is running and accessible.
- Edit the account (pencil icon) and verify the server URL includes the correct port if non-standard (default is 2283 for HTTP or 443 for HTTPS).
- If using a local URL, confirm you are on the same network as the server.
- Try opening the server URL directly in a browser — you should see the Immich login page.

### Sign in fails with "Invalid credentials"
- Double-check your email and password.
- Make sure you are using your **Immich** login credentials, not your NAS or server admin password.
- Immich passwords are case-sensitive.

### Files are not being uploaded
- Check that upload folders are configured in the **Folders** page.
- Confirm the account is connected (green dot on Sync Status).
- Click **Sync Now** and watch the counters — if files are being skipped, they may already exist on the server (SHA-1 deduplication).
- Ensure the upload folder contains supported image/video formats.

### Files are not being downloaded
- Confirm **Sync Mode** is set to **Cloud + Local** (not Cloud Only or Files On-Demand).
- Check that a **Download Folder** is selected and the folder exists on disk.
- Ensure there is sufficient free disk space.
- Click **Sync Now** to trigger an immediate cycle.

### The sync root folder was deleted and keeps reappearing

This happens when the Windows Cloud Files filter driver has the folder registered as a sync root. The driver attaches a reparse point to the folder; if you delete the folder while that reparse point is present, the driver recreates it automatically the next time anything accesses the parent directory.

**To permanently stop this, the reparse point must be removed while the folder exists.**

**Option 1 — change or remove the folder in Settings (recommended):**
1. Make sure Summit is running.
2. Go to Settings → Folders and either select a different sync root folder or change the sync mode to **Cloud Only** or **Cloud + Local**, then save.
3. The app will unregister the old path on the next sync cycle (or immediately if you click Sync Now).
4. Delete the old folder. It will not reappear.

**Option 2 — manual unregister via command line:**

If the app is already uninstalled or the above does not work, recreate the folder first (so the reparse point can be removed), then run from an elevated command prompt:
```
"C:\Program Files\Summit\Summit.exe" --unregister-path "C:\Your\Sync\Folder"
```
After this completes, delete the folder normally.

### Explorer shows "Not Responding" when navigating to the sync folder
- This can occur immediately after changing the sync root folder — restart Explorer (`Ctrl+Shift+Esc` → Details → right-click `explorer.exe` → End task, then File → Run new task → `explorer.exe`).
- If it persists, check the Windows Application event log for CF API errors.
- Ensure Summit is running — the app must be active to respond to Windows Cloud Files callbacks.

### Ghost entries appear in Explorer's left-hand navigation pane (e.g. "Summit")
- These are Desktop\NameSpace entries written when the sync root is registered.
- They are removed automatically when you uninstall Summit or remove the account.
- To remove manually, run `regedit` and delete the key under:
  `HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace`
  that corresponds to Summit, then restart Explorer.

### Explorer Status column is blank or missing cloud icons

**Status column not visible:**
- Right-click any column header in Explorer and tick **Status**.

**Status column visible but blank:**
- The Windows Runtime sync root registration may be missing. Use the **Check WRT registration** button on the Sync Status page.
- This can happen if the app is running without a valid Windows package identity. Installing via the provided NSIS installer and ensuring the MSIX sparse package installs correctly resolves this.
- Try running a sync cycle (Sync Now) — if the WRT registration was just created this session, a restart of Explorer may be needed to pick it up.

**Multiple "Status" columns appear:**
- Stale sync root registry entries from a previous installation are present. Uninstall, reinstall, and run **Check WRT registration** to confirm only one entry exists.

### "Free up space" or "Always keep on this device" do not appear in the right-click menu

Both options only appear in the **classic context menu** ("Show more options") when right-clicking a **hydrated cloud file** (green checkmark icon).

| File state | What you can do |
|---|---|
| **Dehydrated placeholder** (cloud icon) | Double-click to download first, then right-click |
| **Hydrated cloud file** (green checkmark) | Right-click → Show more options → both options appear |
| **Regular local file** (no cloud icon) | Neither option will ever appear — not a cloud-managed file |

**Step-by-step to test:**
1. Confirm your account is in **Files On-Demand** mode with a sync root folder set.
2. Click **Sync Now** — placeholder files are created.
3. Find a file with a **cloud icon** in Explorer. **Double-click** it to trigger a download.
4. Wait for the icon to change to a green checkmark.
5. Right-click the green-checkmark file → **Show more options** → **"Free up space"** and **"Always keep on this device"** should now appear.

**If only "regular local files" are shown in Diagnostics:**
These files existed in the folder before Files On-Demand was activated. Summit only creates placeholders for Immich photos that don't already exist locally with the same filename. Options:
- Use a **different (empty) folder** as your sync root, or
- Delete the existing local copies to let the next sync cycle recreate them as proper cloud placeholders.

### The shell extension context menu DLL is not loading

If the context menu items never appear even after hydrating files and checking via **Show more options**:
1. Use the **Check shell registration** button on the Sync Status page — it will report whether `summit_shell_ext.dll` is correctly registered.
2. If not registered, try reinstalling Summit.
3. If registered but still not appearing, open an elevated command prompt and run:
   ```
   regsvr32 "C:\Program Files\Summit\summit_shell_ext.dll"
   ```
   Then restart Explorer.

### Discovery finds no servers
- Ensure your Immich server is running and accessible on port 2283.
- Confirm your PC is on the same local network as the server.
- Check that your router/firewall does not block subnet broadcasts.
- Try entering the IP address manually (e.g. `http://192.168.1.50:2283`).

### Discovery finds multiple IPs for the same server
- This is normal when Immich runs in Docker on a host with multiple network adapters or when the host has both wired and wireless connections. Both addresses connect to the same server — choose either one.

### The app is not in the system tray
- Check the overflow tray area (click `∧` near the clock).
- Check **Task Manager** — if `tauri-app.exe` is running but the tray icon is invisible, click **Open Dashboard** from the Start Menu shortcut.
- If the app is not running at all, relaunch it from the Start Menu.

### Debug log location

Summit writes a detailed debug log to:
```
%LOCALAPPDATA%\Temp\summit_debug.log
```
(Typically `C:\Users\YourName\AppData\Local\Temp\summit_debug.log`)

This log captures every key decision the app makes — sync cycles, folder checks, CF API calls, and registration steps. Include this file when reporting issues.

A second log (Tauri plugin log) is written to:
```
%APPDATA%\com.summit.app\logs\app.log
```

---

*Summit v1.0.0 — an unofficial companion for [Immich](https://immich.app).*
