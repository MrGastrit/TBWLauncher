; Custom NSIS hooks for TBW Launcher.
; Removes launcher runtime content from %APPDATA%\.tbw on uninstall.

!macro NSIS_HOOK_POSTUNINSTALL
  RMDir /r "$APPDATA\.tbw"
!macroend
