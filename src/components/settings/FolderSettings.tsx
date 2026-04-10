import { useState } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderOpen, X, Plus, Upload, Download, Cloud, Loader } from 'lucide-react'
import { AccountProfile, AppConfig, ipc } from '../../lib/ipc'

interface Props {
  config: AppConfig
  onConfigChange: (c: AppConfig) => void
}

export default function FolderSettings({ config, onConfigChange }: Props) {
  const [selectedId, setSelectedId] = useState(config.activeProfileId)

  if (config.profiles.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-gray-300 p-6 text-center">
        <p className="text-sm text-gray-500">No active account.</p>
        <p className="text-xs text-gray-400 mt-1">Add an account in the Accounts tab first.</p>
      </div>
    )
  }

  const profile = config.profiles.find((p) => p.id === selectedId) ?? config.profiles[0]

  return (
    <div className="space-y-5">
      {/* Profile picker — only shown when there are multiple accounts */}
      {config.profiles.length > 1 && (
        <div>
          <label className="text-xs font-medium text-gray-500 mb-1 block">Configure settings for</label>
          <div className="flex flex-wrap gap-2">
            {config.profiles.map((p) => (
              <button
                key={p.id}
                onClick={() => setSelectedId(p.id)}
                className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border text-sm transition-colors ${
                  p.id === profile.id
                    ? 'border-immich-primary bg-immich-light text-immich-primary font-medium'
                    : 'border-gray-200 text-gray-600 hover:border-gray-300 hover:bg-gray-50'
                }`}
              >
                <span className="w-5 h-5 rounded-full bg-immich-primary flex items-center justify-center text-white text-xs font-semibold flex-shrink-0">
                  {p.displayName.charAt(0).toUpperCase()}
                </span>
                {p.displayName}
              </button>
            ))}
          </div>
        </div>
      )}

      <ProfileFolders key={profile.id} profile={profile} config={config} onConfigChange={onConfigChange} />
    </div>
  )
}

function ProfileFolders({
  profile,
  config,
  onConfigChange,
}: {
  profile: AccountProfile
  config: AppConfig
  onConfigChange: (c: AppConfig) => void
}) {
  const [uploadFolders, setUploadFolders] = useState<string[]>(profile.uploadFolders)
  const [downloadFolder, setDownloadFolder] = useState(profile.downloadFolder)
  const [syncMode, setSyncMode] = useState<'cloud_and_local' | 'cloud_only' | 'cloud_browse'>(profile.defaultSyncMode)
  const [duplicateHandling, setDuplicateHandling] = useState<'overwrite' | 'rename' | 'skip'>(
    profile.duplicateHandling ?? 'rename'
  )
  const [saving, setSaving] = useState(false)
  const [saved, setSaved] = useState(false)
  const [error, setError] = useState('')

  const pickUploadFolder = async () => {
    const selected = await open({ directory: true, multiple: false, title: 'Select Upload Folder' })
    if (typeof selected === 'string' && selected && !uploadFolders.includes(selected)) {
      setUploadFolders((prev) => [...prev, selected])
    }
  }

  const pickDownloadFolder = async () => {
    const selected = await open({ directory: true, multiple: false, title: 'Select Download Folder' })
    if (typeof selected === 'string' && selected) {
      setDownloadFolder(selected)
    }
  }

  const handleSave = async () => {
    setSaving(true)
    setError('')
    try {
      await ipc.updateSyncFolders({
        profileId: profile.id,
        uploadFolders,
        downloadFolder,
        syncMode,
        duplicateHandling,
      })
      const updated: AccountProfile = {
        ...profile,
        uploadFolders,
        downloadFolder,
        defaultSyncMode: syncMode,
        duplicateHandling,
      }
      onConfigChange({
        ...config,
        profiles: config.profiles.map((p) => (p.id === profile.id ? updated : p)),
      })
      setSaved(true)
      setTimeout(() => setSaved(false), 2000)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }

  const isDirty =
    JSON.stringify(uploadFolders) !== JSON.stringify(profile.uploadFolders) ||
    downloadFolder !== profile.downloadFolder ||
    syncMode !== profile.defaultSyncMode ||
    (syncMode === 'cloud_and_local' && duplicateHandling !== (profile.duplicateHandling ?? 'rename'))

  return (
    <div className="space-y-5">
      {/* Sync mode */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <Cloud size={14} className="text-gray-500" />
          <label className="text-sm font-medium text-gray-700">Sync mode</label>
        </div>
        <div className="space-y-2">
          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="radio"
              name="syncMode"
              value="cloud_and_local"
              checked={syncMode === 'cloud_and_local'}
              onChange={() => setSyncMode('cloud_and_local')}
              className="accent-immich-primary mt-0.5"
            />
            <div>
              <p className="text-sm text-gray-700 font-medium">Cloud + Local</p>
              <p className="text-xs text-gray-500">Upload local photos to Immich and download Immich photos to your device.</p>
            </div>
          </label>
          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="radio"
              name="syncMode"
              value="cloud_only"
              checked={syncMode === 'cloud_only'}
              onChange={() => setSyncMode('cloud_only')}
              className="accent-immich-primary mt-0.5"
            />
            <div>
              <p className="text-sm text-gray-700 font-medium">Cloud Only</p>
              <p className="text-xs text-gray-500">Upload local photos to Immich only. Nothing is downloaded to this device.</p>
            </div>
          </label>
          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="radio"
              name="syncMode"
              value="cloud_browse"
              checked={syncMode === 'cloud_browse'}
              onChange={() => setSyncMode('cloud_browse')}
              className="accent-immich-primary mt-0.5"
            />
            <div>
              <p className="text-sm text-gray-700 font-medium">Files On-Demand</p>
              <p className="text-xs text-gray-500">
                Placeholder files appear in Explorer for every Immich photo. Files download automatically
                when opened. Right-click to pin locally or free up space — like OneDrive.
              </p>
            </div>
          </label>
        </div>
      </div>

      <div className="border-t border-gray-100" />

      {/* Upload folders */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <Upload size={14} className="text-gray-500" />
          <label className="text-sm font-medium text-gray-700">Upload folders</label>
        </div>
        <p className="text-xs text-gray-500 mb-2">
          Photos in these folders are uploaded to Immich automatically.
        </p>
        <div className="space-y-1.5">
          {uploadFolders.length === 0 ? (
            <div className="rounded-lg border border-dashed border-gray-300 px-3 py-3 text-center">
              <p className="text-xs text-gray-400">No upload folders selected.</p>
            </div>
          ) : (
            uploadFolders.map((folder) => (
              <FolderRow key={folder} path={folder} onRemove={() => setUploadFolders((p) => p.filter((f) => f !== folder))} />
            ))
          )}
        </div>
        <button
          type="button"
          onClick={pickUploadFolder}
          className="mt-2 flex items-center gap-2 px-3 py-2 border border-dashed border-gray-300 rounded-lg text-sm text-gray-500 hover:text-gray-700 hover:border-gray-400 transition-colors w-full"
        >
          <Plus size={14} />
          Add folder
        </button>
      </div>

      {/* Download folder — shown in cloud_and_local and cloud_browse modes */}
      {(syncMode === 'cloud_and_local' || syncMode === 'cloud_browse') && (
        <>
          <div className="border-t border-gray-100" />
          <div>
            <div className="flex items-center gap-2 mb-1">
              <Download size={14} className="text-gray-500" />
              <label className="text-sm font-medium text-gray-700">
                {syncMode === 'cloud_browse' ? 'Sync root folder' : 'Download folder'}
              </label>
            </div>
            <p className="text-xs text-gray-500 mb-2">
              {syncMode === 'cloud_browse'
                ? 'Placeholder files appear here. Open any file to download it on demand.'
                : 'New photos from Immich are saved here.'}
            </p>
            {downloadFolder ? (
              <FolderRow path={downloadFolder} onRemove={() => setDownloadFolder('')} />
            ) : (
              <div className="rounded-lg border border-dashed border-gray-300 px-3 py-3 text-center mb-2">
                <p className="text-xs text-gray-400">No download folder selected.</p>
              </div>
            )}
            <button
              type="button"
              onClick={pickDownloadFolder}
              className="mt-2 flex items-center gap-2 px-3 py-2 border border-gray-300 rounded-lg text-sm text-gray-600 hover:bg-gray-50 transition-colors"
            >
              <FolderOpen size={14} />
              {downloadFolder ? 'Change folder' : 'Select folder'}
            </button>
          </div>

          {syncMode === 'cloud_and_local' && <div className="border-t border-gray-100" />}

          {/* Duplicate handling — cloud_and_local only */}
          {syncMode === 'cloud_and_local' && <div>
            <label className="text-sm font-medium text-gray-700 mb-2 block">
              If a file already exists locally
            </label>
            <div className="space-y-2">
              {(
                [
                  { value: 'rename', label: 'Keep both', desc: 'Rename the downloaded copy with a timestamp suffix.' },
                  { value: 'overwrite', label: 'Overwrite', desc: 'Replace the existing local file with the version from Immich.' },
                  { value: 'skip', label: 'Skip', desc: "Don't download if a file with the same name already exists." },
                ] as const
              ).map(({ value, label, desc }) => (
                <label key={value} className="flex items-start gap-3 cursor-pointer">
                  <input
                    type="radio"
                    name="duplicateHandling"
                    value={value}
                    checked={duplicateHandling === value}
                    onChange={() => setDuplicateHandling(value)}
                    className="accent-immich-primary mt-0.5"
                  />
                  <div>
                    <p className="text-sm text-gray-700 font-medium">{label}</p>
                    <p className="text-xs text-gray-500">{desc}</p>
                  </div>
                </label>
              ))}
            </div>
          </div>}
        </>
      )}

      {error && (
        <p className="text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
          {error}
        </p>
      )}

      {/* Footer */}
      <div className="flex items-center justify-end gap-3 pt-2 border-t border-gray-100">
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

function FolderRow({ path, onRemove }: { path: string; onRemove: () => void }) {
  const parts = path.replace(/\\/g, '/').split('/').filter(Boolean)
  const display = parts.length > 2 ? `…/${parts.slice(-2).join('/')}` : path

  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-gray-50 border border-gray-200 rounded-lg">
      <FolderOpen size={14} className="text-gray-400 flex-shrink-0" />
      <span className="flex-1 text-sm text-gray-700 truncate" title={path}>
        {display}
      </span>
      <button
        type="button"
        onClick={onRemove}
        className="text-gray-300 hover:text-red-500 transition-colors flex-shrink-0"
        title="Remove"
      >
        <X size={14} />
      </button>
    </div>
  )
}
