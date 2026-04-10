import { useState } from 'react'
import { Plus, Trash2, Check, Pencil } from 'lucide-react'
import { AccountProfile, AppConfig, ipc } from '../../lib/ipc'
import AddAccountModal from './AddAccountModal'
import EditAccountModal from './EditAccountModal'

interface Props {
  config: AppConfig
  onConfigChange: (c: AppConfig) => void
}

export default function AccountList({ config, onConfigChange }: Props) {
  const [showModal, setShowModal] = useState(false)
  const [editingProfile, setEditingProfile] = useState<AccountProfile | null>(null)

  const handleAdded = (profile: AccountProfile) => {
    const updated: AppConfig = {
      ...config,
      profiles: [...config.profiles, profile],
      activeProfileId: config.profiles.length === 0 ? profile.id : config.activeProfileId,
    }
    onConfigChange(updated)
    setShowModal(false)
  }

  const handleSetActive = async (id: string) => {
    await ipc.setActiveProfile(id)
    onConfigChange({ ...config, activeProfileId: id })
  }

  const handleSaved = (updated: AccountProfile) => {
    onConfigChange({
      ...config,
      profiles: config.profiles.map((p) => (p.id === updated.id ? updated : p)),
    })
    setEditingProfile(null)
  }

  const handleDelete = async (id: string) => {
    await ipc.deleteProfile(id)
    const profiles = config.profiles.filter((p) => p.id !== id)
    const activeProfileId =
      config.activeProfileId === id
        ? (profiles[0]?.id ?? '')
        : config.activeProfileId
    onConfigChange({ ...config, profiles, activeProfileId })
  }

  return (
    <div className="space-y-3">
      {config.profiles.length === 0 ? (
        <div className="rounded-lg border border-dashed border-gray-300 p-6 text-center">
          <p className="text-sm text-gray-500">No accounts added yet.</p>
          <p className="text-xs text-gray-400 mt-1">
            Add an account to start syncing with your Immich server.
          </p>
        </div>
      ) : (
        config.profiles.map((profile) => (
          <ProfileRow
            key={profile.id}
            profile={profile}
            isActive={profile.id === config.activeProfileId}
            onSetActive={() => handleSetActive(profile.id)}
            onEdit={() => setEditingProfile(profile)}
            onDelete={() => handleDelete(profile.id)}
          />
        ))
      )}

      <button
        onClick={() => setShowModal(true)}
        className="flex items-center gap-2 w-full px-3 py-2 border border-dashed border-gray-300 rounded-lg text-sm text-gray-500 hover:text-gray-700 hover:border-gray-400 transition-colors"
      >
        <Plus size={15} />
        Add Account
      </button>

      {showModal && (
        <AddAccountModal onClose={() => setShowModal(false)} onAdded={handleAdded} />
      )}

      {editingProfile && (
        <EditAccountModal
          profile={editingProfile}
          onClose={() => setEditingProfile(null)}
          onSaved={handleSaved}
        />
      )}
    </div>
  )
}

function ProfileRow({
  profile,
  isActive,
  onSetActive,
  onEdit,
  onDelete,
}: {
  profile: AccountProfile
  isActive: boolean
  onSetActive: () => void
  onEdit: () => void
  onDelete: () => void
}) {
  const hasLocal = Boolean(profile.localUrl)

  return (
    <div
      className={`flex items-center gap-3 p-3 rounded-lg border ${
        isActive ? 'border-immich-primary bg-immich-light' : 'border-gray-200'
      }`}
    >
      {/* Avatar initial */}
      <div className="w-9 h-9 rounded-full bg-immich-primary flex items-center justify-center text-white text-sm font-semibold flex-shrink-0">
        {profile.displayName.charAt(0).toUpperCase()}
      </div>

      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-gray-800 truncate">{profile.displayName}</p>
        <p className="text-xs text-gray-500 truncate">{profile.email}</p>
        <div className="flex items-center gap-2 mt-0.5">
          <span className="text-xs text-gray-400 truncate">
            {profile.remoteUrl || profile.localUrl}
          </span>
          {hasLocal && (
            <span className="text-xs bg-blue-100 text-blue-600 px-1.5 py-0.5 rounded">
              +Local
            </span>
          )}
        </div>
      </div>

      <div className="flex items-center gap-1 flex-shrink-0">
        {!isActive && (
          <button
            onClick={onSetActive}
            title="Set as active account"
            className="p-1.5 text-gray-400 hover:text-immich-primary hover:bg-gray-100 rounded transition-colors"
          >
            <Check size={15} />
          </button>
        )}
        {isActive && (
          <span className="text-xs font-medium text-immich-primary px-2">Active</span>
        )}
        <button
          onClick={onEdit}
          title="Edit account"
          className="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded transition-colors"
        >
          <Pencil size={15} />
        </button>
        <button
          onClick={onDelete}
          title="Remove account"
          className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded transition-colors"
        >
          <Trash2 size={15} />
        </button>
      </div>
    </div>
  )
}
