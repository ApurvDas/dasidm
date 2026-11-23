; DasIDM's additions to Tauri's NSIS installer.
;
; Per-user installs live in %LOCALAPPDATA%\DasIDM, next to what DasIDM creates there itself:
; tools\ (yt-dlp, FFmpeg, Deno, downloaded only after the user agrees) and browser\ (the
; Firefox native messaging manifest and the unpacked add-on). Tauri's uninstaller removes only
; the files it installed; this removes DasIDM's own leftovers too.

!macro NSIS_HOOK_POSTUNINSTALL
  ; Updating (a newer installer replacing this one) keeps everything.
  ${If} $UpdateMode <> 1
    ; Firefox can no longer start DasIDM: forget the registration and its manifest.
    DeleteRegKey HKCU "Software\Mozilla\NativeMessagingHosts\dev.dasidm.host"
    RMDir /r "$LOCALAPPDATA\DasIDM\browser"
    ; "Delete the application data" also removes the video tools (about 320 MB).
    ${If} $DeleteAppDataCheckboxState = 1
      RMDir /r "$LOCALAPPDATA\DasIDM\tools"
    ${EndIf}
    ; Only if nothing else is left in it.
    RMDir "$LOCALAPPDATA\DasIDM"
  ${EndIf}
!macroend
