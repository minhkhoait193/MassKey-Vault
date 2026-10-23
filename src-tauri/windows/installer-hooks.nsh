; Create a desktop shortcut for an easy one-click launch after installation.
!macro NSIS_HOOK_POSTINSTALL
  CreateShortCut "$DESKTOP\MassKey Vault.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Delete "$DESKTOP\MassKey Vault.lnk"
!macroend
