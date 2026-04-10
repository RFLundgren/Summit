import { create } from 'zustand'
import { ipc, AppConfig } from '../lib/ipc'

interface SettingsStore {
  config: AppConfig
  loaded: boolean
  load: () => Promise<void>
  setConfig: (c: AppConfig) => void
}

const defaultConfig: AppConfig = {
  activeProfileId: '',
  profiles: [],
  syncIntervalSecs: 300,
  autostart: false,
  notificationsEnabled: true,
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  config: defaultConfig,
  loaded: false,

  load: async () => {
    const config = await ipc.getConfig()
    set({ config, loaded: true })
  },

  setConfig: (config) => set({ config }),
}))
