; Tauri NSIS installer hooks for Immich Desktop.
;
; NSIS_HOOK_PREINSTALL runs before files are copied.
; Removes any stale Start Menu shortcuts from previous installs so ghost
; entries don't accumulate if the user installed to a different path.
;
; NSIS_HOOK_PREUNINSTALL runs before files are deleted.
; 1. Calls the app with --unregister-shell to remove COM shell-extension and
;    SyncRootManager registry entries so Explorer stops referencing deleted files.
; 2. Removes the MSIX sparse package so the Start Menu entry is cleaned up.

!macro NSIS_HOOK_PREINSTALL
  ; Remove stale Start Menu shortcuts — covers both per-user and all-users locations.
  Delete "$SMPROGRAMS\Immich Desktop.lnk"
  Delete "$SMPROGRAMS\Immich Desktop\Immich Desktop.lnk"
  RMDir  "$SMPROGRAMS\Immich Desktop"
  Delete "$SMSTARTUP\Immich Desktop.lnk"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Trust the self-signed cert so Windows accepts the sparse MSIX.
  ExecWait 'certutil -addstore -f Root "$INSTDIR\ImmichDesktop.cer"'
  ; Register the sparse MSIX so WRT StorageProviderSyncRootManager::Register()
  ; gets a package identity and writes the correct SyncRootManager key name.
  ExecWait 'powershell.exe -WindowStyle Hidden -Command "Add-AppxPackage -Path \"$INSTDIR\sparse.msix\" -ExternalLocation \"$INSTDIR\" -ForceApplicationShutdown"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\tauri-app.exe" --unregister-shell'
  ExecWait 'powershell.exe -WindowStyle Hidden -Command "Get-AppxPackage *ImmichDesktop* | Remove-AppxPackage"'
!macroend
