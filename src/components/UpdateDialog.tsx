import { Download, RefreshCw, X } from 'lucide-react'
import { ipc, UpdaterStatus } from '../lib/ipc'

interface Props {
  status: UpdaterStatus
  onDismiss: () => void
}

export function UpdateDialog({ status, onDismiss }: Props) {
  const open = status.state === 'available' || status.state === 'downloading'
  if (!open) return null

  const handleDownload = () => ipc.downloadUpdate().catch(() => {})

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/40" onClick={status.state !== 'downloading' ? onDismiss : undefined} />
      <div className="relative w-[360px] bg-white rounded-xl shadow-2xl border border-gray-200 p-5 flex flex-col gap-4">

        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-2">
            <RefreshCw size={15} className="text-immich-primary shrink-0" />
            <span className="text-sm font-semibold text-gray-900">
              {status.state === 'available'   && 'Update Available'}
              {status.state === 'downloading' && 'Downloading Update…'}
            </span>
          </div>
          {status.state !== 'downloading' && (
            <button
              onClick={onDismiss}
              className="text-gray-400 hover:text-gray-600 p-1 rounded hover:bg-gray-100 transition-colors shrink-0"
            >
              <X size={13} />
            </button>
          )}
        </div>

        <div className="text-xs text-gray-500 leading-relaxed">
          {status.state === 'available' && (
            <>
              Summit{' '}
              <span className="font-medium text-gray-900">v{status.version}</span>{' '}
              is available. Would you like to download and install it now?
            </>
          )}
          {status.state === 'downloading' && (
            <div className="flex flex-col gap-2">
              <span>Downloading v{status.version}…</span>
              <div className="h-1.5 rounded-full bg-gray-100 overflow-hidden">
                <div
                  className="h-full bg-immich-primary transition-all duration-300 rounded-full"
                  style={{ width: `${status.percent ?? 0}%` }}
                />
              </div>
              <span className="text-gray-400">{status.percent ?? 0}%</span>
            </div>
          )}
        </div>

        {status.state === 'available' && (
          <div className="flex items-center gap-2 justify-end">
            <button
              onClick={onDismiss}
              className="px-3 py-1.5 text-xs text-gray-500 hover:text-gray-700 rounded-md hover:bg-gray-100 transition-colors"
            >
              Skip for now
            </button>
            <button
              onClick={handleDownload}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-immich-primary hover:bg-immich-primary/90 text-white rounded-md transition-colors"
            >
              <Download size={12} />
              Download &amp; Install
            </button>
          </div>
        )}

      </div>
    </div>
  )
}
