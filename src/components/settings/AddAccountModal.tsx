import { useState } from 'react'
import { X, Loader, Search, ChevronDown, ChevronUp } from 'lucide-react'
import { ipc, AccountProfile } from '../../lib/ipc'

interface Props {
  onClose: () => void
  onAdded: (profile: AccountProfile) => void
}

export default function AddAccountModal({ onClose, onAdded }: Props) {
  const [remoteUrl, setRemoteUrl] = useState('')
  const [localUrl, setLocalUrl] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [loading, setLoading] = useState(false)
  const [discovering, setDiscovering] = useState(false)
  const [discovered, setDiscovered] = useState<string[]>([])
  const [error, setError] = useState('')

  const handleLogin = async () => {
    if (!remoteUrl && !localUrl) {
      setError('Please enter a server URL.')
      return
    }
    if (!email || !password) {
      setError('Please enter your email and password.')
      return
    }
    setLoading(true)
    setError('')
    try {
      const profile = await ipc.loginAccount({
        localUrl: localUrl.trim(),
        remoteUrl: remoteUrl.trim(),
        email: email.trim(),
        password,
      })
      onAdded(profile)
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }

  const handleDiscover = async () => {
    setDiscovering(true)
    setDiscovered([])
    try {
      const found = await ipc.discoverServers()
      setDiscovered(found)
      if (found.length === 0) setError('No Immich servers found on the local network.')
    } catch {
      setError('Discovery failed.')
    } finally {
      setDiscovering(false)
    }
  }

  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50 p-4">
      <div className="bg-white rounded-xl shadow-xl w-full max-w-md">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-200">
          <h2 className="font-semibold text-gray-800">Add Account</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-gray-600">
            <X size={18} />
          </button>
        </div>

        <div className="p-5 space-y-4">
          {/* Remote URL */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">
              Server URL <span className="text-gray-400 font-normal">(remote / internet)</span>
            </label>
            <div className="flex gap-2">
              <input
                type="url"
                value={remoteUrl}
                onChange={(e) => setRemoteUrl(e.target.value)}
                placeholder="https://photos.yourdomain.com"
                className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-immich-primary"
              />
            </div>
          </div>

          {/* Advanced: local URL */}
          <div>
            <button
              type="button"
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="flex items-center gap-1 text-xs text-gray-500 hover:text-gray-700"
            >
              {showAdvanced ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
              Local network URL (optional — enables faster sync at home)
            </button>

            {showAdvanced && (
              <div className="mt-2 space-y-2">
                <div className="flex gap-2">
                  <input
                    type="url"
                    value={localUrl}
                    onChange={(e) => setLocalUrl(e.target.value)}
                    placeholder="http://192.168.1.50:2283"
                    className="flex-1 px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-immich-primary"
                  />
                  <button
                    type="button"
                    onClick={handleDiscover}
                    disabled={discovering}
                    title="Search for Immich on local network"
                    className="px-3 py-2 border border-gray-300 rounded-lg text-gray-600 hover:bg-gray-50 disabled:opacity-50 flex items-center gap-1 text-sm"
                  >
                    {discovering ? (
                      <Loader size={14} className="animate-spin" />
                    ) : (
                      <Search size={14} />
                    )}
                    {discovering ? 'Scanning…' : 'Find'}
                  </button>
                </div>

                {discovered.length > 0 && (
                  <div className="rounded-lg border border-gray-200 divide-y">
                    {discovered.map((url) => (
                      <button
                        key={url}
                        type="button"
                        onClick={() => setLocalUrl(url)}
                        className="w-full text-left px-3 py-2 text-sm text-gray-700 hover:bg-gray-50"
                      >
                        {url}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Credentials */}
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Email</label>
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-immich-primary"
            />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleLogin()}
              placeholder="••••••••"
              className="w-full px-3 py-2 border border-gray-300 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-immich-primary"
            />
          </div>

          {error && (
            <p className="text-sm text-red-600 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
              {error}
            </p>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 px-5 py-4 border-t border-gray-200 bg-gray-50 rounded-b-xl">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-gray-600 hover:text-gray-800"
          >
            Cancel
          </button>
          <button
            onClick={handleLogin}
            disabled={loading}
            className="px-4 py-2 text-sm font-medium text-white bg-immich-primary hover:bg-immich-hover disabled:opacity-50 rounded-lg flex items-center gap-2"
          >
            {loading && <Loader size={14} className="animate-spin" />}
            {loading ? 'Signing in…' : 'Sign In'}
          </button>
        </div>
      </div>
    </div>
  )
}
