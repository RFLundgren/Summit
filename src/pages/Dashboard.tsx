import { useCallback, useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { isEnabled as autostartIsEnabled, enable as autostartEnable, disable as autostartDisable } from '@tauri-apps/plugin-autostart'
import {
  Cloud, FolderOpen, RefreshCw, Users, Clock, SlidersHorizontal,
  X, Minus, Wifi, Globe, HelpCircle, Loader, Play, Pause,
  ArrowUp, ArrowDown, SkipForward, AlertCircle, Info, History,
} from 'lucide-react'
import { useSettingsStore } from '../stores/settingsStore'
import { ipc, ConnectionTestResult, AppConfig, SyncStatus, ActivityEntry } from '../lib/ipc'
import AccountList from '../components/settings/AccountList'
import FolderSettings from '../components/settings/FolderSettings'

type Page = 'status' | 'folders' | 'accounts' | 'activity' | 'sync' | 'app'

const NAV: { id: Page; icon: React.ElementType; label: string }[] = [
  { id: 'status',   icon: RefreshCw,  label: 'Sync Status' },
  { id: 'folders',  icon: FolderOpen, label: 'Folders' },
  { id: 'accounts', icon: Users,      label: 'Accounts' },
  { id: 'activity', icon: History,    label: 'Activity' },
]

const NAV_BOTTOM: { id: Page; icon: React.ElementType; label: string }[] = [
  { id: 'sync', icon: Clock,             label: 'Sync' },
  { id: 'app',  icon: SlidersHorizontal, label: 'App Settings' },
]

export default function Dashboard() {
  const { config, loaded, load, setConfig } = useSettingsStore()
  const [page, setPage] = useState<Page>('status')
  const [connStatus, setConnStatus] = useState<ConnectionTestResult | null>(null)
  const [syncStatus, setSyncStatus] = useState<SyncStatus | null>(null)
  const [showAbout, setShowAbout] = useState(false)

  useEffect(() => {
    load()
    const unlisteners: (() => void)[] = []
    listen('config://changed', () => load()).then((fn) => unlisteners.push(fn))
    listen<SyncStatus>('sync://status', (e) => setSyncStatus(e.payload)).then((fn) => unlisteners.push(fn))
    return () => unlisteners.forEach((fn) => fn())
  }, [])

  useEffect(() => {
    if (loaded && config.profiles.length > 0) {
      ipc.getActiveProfileStatus().then(setConnStatus).catch(() => {})
      ipc.getSyncStatus().then(setSyncStatus).catch(() => {})
    }
  }, [loaded, config.activeProfileId])

  const activeProfile = config.profiles.find((p) => p.id === config.activeProfileId)
  const closeWindow = () => getCurrentWindow().hide()
  const minimizeWindow = () => getCurrentWindow().minimize()
  const openHelp = () => invoke('show_window', { label: 'help' })

  return (
    <div className="flex flex-col h-screen bg-white">
      {/* Title bar */}
      <div
        data-tauri-drag-region
        className="flex items-center justify-between px-4 py-3 bg-immich-primary select-none flex-shrink-0"
      >
        <div className="flex items-center gap-2 pointer-events-none">
          <Cloud size={18} className="text-white" />
          <span className="text-white font-semibold text-sm">Summit</span>
          {activeProfile && (
            <span className="text-white/60 text-xs ml-1">{activeProfile.displayName}</span>
          )}
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={minimizeWindow}
            className="p-1 text-white/70 hover:text-white hover:bg-white/10 rounded transition-colors"
          >
            <Minus size={14} />
          </button>
          <button
            onClick={closeWindow}
            className="p-1 text-white/70 hover:text-white hover:bg-white/10 rounded transition-colors"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar */}
        <aside className="w-48 bg-gray-50 border-r border-gray-200 flex flex-col py-3 flex-shrink-0">
          <nav className="flex-1 px-2 space-y-0.5">
            {NAV.map(({ id, icon: Icon, label }) => (
              <SidebarItem
                key={id}
                icon={<Icon size={15} />}
                label={label}
                active={page === id}
                onClick={() => setPage(id)}
              />
            ))}

            <div className="my-2 border-t border-gray-200" />

            {NAV_BOTTOM.map(({ id, icon: Icon, label }) => (
              <SidebarItem
                key={id}
                icon={<Icon size={15} />}
                label={label}
                active={page === id}
                onClick={() => setPage(id)}
              />
            ))}
          </nav>

          <div className="px-2 pt-2 border-t border-gray-200 space-y-0.5">
            <SidebarItem
              icon={<HelpCircle size={15} />}
              label="Help"
              active={false}
              onClick={openHelp}
            />
            <SidebarItem
              icon={<Info size={15} />}
              label="About"
              active={false}
              onClick={() => setShowAbout(true)}
            />
          </div>
        </aside>

        {/* Main content */}
        <main className="flex-1 overflow-y-auto">
          {!loaded ? (
            <div className="p-6 text-sm text-gray-400">Loading…</div>
          ) : page === 'status' ? (
            <StatusPage
              connStatus={connStatus}
              syncStatus={syncStatus}
              activeProfile={activeProfile ?? null}
              hasAccount={Boolean(activeProfile)}
              onGoToAccounts={() => setPage('accounts')}
              syncIntervalSecs={config.syncIntervalSecs}
            />
          ) : page === 'activity' ? (
            <ActivityPage config={config} syncStatus={syncStatus} />
          ) : page === 'folders' ? (
            <div className="p-5">
              <PageHeader title="Folders" description="Choose which folders to upload from and download into." />
              <FolderSettings config={config} onConfigChange={setConfig} />
            </div>
          ) : page === 'accounts' ? (
            <div className="p-5">
              <PageHeader title="Accounts" description="Manage your Immich accounts." />
              <AccountList config={config} onConfigChange={setConfig} />
            </div>
          ) : page === 'sync' ? (
            <SyncPage config={config} />
          ) : (
            <AppPage config={config} />
          )}
        </main>
      </div>

      {showAbout && <AboutModal onClose={() => setShowAbout(false)} />}
    </div>
  )
}

// ── Shared ────────────────────────────────────────────────────────────────────

function SidebarItem({
  icon, label, active, onClick,
}: {
  icon: React.ReactNode
  label: string
  active: boolean
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2 w-full px-3 py-2 text-sm rounded-lg transition-colors text-left ${
        active
          ? 'bg-immich-light text-immich-primary font-medium'
          : 'text-gray-600 hover:bg-gray-200'
      }`}
    >
      {icon}
      {label}
    </button>
  )
}

function PageHeader({ title, description }: { title: string; description: string }) {
  return (
    <div className="mb-5">
      <h2 className="text-base font-semibold text-gray-800">{title}</h2>
      <p className="text-xs text-gray-500 mt-0.5">{description}</p>
    </div>
  )
}

// ── Status page ───────────────────────────────────────────────────────────────

function StatusPage({
  connStatus,
  syncStatus,
  activeProfile,
  hasAccount,
  onGoToAccounts,
  syncIntervalSecs,
}: {
  connStatus: ConnectionTestResult | null
  syncStatus: SyncStatus | null
  activeProfile: import('../lib/ipc').AccountProfile | null
  hasAccount: boolean
  onGoToAccounts: () => void
  syncIntervalSecs: number
}) {
  const [triggering, setTriggering] = useState(false)
  const [paused, setPaused] = useState(syncStatus?.phase === 'paused')
  const [countdown, setCountdown] = useState<string | null>(null)
  const [diagResult, setDiagResult] = useState<string | null>(null)
  const [diagRunning, setDiagRunning] = useState(false)
  const [shellDiagResult, setShellDiagResult] = useState<string | null>(null)
  const [shellDiagRunning, setShellDiagRunning] = useState(false)
  const [wrtDiagResult, setWrtDiagResult] = useState<string | null>(null)
  const [wrtDiagRunning, setWrtDiagRunning] = useState(false)

  useEffect(() => {
    setPaused(syncStatus?.phase === 'paused')
  }, [syncStatus?.phase])

  useEffect(() => {
    const lastSyncAt = syncStatus?.lastSyncAt
    const phase = syncStatus?.phase
    if (!lastSyncAt || phase === 'uploading' || phase === 'downloading') {
      setCountdown(null)
      return
    }
    const tick = () => {
      const nextMs = new Date(lastSyncAt).getTime() + syncIntervalSecs * 1000
      const diffSecs = Math.max(0, Math.floor((nextMs - Date.now()) / 1000))
      if (diffSecs === 0) { setCountdown('any moment'); return }
      const m = Math.floor(diffSecs / 60)
      const s = diffSecs % 60
      setCountdown(m > 0 ? `${m}m ${s}s` : `${s}s`)
    }
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [syncStatus?.lastSyncAt, syncStatus?.phase, syncIntervalSecs])

  if (!hasAccount) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center gap-4 p-6">
        <div className="w-16 h-16 rounded-full bg-immich-light flex items-center justify-center">
          <Cloud size={32} className="text-immich-primary" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-gray-800">Welcome to Summit</h2>
          <p className="text-sm text-gray-500 mt-1 max-w-xs">
            Sign in to your Immich account to start syncing your photos.
          </p>
        </div>
        <button
          onClick={onGoToAccounts}
          className="px-5 py-2 bg-immich-primary text-white text-sm font-medium rounded-lg hover:bg-immich-hover transition-colors"
        >
          Add Account
        </button>
      </div>
    )
  }

  const connected = connStatus?.success === true
  const checking = connStatus === null
  const urlMode = connStatus?.urlMode
  const phase = syncStatus?.phase ?? 'idle'
  const isSyncing = phase === 'uploading' || phase === 'downloading'

  const handleTrigger = async () => {
    setTriggering(true)
    try { await ipc.triggerSync() } finally { setTriggering(false) }
  }

  const handlePauseResume = async () => {
    if (paused) {
      await ipc.resumeSync()
      setPaused(false)
    } else {
      await ipc.pauseSync()
      setPaused(true)
    }
  }

  const handleDiagnose = async () => {
    const folder = activeProfile?.downloadFolder
    if (!folder) { setDiagResult('No download folder configured for the active profile.'); return }
    const syncRoot = folder.replace(/[/\\]+$/, '')
    setDiagRunning(true)
    setDiagResult(null)
    try {
      const result = await ipc.checkCloudFileState(syncRoot)
      setDiagResult(result)
    } catch (e) {
      setDiagResult(`Error: ${e}`)
    } finally {
      setDiagRunning(false)
    }
  }

  const handleShellDiag = async () => {
    setShellDiagRunning(true)
    setShellDiagResult(null)
    try {
      const result = await ipc.checkShellRegistration()
      setShellDiagResult(result)
    } catch (e) {
      setShellDiagResult(`Error: ${e}`)
    } finally {
      setShellDiagRunning(false)
    }
  }

  const handleWrtDiag = async () => {
    const folder = activeProfile?.downloadFolder
    if (!folder) { setWrtDiagResult('No download folder configured.'); return }
    const syncRoot = folder.replace(/[/\\]+$/, '')
    setWrtDiagRunning(true)
    setWrtDiagResult(null)
    try {
      const result = await ipc.checkWrtRegistration(syncRoot)
      setWrtDiagResult(result)
    } catch (e) {
      setWrtDiagResult(`Error: ${e}`)
    } finally {
      setWrtDiagRunning(false)
    }
  }

  return (
    <div className="p-5 space-y-4">
      <PageHeader title="Sync Status" description="Current connection and sync activity." />

      {/* Connection card */}
      <div className="rounded-xl border border-gray-200 p-4">
        <div className="flex items-start justify-between">
          <div className="flex items-center gap-3">
            <div className={`w-2.5 h-2.5 rounded-full mt-0.5 flex-shrink-0 ${
              checking ? 'bg-gray-300 animate-pulse' : connected ? 'bg-green-400' : 'bg-red-400'
            }`} />
            <div>
              <p className="text-sm font-medium text-gray-800">
                {checking ? 'Checking…' : connected ? 'Connected' : 'Connection error'}
              </p>
              <p className="text-xs text-gray-500 mt-0.5">
                {connStatus?.message}
                {connStatus?.version && (
                  <span className="ml-1 text-gray-400">· v{connStatus.version}</span>
                )}
              </p>
            </div>
          </div>
          {connected && urlMode && urlMode !== 'Direct' && (
            <div className={`flex items-center gap-1 text-xs px-2 py-1 rounded-full font-medium ${
              urlMode === 'Local' ? 'bg-green-100 text-green-700' : 'bg-blue-100 text-blue-700'
            }`}>
              {urlMode === 'Local' ? <Wifi size={11} /> : <Globe size={11} />}
              {urlMode}
            </div>
          )}
        </div>
      </div>

      {/* Sync engine card */}
      {connected && syncStatus && (
        <div className="rounded-xl border border-gray-200 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {isSyncing ? (
                <Loader size={14} className="text-immich-primary animate-spin" />
              ) : phase === 'paused' ? (
                <Pause size={14} className="text-amber-500" />
              ) : phase === 'error' ? (
                <AlertCircle size={14} className="text-red-500" />
              ) : (
                <RefreshCw size={14} className="text-gray-400" />
              )}
              <span className="text-sm font-medium text-gray-700 capitalize">
                {phase === 'idle' ? 'Idle' : phase === 'uploading' ? 'Uploading…' : phase === 'downloading' ? 'Downloading…' : phase === 'paused' ? 'Paused' : 'Error'}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={handlePauseResume}
                title={paused ? 'Resume sync' : 'Pause sync'}
                className="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
              >
                {paused ? <Play size={14} /> : <Pause size={14} />}
              </button>
              <button
                onClick={handleTrigger}
                disabled={triggering || isSyncing}
                title="Sync now"
                className="p-1.5 text-gray-400 hover:text-immich-primary hover:bg-immich-light rounded transition-colors disabled:opacity-40"
              >
                {triggering ? <Loader size={14} className="animate-spin" /> : <RefreshCw size={14} />}
              </button>
            </div>
          </div>

          <p className="text-xs text-gray-500">{syncStatus.message}</p>

          {/* Counters */}
          <div className="flex items-center gap-4 pt-1 border-t border-gray-100">
            <Stat icon={<ArrowUp size={11} />} label="Uploaded" value={syncStatus.uploaded} color="text-blue-600" />
            <Stat icon={<ArrowDown size={11} />} label="Downloaded" value={syncStatus.downloaded} color="text-green-600" />
            <Stat icon={<SkipForward size={11} />} label="Skipped" value={syncStatus.skipped} color="text-gray-400" />
            {syncStatus.errors > 0 && (
              <Stat icon={<AlertCircle size={11} />} label="Errors" value={syncStatus.errors} color="text-red-500" />
            )}
          </div>

          <div className="flex items-center justify-between text-xs text-gray-400">
            {syncStatus.lastSyncAt && (
              <span>Last sync: {new Date(syncStatus.lastSyncAt).toLocaleTimeString()}</span>
            )}
            {phase === 'idle' && countdown && (
              <span className="ml-auto">Next sync in {countdown}</span>
            )}
          </div>
        </div>
      )}

      {/* Cloud files diagnostic */}
      {activeProfile?.defaultSyncMode === 'cloud_browse' && (
        <div className="rounded-xl border border-gray-200 p-4 space-y-2">
          <p className="text-xs font-medium text-gray-600">Files On-Demand Diagnostics</p>
          <div className="flex items-center gap-2">
            <button
              onClick={handleDiagnose}
              disabled={diagRunning}
              className="text-xs px-2 py-1 bg-gray-100 hover:bg-gray-200 rounded transition-colors disabled:opacity-40"
            >
              {diagRunning ? 'Checking…' : 'Check file states'}
            </button>
            <button
              onClick={handleShellDiag}
              disabled={shellDiagRunning}
              className="text-xs px-2 py-1 bg-gray-100 hover:bg-gray-200 rounded transition-colors disabled:opacity-40"
            >
              {shellDiagRunning ? 'Checking…' : 'Check shell registration'}
            </button>
            <button
              onClick={handleWrtDiag}
              disabled={wrtDiagRunning}
              className="text-xs px-2 py-1 bg-gray-100 hover:bg-gray-200 rounded transition-colors disabled:opacity-40"
            >
              {wrtDiagRunning ? 'Checking…' : 'Check WinRT registration'}
            </button>
          </div>
          {diagResult && <DiagOutput text={diagResult} />}
          {shellDiagResult && <DiagOutput text={shellDiagResult} />}
          {wrtDiagResult && <DiagOutput text={wrtDiagResult} />}
        </div>
      )}
    </div>
  )
}

function DiagOutput({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const copy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    }).catch(() => {})
  }
  return (
    <div className="relative">
      <pre className="text-xs text-gray-600 bg-gray-50 rounded p-2 pr-14 overflow-x-auto whitespace-pre-wrap leading-relaxed select-text">
        {text}
      </pre>
      <button
        onClick={copy}
        className="absolute top-1.5 right-1.5 text-xs px-2 py-0.5 bg-white border border-gray-200 hover:bg-gray-100 rounded transition-colors text-gray-500"
      >
        {copied ? 'Copied!' : 'Copy'}
      </button>
    </div>
  )
}

function Stat({ icon, label, value, color }: { icon: React.ReactNode; label: string; value: number; color: string }) {
  return (
    <div className="flex items-center gap-1">
      <span className={color}>{icon}</span>
      <span className="text-xs text-gray-500">{label}:</span>
      <span className={`text-xs font-medium ${color}`}>{value}</span>
    </div>
  )
}

// ── Activity page ─────────────────────────────────────────────────────────────

function relativeTime(iso: string): string {
  const secs = Math.floor((Date.now() - new Date(iso).getTime()) / 1000)
  if (secs < 60) return 'just now'
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins}m ago`
  const hrs = Math.floor(mins / 60)
  if (hrs < 24) return `${hrs}h ago`
  return `${Math.floor(hrs / 24)}d ago`
}

const ACTIVITY_ICON: Record<string, React.ReactNode> = {
  upload:   <ArrowUp    size={12} className="text-blue-500"  />,
  download: <ArrowDown  size={12} className="text-green-500" />,
  skip:     <SkipForward size={12} className="text-gray-400" />,
  error:    <AlertCircle size={12} className="text-red-500"  />,
}

function ActivityPage({ config, syncStatus }: { config: AppConfig; syncStatus: SyncStatus | null }) {
  const [entries, setEntries] = useState<ActivityEntry[]>([])
  const [loading, setLoading] = useState(true)

  const load = useCallback(async () => {
    const id = config.activeProfileId
    if (!id) { setLoading(false); return }
    try {
      setEntries(await ipc.getRecentActivity(id, 100))
    } catch (_) {}
    setLoading(false)
  }, [config.activeProfileId])

  useEffect(() => { load() }, [load])

  // Refresh when a sync cycle finishes (phase transitions back to idle)
  useEffect(() => {
    if (syncStatus?.phase === 'idle') load()
  }, [syncStatus?.phase])

  return (
    <div className="p-5">
      <PageHeader title="Activity" description="Recent sync events across all upload and download cycles." />

      {loading ? (
        <div className="flex items-center gap-2 text-sm text-gray-400 py-4">
          <Loader size={14} className="animate-spin" /> Loading…
        </div>
      ) : entries.length === 0 ? (
        <div className="flex flex-col items-center gap-3 py-12 text-center">
          <div className="w-12 h-12 rounded-full bg-gray-100 flex items-center justify-center">
            <History size={22} className="text-gray-400" />
          </div>
          <p className="text-sm text-gray-500">No activity yet.</p>
          <p className="text-xs text-gray-400">Events will appear here after the first sync cycle.</p>
        </div>
      ) : (
        <div className="space-y-0 rounded-xl border border-gray-200 overflow-hidden">
          {entries.map((e, i) => (
            <div
              key={i}
              className="flex items-start gap-3 px-4 py-2.5 border-b border-gray-100 last:border-0 hover:bg-gray-50 transition-colors"
            >
              <span className="mt-0.5 flex-shrink-0">{ACTIVITY_ICON[e.eventType] ?? ACTIVITY_ICON.skip}</span>
              <div className="flex-1 min-w-0">
                <p className="text-xs font-medium text-gray-700 truncate">{e.fileName}</p>
                {e.message && <p className="text-xs text-gray-400 truncate">{e.message}</p>}
              </div>
              <span className="text-xs text-gray-400 flex-shrink-0 mt-0.5">{relativeTime(e.occurredAt)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ── Sync settings page ────────────────────────────────────────────────────────

function SyncPage({ config }: { config: AppConfig }) {
  const { setConfig } = useSettingsStore()
  const [interval, setInterval] = useState(config.syncIntervalSecs)
  const [saving, setSaving] = useState(false)
  const [saved, setSaved] = useState(false)

  const options = [
    { label: '1 minute',   value: 60 },
    { label: '5 minutes',  value: 300 },
    { label: '15 minutes', value: 900 },
    { label: '30 minutes', value: 1800 },
    { label: '1 hour',     value: 3600 },
  ]

  const handleSave = async () => {
    setSaving(true)
    try {
      await ipc.saveAppConfig({
        syncIntervalSecs: interval,
        autostart: config.autostart,
        notificationsEnabled: config.notificationsEnabled,
      })
      setConfig({ ...config, syncIntervalSecs: interval })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="p-5">
      <PageHeader title="Sync" description="Configure how often Summit checks for changes." />

      <div className="space-y-1">
        {options.map((o) => (
          <label key={o.value} className="flex items-center gap-2 cursor-pointer py-1">
            <input
              type="radio"
              name="interval"
              value={o.value}
              checked={interval === o.value}
              onChange={() => setInterval(o.value)}
              className="accent-immich-primary"
            />
            <span className="text-sm text-gray-700">{o.label}</span>
          </label>
        ))}
      </div>
      <p className="mt-2 text-xs text-gray-500">
        How often to check Immich for new photos to download.
        Uploads happen immediately when a new file is detected.
      </p>

      <div className="flex items-center gap-3 mt-5">
        {saved && <span className="text-sm text-green-600 font-medium">Saved!</span>}
        <button
          onClick={handleSave}
          disabled={saving || interval === config.syncIntervalSecs}
          className="px-4 py-2 text-sm font-medium text-white bg-immich-primary hover:bg-immich-hover disabled:opacity-40 rounded-lg flex items-center gap-2 transition-colors"
        >
          {saving && <Loader size={14} className="animate-spin" />}
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  )
}

// ── App settings page ─────────────────────────────────────────────────────────

function AppPage({ config }: { config: AppConfig }) {
  const { setConfig } = useSettingsStore()
  const [autostart, setAutostart] = useState(config.autostart)
  const [notifications, setNotifications] = useState(config.notificationsEnabled)
  const [saving, setSaving] = useState(false)
  const [saved, setSaved] = useState(false)

  const isDirty = autostart !== config.autostart || notifications !== config.notificationsEnabled

  const handleSave = async () => {
    setSaving(true)
    try {
      await ipc.saveAppConfig({
        syncIntervalSecs: config.syncIntervalSecs,
        autostart,
        notificationsEnabled: notifications,
      })
      try {
        const currently = await autostartIsEnabled()
        if (autostart && !currently) await autostartEnable()
        if (!autostart && currently) await autostartDisable()
      } catch (_) {}
      setConfig({ ...config, autostart, notificationsEnabled: notifications })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="p-5">
      <PageHeader title="App Settings" description="Startup and notification preferences." />

      <div className="space-y-5">
        <Toggle
          label="Launch at startup"
          description="Start Summit automatically when you log in to Windows."
          checked={autostart}
          onToggle={() => setAutostart((v: boolean) => !v)}
        />
        <div className="border-t border-gray-100" />
        <Toggle
          label="Show notifications"
          description="Notify when syncs complete, conflicts occur, or errors are detected."
          checked={notifications}
          onToggle={() => setNotifications((v: boolean) => !v)}
        />
      </div>

      <div className="flex items-center gap-3 mt-5">
        {saved && <span className="text-sm text-green-600 font-medium">Saved!</span>}
        <button
          onClick={handleSave}
          disabled={saving || !isDirty}
          className="px-4 py-2 text-sm font-medium text-white bg-immich-primary hover:bg-immich-hover disabled:opacity-40 rounded-lg flex items-center gap-2 transition-colors"
        >
          {saving && <Loader size={14} className="animate-spin" />}
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  )
}

// ── About modal ───────────────────────────────────────────────────────────────

function AboutModal({ onClose }: { onClose: () => void }) {
  const [version, setVersion] = useState('1.0.0')

  useEffect(() => {
    import('@tauri-apps/api/app').then(({ getVersion }) =>
      getVersion().then(setVersion).catch(() => {})
    )
  }, [])

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-sm">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200">
          <span className="font-semibold text-gray-800">About</span>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">
            <X size={18} />
          </button>
        </div>
        <div className="px-5 py-6 flex flex-col items-center text-center gap-3">
          <div className="w-14 h-14 rounded-2xl bg-immich-light flex items-center justify-center">
            <Cloud size={28} className="text-immich-primary" />
          </div>
          <div>
            <h2 className="text-base font-semibold text-gray-800">Summit</h2>
            <p className="text-xs text-gray-400 mt-0.5">Version {version}</p>
          </div>
          <p className="text-sm text-gray-500 max-w-xs">
            A Windows desktop sync client for self-hosted Immich. Keeps your local folders
            and your Immich library in sync — automatically, in the background.
          </p>
          <div className="text-xs text-gray-400 pt-2 border-t border-gray-100 w-full">
            Built with Tauri v2 · Rust · React · TypeScript
          </div>
        </div>
      </div>
    </div>
  )
}

function Toggle({
  label, description, checked, onToggle,
}: {
  label: string
  description: string
  checked: boolean
  onToggle: () => void
}) {
  return (
    <div className="flex items-start justify-between gap-4">
      <div>
        <p className="text-sm font-medium text-gray-700">{label}</p>
        <p className="text-xs text-gray-500 mt-0.5">{description}</p>
      </div>
      <button
        type="button"
        onClick={onToggle}
        className={`relative flex-shrink-0 w-10 h-6 rounded-full transition-colors ${
          checked ? 'bg-immich-primary' : 'bg-gray-300'
        }`}
      >
        <span
          className={`absolute top-1 w-4 h-4 bg-white rounded-full shadow transition-transform ${
            checked ? 'translate-x-5' : 'translate-x-1'
          }`}
        />
      </button>
    </div>
  )
}
