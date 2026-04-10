import { useState, useCallback, useRef } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  X, MountainSnow, Zap, CheckCircle, Sparkles, Monitor, LayoutDashboard, RefreshCw,
  Wifi, Search, Users, Folder, Cloud, Eye, Copy, Timer, Settings,
  UserCog, Activity, HelpCircle, UserPlus, Heart,
} from 'lucide-react'

// ─── Shared primitives ────────────────────────────────────────────────────────

function H1({ children }: { children: React.ReactNode }) {
  return <h1 className="text-base font-semibold text-gray-900 mb-4">{children}</h1>
}

function H2({ children }: { children: React.ReactNode }) {
  return <h2 className="text-sm font-semibold text-gray-800 mt-5 mb-2">{children}</h2>
}

function H3({ children }: { children: React.ReactNode }) {
  return <h3 className="text-xs font-semibold text-gray-700 mt-4 mb-1.5">{children}</h3>
}

function P({ children }: { children: React.ReactNode }) {
  return <p className="text-xs text-gray-600 leading-relaxed mb-2">{children}</p>
}

function Li({ children }: { children: React.ReactNode }) {
  return <li className="text-xs text-gray-600 leading-relaxed">{children}</li>
}

function Ul({ children }: { children: React.ReactNode }) {
  return <ul className="list-disc list-inside space-y-1 mb-2 ml-1">{children}</ul>
}

function Ol({ children }: { children: React.ReactNode }) {
  return <ol className="list-decimal list-inside space-y-1 mb-2 ml-1">{children}</ol>
}

function Code({ children }: { children: React.ReactNode }) {
  return <code className="text-immich-primary bg-gray-100 px-1 py-0.5 rounded text-[11px] font-mono">{children}</code>
}

function Pre({ children }: { children: React.ReactNode }) {
  return (
    <pre className="bg-gray-100 rounded-md p-3 text-[11px] font-mono text-gray-700 overflow-x-auto mb-2 leading-relaxed">
      {children}
    </pre>
  )
}

function Note({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-blue-50 border border-blue-100 rounded-md px-3 py-2 mb-2">
      <p className="text-xs text-blue-700 leading-relaxed">{children}</p>
    </div>
  )
}

function Table({ headers, rows }: { headers: string[]; rows: string[][] }) {
  return (
    <div className="overflow-x-auto mb-3">
      <table className="w-full text-xs border-collapse">
        <thead>
          <tr className="border-b border-gray-200">
            {headers.map((h, i) => (
              <th key={i} className="text-left py-1.5 pr-4 text-gray-700 font-semibold whitespace-nowrap">{h}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr key={i} className="border-b border-gray-100">
              {row.map((cell, j) => (
                <td key={j} className="py-1.5 pr-4 text-gray-600 leading-relaxed" dangerouslySetInnerHTML={{ __html: cell }} />
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

// ─── Topics ───────────────────────────────────────────────────────────────────

const TOPICS = [
  { id: 'whatsnew',      label: "What's New v1.0.0",      icon: Sparkles },
  { id: 'resolved',      label: 'Resolved Issues',         icon: CheckCircle },
  { id: 'start',         label: 'Getting Started',         icon: Zap },
  { id: 'account',       label: 'Adding an Account',       icon: UserPlus },
  { id: 'tray',          label: 'System Tray',             icon: Monitor },
  { id: 'dashboard',     label: 'Dashboard',               icon: LayoutDashboard },
  { id: 'sync-status',   label: 'Sync Status',             icon: RefreshCw },
  { id: 'connection',    label: 'Connection Modes',        icon: Wifi },
  { id: 'discovery',     label: 'Network Discovery',       icon: Search },
  { id: 'accounts',      label: 'Multiple Accounts',       icon: Users },
  { id: 'folders',       label: 'Folders & Sync Mode',     icon: Folder },
  { id: 'fod',           label: 'Files On-Demand',         icon: Cloud },
  { id: 'icons',         label: 'Explorer Status Icons',   icon: Eye },
  { id: 'duplicates',    label: 'Duplicate Handling',      icon: Copy },
  { id: 'sync-settings', label: 'Sync Settings',           icon: Timer },
  { id: 'app-settings',  label: 'App Settings',            icon: Settings },
  { id: 'edit-account',  label: 'Edit / Remove Account',  icon: UserCog },
  { id: 'diagnostics',   label: 'Diagnostics',             icon: Activity },
  { id: 'troubleshoot',  label: 'Troubleshooting',         icon: HelpCircle },
]

// ─── Topic content components ─────────────────────────────────────────────────

function WhatsNew() {
  return (
    <>
      <H1>What's New in Version 1.0.0</H1>

      <H2>Files On-Demand — OneDrive-style cloud placeholders</H2>
      <P>Summit now supports <strong>Files On-Demand</strong> via the Windows Cloud Files API — the same mechanism used by OneDrive. Every photo in your Immich library appears as a placeholder file in Windows Explorer. Files occupy almost no disk space until you open them, at which point they download from Immich automatically. You can free up space or pin files using the Explorer right-click menu.</P>

      <H2>Explorer Status column icons</H2>
      <P>When using Files On-Demand, Windows Explorer shows cloud status icons in the <strong>Status</strong> column — a cloud icon for online-only placeholders and a green checkmark for locally available files — exactly like OneDrive.</P>

      <H2>Context menu integration</H2>
      <P>Right-clicking a hydrated cloud file now shows <strong>"Free up space"</strong> (revert to placeholder) and <strong>"Always keep on this device"</strong> (pin locally) directly in the Windows Explorer classic context menu (Show more options).</P>

      <H2>Local network auto-discovery</H2>
      <P>When adding or editing an account, the <strong>Find</strong> button automatically scans your local network for Immich servers on port 2283. Discovered servers can be selected with a single click.</P>

      <H2>Local / Remote URL fallback</H2>
      <P>Each account can have both a <strong>local network URL</strong> and a <strong>remote internet URL</strong>. Summit automatically uses the local URL when you are on your home or office network and falls back to the remote URL when away — no manual switching required.</P>

      <H2>Multiple account support</H2>
      <P>Summit supports multiple Immich accounts simultaneously. Each account has independent upload folders, download folder, sync mode, and duplicate handling settings.</P>

      <H2>Pause / Resume sync</H2>
      <P>Sync can be paused and resumed from both the tray icon right-click menu and the Dashboard Sync Status page. The tray tooltip updates to reflect the current sync phase.</P>

      <H2>Reliable debug logging</H2>
      <P>All key decisions are logged to <Code>%LOCALAPPDATA%\Temp\summit_debug.log</Code> in real time, independently of the application log system. This file is always written even during early startup and is the first place to check when investigating issues.</P>
    </>
  )
}

function ResolvedIssues() {
  return (
    <>
      <H1>Resolved Issues &amp; Fixes — Version 1.0.0</H1>

      <H2>Explorer freezes / "Not Responding" navigating to sync folder</H2>
      <P><strong>Root cause:</strong> The Windows Cloud Files API was configured with <Code>CF_POPULATION_POLICY_FULL</Code>, causing Windows to send a <Code>FETCH_PLACEHOLDERS</Code> callback every time Explorer navigated into the folder and wait indefinitely for a response.</P>
      <P><strong>Fix:</strong> Changed to <Code>CF_POPULATION_POLICY_ALWAYS_FULL</Code>, which tells Windows that all placeholders are created proactively and it should never fire the population callback.</P>

      <H2>Sync root folder being recreated after deletion</H2>
      <P><strong>Root cause:</strong> The CF filter driver attaches a reparse point to the registered folder. Deleting the folder while the reparse point is present causes the driver to recreate it automatically.</P>
      <P><strong>Fix:</strong> <Code>CfUnregisterSyncRoot</Code> is called while the folder still exists. The uninstaller does this automatically for all configured sync folders. A <Code>--unregister-path</Code> CLI flag is also available for manual recovery.</P>

      <H2>Multiple "Status" columns appearing in Explorer</H2>
      <P><strong>Root cause:</strong> Each installation generated a new profile GUID, leaving stale <Code>SyncRootManager</Code> registry entries from previous installations.</P>
      <P><strong>Fix:</strong> Before writing a new <Code>SyncRootManager</Code> entry, the app removes all existing entries whose key name starts with the app's provider prefix. Only one entry ever exists.</P>

      <H2>Explorer hanging after duplicate registry entries</H2>
      <P><strong>Root cause:</strong> Both a manual HKCU entry and an HKLM entry written by <Code>StorageProviderSyncRootManager::Register()</Code> existed simultaneously, causing Windows Explorer's namespace builder to deadlock.</P>
      <P><strong>Fix:</strong> After <Code>Register()</Code> writes its HKLM entry, the app immediately deletes the HKCU entry. Only the HKLM entry is kept.</P>

      <H2>Status column icons not appearing</H2>
      <P><strong>Root cause:</strong> The <Code>StorageProviderStatusUISourceFactory</Code> registry value pointing to the shell extension DLL was not being written to the HKLM entry.</P>
      <P><strong>Fix:</strong> After <Code>Register()</Code> succeeds, the app manually writes the <Code>StorageProviderStatusUISourceFactory</Code> value containing the CLSID of the status icon DLL.</P>

      <H2>Shell extension DLL locked during installer updates</H2>
      <P><strong>Root cause:</strong> Explorer loads <Code>summit_shell_ext.dll</Code> and holds it open, preventing installer overwrites.</P>
      <P><strong>Fix:</strong> The installer ships the DLL with its base name. On startup the app copies it to a versioned filename (e.g. <Code>summit_shell_ext_1.0.0.dll</Code>) and registers that copy. The base-named file is never locked by Explorer. Old versioned files are cleaned up automatically.</P>
    </>
  )
}

function GettingStarted() {
  return (
    <>
      <H1>Getting Started</H1>
      <P>Summit runs silently in the background as a <strong>system tray application</strong> — there is no persistent window on your taskbar. After launching, look for the Summit icon in your system tray (bottom-right corner of the taskbar, near the clock).</P>
      <P>The app starts hidden. Click the tray icon to open the Dashboard, or right-click for the menu.</P>
      <Note><strong>First run:</strong> No folders are created automatically. Nothing happens until you add an account and configure your sync folders.</Note>

      <H2>Quick setup checklist</H2>
      <Ol>
        <Li>Click the tray icon → <strong>Open Dashboard</strong></Li>
        <Li>Click <strong>Add Account</strong> and sign in to your Immich server</Li>
        <Li>Go to <strong>Folders</strong> and choose a sync mode and folders</Li>
        <Li>Click <strong>Sync Now</strong> on the Sync Status page to run your first cycle</Li>
      </Ol>
    </>
  )
}

function AddingAccount() {
  return (
    <>
      <H1>Adding Your First Account</H1>
      <Ol>
        <Li>Click the tray icon (or right-click → <strong>Open Dashboard</strong>).</Li>
        <Li>Click <strong>Add Account</strong> on the welcome screen, or go to <strong>Accounts</strong> in the sidebar.</Li>
        <Li>Fill in the form:
          <Ul>
            <Li><strong>Server URL</strong> — address of your Immich server (e.g. <Code>https://photos.yourdomain.com</Code> or <Code>http://192.168.1.50:2283</Code>)</Li>
            <Li><strong>Email</strong> — your Immich login email</Li>
            <Li><strong>Password</strong> — your Immich password</Li>
          </Ul>
        </Li>
        <Li>Click <strong>Sign In</strong>.</Li>
      </Ol>
      <P>Summit authenticates with your server and automatically creates a named API key called <strong>"Summit — YourPCName"</strong> on your Immich account. Your password is never stored — only the API key is saved locally.</P>
    </>
  )
}

function SystemTray() {
  return (
    <>
      <H1>The System Tray</H1>
      <P>Right-click the Summit tray icon for these options:</P>
      <Table
        headers={['Option', 'Description']}
        rows={[
          ['<strong>Open Dashboard</strong>', 'Shows the main window'],
          ['<strong>Pause Sync</strong>', 'Temporarily pauses all sync activity'],
          ['<strong>Resume Sync</strong>', 'Appears when paused — resumes normal operation'],
          ['<strong>Quit Summit</strong>', 'Exits the application completely'],
        ]}
      />
      <P>Left-clicking the tray icon opens the Dashboard directly. The tray tooltip shows the current sync phase: Idle, Uploading, Downloading, Paused, or Error.</P>
      <P>Closing the Dashboard <strong>hides</strong> it — Summit continues running in the background.</P>
    </>
  )
}

function Dashboard() {
  return (
    <>
      <H1>The Dashboard</H1>
      <P>The Dashboard is a single window with a sidebar for navigation:</P>
      <Table
        headers={['Sidebar item', 'What it does']}
        rows={[
          ['<strong>Sync Status</strong>', 'Live sync activity, counters, pause/resume/sync-now'],
          ['<strong>Folders</strong>', 'Choose upload and download folders, sync mode, duplicate handling'],
          ['<strong>Accounts</strong>', 'Add, edit, and manage Immich accounts'],
          ['<strong>Sync</strong>', 'Configure how often the sync cycle runs'],
          ['<strong>App Settings</strong>', 'Launch at startup, notifications'],
          ['<strong>Help</strong>', 'Opens this help window'],
          ['<strong>About</strong>', 'App version and info'],
        ]}
      />
    </>
  )
}

function SyncStatus() {
  return (
    <>
      <H1>Sync Status</H1>
      <P>The Sync Status page shows:</P>
      <Ul>
        <Li><strong>Connection status</strong> — green dot (connected), red dot (error), with server version and Local/Remote badge</Li>
        <Li><strong>Current phase</strong> — Idle, Uploading, Downloading, Paused, or Error</Li>
        <Li><strong>Counters</strong> — files uploaded ↑, downloaded ↓, skipped, and errors for the current cycle</Li>
        <Li><strong>Last sync time</strong> — when the most recent cycle completed</Li>
        <Li><strong>Pause / Resume</strong> button — temporarily stops the sync engine</Li>
        <Li><strong>Sync Now</strong> button — triggers an immediate sync cycle</Li>
      </Ul>
      <P>All enabled accounts are synced in sequence each cycle. The counters reflect totals across all accounts.</P>
    </>
  )
}

function ConnectionModes() {
  return (
    <>
      <H1>Connection Modes: Local vs Remote</H1>
      <P>If you have configured a <strong>Local network URL</strong> for your account, Summit automatically tries to connect via your local network first (faster). If the local connection fails or is unavailable, it falls back to your remote/internet URL automatically.</P>
      <P>The Dashboard shows which mode is active:</P>
      <Ul>
        <Li><strong>Local</strong> badge — connected via your home/office network</Li>
        <Li><strong>Remote</strong> badge — connected via the internet</Li>
      </Ul>
      <P>No action is required — switching happens automatically each time the app connects.</P>
    </>
  )
}

function NetworkDiscovery() {
  return (
    <>
      <H1>Local Network Discovery</H1>
      <P>When adding or editing an account, you can use the <strong>Find</strong> button to automatically scan your local network for Immich servers.</P>
      <Ol>
        <Li>In the Add Account or Edit Account form, expand the <strong>Local network URL</strong> section.</Li>
        <Li>Click <strong>Find</strong>.</Li>
        <Li>Summit scans your local <Code>/24</Code> subnet on port 2283 and lists any Immich servers found.</Li>
        <Li>Click a result to fill in the URL automatically.</Li>
      </Ol>
      <Note>Discovery may take up to 10 seconds. It also tries <Code>immich.local</Code> and <Code>immich</Code> hostnames automatically. If your Immich server runs on a non-standard port, enter the address manually.</Note>
    </>
  )
}

function MultipleAccounts() {
  return (
    <>
      <H1>Managing Multiple Accounts</H1>
      <P>Summit supports multiple Immich accounts (e.g. personal + family servers). <strong>All enabled accounts sync simultaneously</strong> — you do not need to switch between them.</P>
      <Ul>
        <Li><strong>Add accounts</strong> via the Accounts sidebar page.</Li>
        <Li><strong>Active account</strong> — the account shown in the Dashboard title bar and Sync Status page. Switch it using the checkmark button next to any account in the list.</Li>
        <Li><strong>Enabled / disabled</strong> — all accounts with a configured API key are synced automatically. Delete an account to stop syncing it entirely.</Li>
      </Ul>
      <P>Each account has its own upload folders, download folder, sync mode, and duplicate handling settings, configured independently on the <strong>Folders</strong> page.</P>
    </>
  )
}

function FoldersAndSyncMode() {
  return (
    <>
      <H1>Folders &amp; Sync Mode</H1>
      <P>Go to <strong>Folders</strong> in the sidebar to configure sync behaviour for the active account.</P>

      <H2>Sync Mode</H2>
      <Table
        headers={['Mode', 'Behaviour']}
        rows={[
          ['<strong>Cloud + Local</strong>', 'Upload local photos to Immich <strong>and</strong> download Immich photos to your device as regular files'],
          ['<strong>Cloud Only</strong>', 'Upload local photos to Immich only — nothing is downloaded to this device'],
          ['<strong>Files On-Demand</strong>', 'Placeholder files appear in Explorer for every Immich photo. Files download automatically when you open them.'],
        ]}
      />

      <H2>Upload Folders</H2>
      <P>Add one or more local folders. Any image or video file found in these folders (including subfolders) will be uploaded to Immich. Supported formats: JPG, PNG, GIF, HEIC, WEBP, TIFF, RAW and common camera RAW formats.</P>
      <P>Files are deduplicated by SHA-1 hash on the Immich server — re-uploading the same file does nothing.</P>

      <H2>Download Folder (Cloud + Local mode)</H2>
      <P>Only shown in <strong>Cloud + Local</strong> mode. New photos from Immich that are not already on your device will be downloaded here as regular files.</P>
      <Ul>
        <Li>The folder must already exist — Summit <strong>never creates folders automatically</strong>.</Li>
        <Li>Works with any location Windows can write to, including mapped network drives and UNC paths.</Li>
      </Ul>
    </>
  )
}

function FilesOnDemand() {
  return (
    <>
      <H1>Files On-Demand (Cloud Placeholders)</H1>
      <P>Files On-Demand is Summit's most powerful sync mode. Placeholder files appear in Explorer for every photo in your Immich library. Files occupy almost no disk space until you open them, at which point they are downloaded from Immich automatically.</P>

      <H2>How it works</H2>
      <Ol>
        <Li>After each sync cycle, Summit creates <strong>placeholder files</strong> in your sync root folder — one per photo in your Immich library.</Li>
        <Li>Placeholders look like real files in Explorer and show a <strong>cloud icon</strong> in the Status column.</Li>
        <Li>When you open a placeholder (double-click), Windows asks Summit to download the file. A progress indicator appears; the file opens once downloaded.</Li>
        <Li>After hydration, the file shows a <strong>green checkmark</strong> icon and is fully available offline.</Li>
        <Li>Right-clicking a hydrated file reveals <strong>"Free up space"</strong> and <strong>"Always keep on this device"</strong>.</Li>
      </Ol>

      <H2>Sync Root Folder requirements</H2>
      <Ul>
        <Li>Must be on a <strong>local NTFS drive</strong> (e.g. <Code>C:\</Code>, <Code>D:\</Code>). Mapped network drives are not supported.</Li>
        <Li>Must <strong>not</strong> be a Windows known folder directly — use a subfolder (e.g. <Code>C:\Users\You\Pictures\Immich</Code>, not <Code>C:\Users\You\Pictures</Code>).</Li>
        <Li>Must <strong>already exist</strong> before you save the setting. Create the folder in Explorer first.</Li>
      </Ul>

      <H2>First-time setup</H2>
      <Ol>
        <Li>Create your desired sync folder in Explorer (e.g. <Code>D:\Immich Photos</Code>).</Li>
        <Li>In Summit → <strong>Folders</strong>, set Sync Mode to <strong>Files On-Demand</strong> and select the folder.</Li>
        <Li>Click <strong>Save</strong>.</Li>
        <Li>Click <strong>Sync Now</strong> on the Sync Status page. Placeholder files will appear after the cycle completes.</Li>
      </Ol>

      <H2>Changing your sync root folder</H2>
      <P>You can change the sync root folder at any time in Settings → Folders. What happens:</P>
      <Ol>
        <Li>The new folder is validated and saved.</Li>
        <Li>The <strong>old folder</strong> is safely disconnected — the Windows CF driver releases its reparse point.</Li>
        <Li>The <strong>new folder</strong> is registered as the sync root.</Li>
        <Li>Placeholder files are created in the new folder at the end of the next sync cycle.</Li>
      </Ol>
      <P>The old placeholder files are left behind as ordinary zero-byte files. You can delete the old folder safely after the next sync cycle completes.</P>

      <H2>Context menu options</H2>
      <P>Both options appear under <strong>Show more options</strong> when right-clicking a <strong>hydrated</strong> (downloaded) cloud file:</P>
      <Ul>
        <Li><strong>Free up space</strong> — removes the local copy, leaving a placeholder.</Li>
        <Li><strong>Always keep on this device</strong> — pins the file so it is never dehydrated automatically.</Li>
      </Ul>
      <Note>These options do <strong>not</strong> appear on dehydrated placeholders or regular local files.</Note>
    </>
  )
}

function ExplorerIcons() {
  return (
    <>
      <H1>Explorer Status Column Icons</H1>
      <P>When your sync root folder is registered, Explorer shows cloud status icons in the <strong>Status</strong> column. If the Status column is not visible, right-click any column header in Explorer and enable it.</P>
      <Table
        headers={['Icon', 'Meaning']}
        rows={[
          ['☁️ Cloud icon', '<strong>Dehydrated placeholder</strong> — file exists in Immich but not downloaded locally. Double-click to download.'],
          ['✓ Green checkmark', '<strong>Hydrated cloud file</strong> — fully downloaded locally. "Free up space" available via right-click.'],
          ['📌 Checkmark + pin', '<strong>Pinned cloud file</strong> — always kept local. "Always keep on this device" was used.'],
          ['↕ Sync arrows', '<strong>Syncing</strong> — file is currently being uploaded or downloaded.'],
        ]}
      />
      <P>If the Status column shows blank entries or no icons, see the Troubleshooting section.</P>
    </>
  )
}

function DuplicateHandling() {
  return (
    <>
      <H1>Duplicate Handling</H1>
      <P>When a file being downloaded already exists in your download folder, Summit follows your chosen policy:</P>
      <Table
        headers={['Option', 'Behaviour']}
        rows={[
          ['<strong>Keep both</strong>', 'Renames the downloaded copy with a timestamp suffix, e.g. <code class="text-immich-primary bg-gray-100 px-1 rounded text-[11px] font-mono">photo.conflict-20260322-153000.jpg</code>'],
          ['<strong>Overwrite</strong>', 'Replaces the existing local file with the version from Immich'],
          ['<strong>Skip</strong>', 'Does not download the file; marks it as skipped for this cycle'],
        ]}
      />
      <Note>This setting applies to <strong>downloads only</strong>. Uploads use SHA-1 checksums for deduplication — the Immich server rejects exact duplicates automatically.</Note>
    </>
  )
}

function SyncSettings() {
  return (
    <>
      <H1>Sync Settings</H1>
      <P>Go to <strong>Sync</strong> in the sidebar to set the sync interval:</P>
      <Table
        headers={['Interval', 'Use case']}
        rows={[
          ['1 minute', 'Near real-time (higher resource usage)'],
          ['5 minutes', 'Default — good balance'],
          ['15 minutes', 'Light usage'],
          ['30 minutes', 'Infrequent sync'],
          ['1 hour', 'Minimal background activity'],
        ]}
      />
      <P>The interval controls how often a full scan cycle runs. Each cycle: scans all upload folders → uploads new images → downloads new Immich assets (if enabled) → creates/updates placeholders (if Files On-Demand).</P>
      <P>New files are also detected immediately via a file system watcher — when a new image appears in a watched upload folder, it uploads within a few seconds without waiting for the next interval.</P>
    </>
  )
}

function AppSettings() {
  return (
    <>
      <H1>App Settings</H1>
      <P>Go to <strong>App Settings</strong> in the sidebar:</P>
      <Ul>
        <Li><strong>Launch at startup</strong> — Summit starts automatically when you log in to Windows. Uses the Windows Task Scheduler (not a registry Run key), so it works reliably for standard and administrator accounts.</Li>
        <Li><strong>Show notifications</strong> — desktop notifications for sync events (conflicts, errors, completions).</Li>
      </Ul>
    </>
  )
}

function EditAccount() {
  return (
    <>
      <H1>Editing or Removing an Account</H1>
      <P>In <strong>Accounts</strong>, each account row has two action buttons:</P>
      <Ul>
        <Li><strong>Pencil icon</strong> — opens the Edit Account form to change the display name, remote URL, or local URL.</Li>
        <Li><strong>Trash icon</strong> — removes the account from Summit. Nothing is deleted from your Immich server or from your local files.</Li>
      </Ul>
      <Note>The API key created at login remains on your Immich server. To revoke it, go to Immich → Account Settings → API Keys and delete the key named <strong>"Summit — YourPCName"</strong>.</Note>
    </>
  )
}

function Diagnostics() {
  return (
    <>
      <H1>Diagnostics</H1>

      <H2>Files On-Demand Diagnostics</H2>
      <P>On the <strong>Sync Status</strong> page, when your active profile is in Files On-Demand mode, a <strong>Files On-Demand Diagnostics</strong> card appears. Click <strong>"Check file states"</strong> to inspect every file in your sync folder:</P>
      <Table
        headers={['State shown', 'Meaning']}
        rows={[
          ['<code class="text-immich-primary bg-gray-100 px-1 rounded text-[11px] font-mono">PLACEHOLDER (dehydrated)</code>', 'Online-only placeholder — open it to hydrate'],
          ['<code class="text-immich-primary bg-gray-100 px-1 rounded text-[11px] font-mono">HYDRATED CLOUD FILE</code>', 'Fully downloaded — "Free up space" available'],
          ['<code class="text-immich-primary bg-gray-100 px-1 rounded text-[11px] font-mono">PINNED cloud file</code>', 'Pinned locally — won\'t auto-dehydrate'],
          ['<code class="text-immich-primary bg-gray-100 px-1 rounded text-[11px] font-mono">regular local file</code>', 'Not managed by cloud sync — existed before FOD was set up'],
        ]}
      />

      <H2>Shell Registration Check</H2>
      <P>The <strong>"Check shell registration"</strong> button reports whether the Windows Cloud Files API sync root and the Explorer shell extension (context menu DLL) are correctly registered. Use this if cloud icons or context menu items are missing.</P>

      <H2>WRT Registration Check</H2>
      <P>The <strong>"Check WRT registration"</strong> button reports whether the Windows Runtime <Code>StorageProviderSyncRootManager</Code> entry exists in the system registry. This entry is responsible for Status column icons appearing in Explorer. If absent, cloud icons will not show.</P>
    </>
  )
}

function Troubleshoot() {
  return (
    <>
      <H1>Troubleshooting</H1>

      <H3>"No connection" shown in the Dashboard</H3>
      <Ul>
        <Li>Check that your Immich server is running and accessible.</Li>
        <Li>Verify the server URL includes the correct port (default: 2283 for HTTP, 443 for HTTPS).</Li>
        <Li>If using a local URL, confirm you are on the same network as the server.</Li>
        <Li>Try opening the server URL directly in a browser — you should see the Immich login page.</Li>
      </Ul>

      <H3>Sign in fails with "Invalid credentials"</H3>
      <Ul>
        <Li>Double-check your email and password.</Li>
        <Li>Make sure you are using your <strong>Immich</strong> login credentials, not your NAS or server admin password.</Li>
        <Li>Immich passwords are case-sensitive.</Li>
      </Ul>

      <H3>Files are not being uploaded</H3>
      <Ul>
        <Li>Check that upload folders are configured in the <strong>Folders</strong> page.</Li>
        <Li>Confirm the account is connected (green dot on Sync Status).</Li>
        <Li>Click <strong>Sync Now</strong> and watch the counters — skipped files may already exist on the server.</Li>
        <Li>Ensure the upload folder contains supported image/video formats.</Li>
      </Ul>

      <H3>Files are not being downloaded</H3>
      <Ul>
        <Li>Confirm <strong>Sync Mode</strong> is set to <strong>Cloud + Local</strong>.</Li>
        <Li>Check that a <strong>Download Folder</strong> is selected and the folder exists on disk.</Li>
        <Li>Ensure there is sufficient free disk space.</Li>
      </Ul>

      <H3>The sync root folder was deleted and keeps reappearing</H3>
      <P>The Windows Cloud Files filter driver attaches a reparse point to registered folders. Deleting the folder while that reparse point is present causes the driver to recreate it automatically.</P>
      <P><strong>Option 1 — change the folder in Settings (recommended):</strong></P>
      <Ol>
        <Li>Make sure Summit is running.</Li>
        <Li>Go to Settings → Folders and select a different sync root folder or change the sync mode, then save.</Li>
        <Li>The app unregisters the old path. Delete the old folder — it will not reappear.</Li>
      </Ol>
      <P><strong>Option 2 — manual unregister via command line:</strong></P>
      <Pre>{"\"C:\\Program Files\\Summit\\Summit.exe\" --unregister-path \"C:\\Your\\Sync\\Folder\""}</Pre>

      <H3>Explorer shows "Not Responding" navigating to sync folder</H3>
      <Ul>
        <Li>Restart Explorer: <Code>Ctrl+Shift+Esc</Code> → Details → right-click <Code>explorer.exe</Code> → End task, then File → Run new task → <Code>explorer.exe</Code>.</Li>
        <Li>Ensure Summit is running — the app must be active to respond to Windows Cloud Files callbacks.</Li>
      </Ul>

      <H3>Ghost entries in Explorer's navigation pane</H3>
      <Ul>
        <Li>These are removed automatically when you uninstall Summit or remove the account.</Li>
        <Li>To remove manually, open <Code>regedit</Code> and delete the Summit key under:<br /><Code>HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\Desktop\NameSpace</Code></Li>
      </Ul>

      <H3>Explorer Status column is blank or missing cloud icons</H3>
      <Ul>
        <Li><strong>Column not visible:</strong> Right-click any column header in Explorer and tick <strong>Status</strong>.</Li>
        <Li><strong>Column blank:</strong> Use the <strong>Check WRT registration</strong> button on Sync Status. A restart of Explorer may be needed after registration.</Li>
        <Li><strong>Multiple Status columns:</strong> Stale registry entries from a previous installation. Uninstall, reinstall, and run Check WRT registration.</Li>
      </Ul>

      <H3>"Free up space" / "Always keep on this device" not appearing</H3>
      <P>Both options only appear in the <strong>classic context menu</strong> ("Show more options") when right-clicking a <strong>hydrated cloud file</strong> (green checkmark icon).</P>
      <Ol>
        <Li>Confirm your account is in <strong>Files On-Demand</strong> mode with a sync root folder set.</Li>
        <Li>Click <strong>Sync Now</strong> — placeholder files are created.</Li>
        <Li>Find a file with a <strong>cloud icon</strong>. <strong>Double-click</strong> it to download.</Li>
        <Li>Wait for the icon to change to a green checkmark.</Li>
        <Li>Right-click the file → <strong>Show more options</strong> → both options should now appear.</Li>
      </Ol>

      <H3>Shell extension DLL is not loading</H3>
      <Ol>
        <Li>Use the <strong>Check shell registration</strong> button on the Sync Status page.</Li>
        <Li>If not registered, try reinstalling Summit.</Li>
        <Li>If registered but not appearing, run from an elevated command prompt:</Li>
      </Ol>
      <Pre>{'regsvr32 "C:\\Program Files\\Summit\\summit_shell_ext.dll"'}</Pre>
      <P>Then restart Explorer.</P>

      <H3>Debug log location</H3>
      <P>Summit writes a detailed debug log to:</P>
      <Pre>{'%LOCALAPPDATA%\\Temp\\summit_debug.log'}</Pre>
      <P>A second log (Tauri plugin) is written to:</P>
      <Pre>{'%APPDATA%\\com.summit.app\\logs\\app.log'}</Pre>
    </>
  )
}

// ─── Main component ───────────────────────────────────────────────────────────

export default function Help() {
  const [topic, setTopic] = useState('whatsnew')
  const contentRef = useRef<HTMLDivElement>(null)
  const closeWindow = () => getCurrentWindow().hide()

  const navigate = useCallback((id: string) => {
    setTopic(id)
    if (contentRef.current) contentRef.current.scrollTop = 0
  }, [])

  const renderTopic = () => {
    switch (topic) {
      case 'whatsnew':      return <WhatsNew />
      case 'resolved':      return <ResolvedIssues />
      case 'start':         return <GettingStarted />
      case 'account':       return <AddingAccount />
      case 'tray':          return <SystemTray />
      case 'dashboard':     return <Dashboard />
      case 'sync-status':   return <SyncStatus />
      case 'connection':    return <ConnectionModes />
      case 'discovery':     return <NetworkDiscovery />
      case 'accounts':      return <MultipleAccounts />
      case 'folders':       return <FoldersAndSyncMode />
      case 'fod':           return <FilesOnDemand />
      case 'icons':         return <ExplorerIcons />
      case 'duplicates':    return <DuplicateHandling />
      case 'sync-settings': return <SyncSettings />
      case 'app-settings':  return <AppSettings />
      case 'edit-account':  return <EditAccount />
      case 'diagnostics':   return <Diagnostics />
      case 'troubleshoot':  return <Troubleshoot />
      default:              return <WhatsNew />
    }
  }

  return (
    <div className="flex flex-col h-screen bg-white">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-3.5 border-b border-gray-200 bg-white shrink-0">
        <div className="flex items-center gap-2">
          <MountainSnow size={14} className="text-immich-primary" />
          <span className="text-sm font-semibold text-gray-800">Summit — Help &amp; Reference</span>
          <span className="text-xs text-gray-400">v1.0.0</span>
        </div>
        <button
          onClick={closeWindow}
          className="text-gray-400 hover:text-gray-600 p-1 rounded hover:bg-gray-100 transition-colors"
        >
          <X size={15} />
        </button>
      </div>

      <div className="flex flex-1 min-h-0">
        {/* Sidebar */}
        <div className="flex flex-col gap-0.5 p-2.5 w-44 border-r border-gray-200 bg-gray-50 shrink-0 overflow-y-auto">
          {TOPICS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => navigate(id)}
              className={`flex items-center gap-2 px-2.5 py-1.5 rounded-md text-left text-xs font-medium transition-colors ${
                topic === id
                  ? 'bg-immich-primary text-white'
                  : 'text-gray-600 hover:bg-gray-200 hover:text-gray-900'
              }`}
            >
              <Icon size={12} className="shrink-0" />
              {label}
            </button>
          ))}
        </div>

        {/* Content */}
        <div ref={contentRef} className="flex-1 overflow-y-auto p-6 min-w-0">
          {renderTopic()}

          {/* Footer */}
          <div className="mt-8 pt-4 border-t border-gray-100 flex items-center justify-between">
            <p className="text-xs text-gray-400">Summit v1.0.0 — an unofficial companion for Immich</p>
            <a
              href="https://paypal.me/RFLundgren"
              className="flex items-center gap-1.5 text-xs text-gray-400 hover:text-pink-400 transition-colors no-underline"
              title="Support Summit development"
            >
              <Heart size={11} />
              Support development
            </a>
          </div>
        </div>
      </div>
    </div>
  )
}
