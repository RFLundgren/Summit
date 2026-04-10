import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ── Types ────────────────────────────────────────────────────────────────────

export interface AccountProfile {
  id: string
  displayName: string
  email: string
  localUrl: string
  remoteUrl: string
  apiKey: string
  uploadFolders: string[]
  downloadFolder: string
  defaultSyncMode: 'cloud_and_local' | 'cloud_only' | 'cloud_browse'
  duplicateHandling: 'overwrite' | 'rename' | 'skip'
  enabled: boolean
}

export interface AppConfig {
  activeProfileId: string
  profiles: AccountProfile[]
  syncIntervalSecs: number
  autostart: boolean
  notificationsEnabled: boolean
}

export interface ConnectionTestResult {
  success: boolean
  message: string
  version: string | null
  urlMode: string | null // "Local" | "Remote" | "Direct"
}

export interface SyncStatus {
  phase: 'idle' | 'uploading' | 'downloading' | 'paused' | 'error'
  uploaded: number
  downloaded: number
  skipped: number
  errors: number
  lastSyncAt: string | null
  message: string
}

export interface ActivityEntry {
  occurredAt: string
  eventType: 'upload' | 'download' | 'skip' | 'error'
  fileName: string
  message: string
}

export interface UpdaterStatus {
  state: 'idle' | 'checking' | 'available' | 'not-available' | 'downloading' | 'error'
  version?: string
  percent?: number
  error?: string
}

// ── Commands ─────────────────────────────────────────────────────────────────

export const ipc = {
  // Auth / profiles
  loginAccount: (args: {
    localUrl: string
    remoteUrl: string
    email: string
    password: string
  }) => invoke<AccountProfile>('login_account', args),

  getConfig: () => invoke<AppConfig>('get_config'),

  setActiveProfile: (profileId: string) =>
    invoke<void>('set_active_profile', { profileId }),

  deleteProfile: (profileId: string) =>
    invoke<void>('delete_profile', { profileId }),

  saveAppConfig: (args: {
    syncIntervalSecs: number
    autostart: boolean
    notificationsEnabled: boolean
  }) => invoke<void>('save_app_config', args),

  // Connection
  testConnection: (url: string, key: string) =>
    invoke<ConnectionTestResult>('test_connection', { url, key }),

  getActiveProfileStatus: () =>
    invoke<ConnectionTestResult>('get_active_profile_status'),

  updateProfile: (args: {
    profileId: string
    displayName: string
    localUrl: string
    remoteUrl: string
  }) => invoke<void>('update_profile', args),

  updateSyncFolders: (args: {
    profileId: string
    uploadFolders: string[]
    downloadFolder: string
    syncMode: string
    duplicateHandling: string
  }) => invoke<void>('update_sync_folders', args),

  discoverServers: () => invoke<string[]>('discover_servers'),

  // Sync engine
  getSyncStatus: () => invoke<SyncStatus>('get_sync_status'),
  triggerSync: () => invoke<void>('trigger_sync'),
  pauseSync: () => invoke<void>('pause_sync'),
  resumeSync: () => invoke<void>('resume_sync'),
  getRecentActivity: (profileId: string, limit?: number) =>
    invoke<ActivityEntry[]>('get_recent_activity', { profileId, limit }),

  checkCloudFileState: (folder: string) =>
    invoke<string>('check_cloud_file_state', { folder }),

  checkShellRegistration: () =>
    invoke<string>('check_shell_registration'),

  checkWrtRegistration: (folder: string) =>
    invoke<string>('check_wrt_registration', { folder }),

  // Updater
  checkForUpdates: () => invoke<void>('check_for_updates'),
  downloadUpdate: () => invoke<void>('download_update'),

  onUpdaterStatus: (cb: (status: UpdaterStatus) => void) =>
    listen<UpdaterStatus>('updater:status', (e) => cb(e.payload)),
}
