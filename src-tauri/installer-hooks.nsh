; Tauri NSIS installer hooks for Harmonia.
;
; The standard NSIS template already creates the Start Menu shortcut and the
; uninstaller. These hooks additionally place a desktop shortcut so the app is
; one click away. All macros must be defined; the ones we don't use are empty.

!macro preInit
!macroend

!macro postInit
!macroend

!macro preInstall
!macroend

!macro postInstall
  CreateShortcut "$DESKTOP\Harmonia.lnk" "$INSTDIR\Harmonia.exe"
!macroend

!macro preUnInstall
!macroend

!macro postUnInstall
!macroend
