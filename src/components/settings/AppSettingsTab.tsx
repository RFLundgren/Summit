interface AppSettings {
  autostart: boolean
  notificationsEnabled: boolean
  [key: string]: unknown
}

interface Props {
  settings: AppSettings
  onChange: (s: AppSettings) => void
}

function Toggle({
  label,
  description,
  checked,
  onToggle,
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

export default function AppSettingsTab({ settings, onChange }: Props) {
  return (
    <div className="space-y-5">
      <Toggle
        label="Launch at startup"
        description="Start Immich Desktop automatically when you log in to Windows."
        checked={settings.autostart}
        onToggle={() => onChange({ ...settings, autostart: !settings.autostart })}
      />
      <div className="border-t border-gray-100" />
      <Toggle
        label="Show notifications"
        description="Notify when syncs complete, conflicts occur, or errors are detected."
        checked={settings.notificationsEnabled}
        onToggle={() =>
          onChange({ ...settings, notificationsEnabled: !settings.notificationsEnabled })
        }
      />
    </div>
  )
}
