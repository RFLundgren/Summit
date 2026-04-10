import { useEffect, useState } from 'react'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { isEnabled, enable, disable } from '@tauri-apps/plugin-autostart'
import { X } from 'lucide-react'
import { useSettingsStore } from '../stores/settingsStore'
import { ipc } from '../lib/ipc'
import AccountList from '../components/settings/AccountList'
import AppSettingsTab from '../components/settings/AppSettingsTab'
import FolderSettings from '../components/settings/FolderSettings'

type Tab = 'accounts' | 'folders' | 'sync' | 'app'

export default function Settings() {
  const { config, loaded, load, setConfig } = useSettingsStore()
  const [tab, setTab] = useState<Tab>('accounts')
  const [syncInterval, setSyncInterval] = useState(300)
  const [autostart, setAutostart] = useState(false)
  const [notifications, setNotifications] = useState(true)
  const [saving, setSaving] = useState(false)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    load()
  }, [])

  useEffect(() => {
    if (loaded) {
      setSyncInterval(config.syncIntervalSecs)
      setAutostart(config.autostart)
      setNotifications(config.notificationsEnabled)
    }
  }, [loaded])

  const handleSaveApp = async () => {
    setSaving(true)
    try {
      await ipc.saveAppConfig({
        syncIntervalSecs: syncInterval,
        autostart,
        notificationsEnabled: notifications,
      })
      try {
        const currently = await isEnabled()
        if (autostart && !currently) await enable()
        if (!autostart && currently) await disable()
      } catch (_) {}
      setConfig({ ...config, syncIntervalSecs: syncInterval, autostart, notificationsEnabled: notifications })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } finally {
      setSaving(false)
    }
  }

  const closeWindow = () => getCurrentWindow().hide()

  const fakeSettings = {
    serverUrl: '', apiKey: '', uploadFolders: [], downloadFolder: '',
    defaultSyncMode: 'cloud_and_local' as const, syncIntervalSecs: syncInterval,
    autostart, notificationsEnabled: notifications,
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: 'accounts', label: 'Accounts' },
    { id: 'folders', label: 'Folders' },
    { id: 'sync', label: 'Sync' },
    { id: 'app', label: 'App' },
  ]

  return (
    <div className="flex flex-col h-screen bg-white">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200">
        <span className="font-semibold text-gray-800">Settings</span>
        <button onClick={closeWindow} className="text-gray-400 hover:text-gray-600">
          <X size={18} />
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-gray-200 px-5">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`px-4 py-3 text-sm font-medium border-b-2 transition-colors ${
              tab === t.id
                ? 'border-immich-primary text-immich-primary'
                : 'border-transparent text-gray-500 hover:text-gray-700'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-5 py-5">
        {!loaded ? (
          <div className="text-sm text-gray-400">Loading…</div>
        ) : tab === 'accounts' ? (
          <AccountList config={config} onConfigChange={setConfig} />
        ) : tab === 'folders' ? (
          <FolderSettings config={config} onConfigChange={setConfig} />
        ) : tab === 'sync' ? (
          <SyncTab interval={syncInterval} onChange={setSyncInterval} />
        ) : (
          <AppSettingsTab
            settings={fakeSettings}
            onChange={(s) => {
              setAutostart(s.autostart)
              setNotifications(s.notificationsEnabled)
            }}
          />
        )}
      </div>

      {/* Footer — only for Sync and App tabs */}
      {tab !== 'accounts' && tab !== 'folders' && (
        <div className="flex items-center justify-end gap-3 px-5 py-4 border-t border-gray-200 bg-gray-50">
          {saved && <span className="text-sm text-green-600 font-medium">Saved!</span>}
          <button
            onClick={handleSaveApp}
            disabled={saving}
            className="px-4 py-2 text-sm font-medium text-white bg-immich-primary hover:bg-immich-hover disabled:opacity-50 rounded-lg"
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      )}
    </div>
  )
}

function SyncTab({
  interval,
  onChange,
}: {
  interval: number
  onChange: (v: number) => void
}) {
  const options = [
    { label: '1 minute', value: 60 },
    { label: '5 minutes', value: 300 },
    { label: '15 minutes', value: 900 },
    { label: '30 minutes', value: 1800 },
    { label: '1 hour', value: 3600 },
  ]

  return (
    <div className="space-y-5">
      <div>
        <label className="block text-sm font-medium text-gray-700 mb-2">
          Sync interval
        </label>
        <div className="space-y-1">
          {options.map((o) => (
            <label key={o.value} className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="interval"
                value={o.value}
                checked={interval === o.value}
                onChange={() => onChange(o.value)}
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
      </div>
    </div>
  )
}
