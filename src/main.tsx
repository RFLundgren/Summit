import React from 'react'
import ReactDOM from 'react-dom/client'
import { getCurrentWindow } from '@tauri-apps/api/window'
import Dashboard from './pages/Dashboard'
import Settings from './pages/Settings'
import Help from './pages/Help'
import './index.css'

const label = getCurrentWindow().label
const Page = label === 'settings' ? Settings : label === 'help' ? Help : Dashboard

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Page />
  </React.StrictMode>
)
