Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
Name "My Rivian Data"
OutFile "${OUTPUT}"
RequestExecutionLevel user
CRCCheck force
SetCompressor /SOLID lzma
InstallDir "$LOCALAPPDATA\My Rivian Data\versions\${VERSION}-${REVISION}"
!define REGKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\MyRivianData-${VERSION}-${REVISION}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\my-rivian-data.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open My Rivian Data"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${SOURCE}\LICENSE"
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!ifdef SIGNER
  !uninstfinalize '"${NODE}" "${SIGNER}" "%1"'
!endif

Function .onInit
  SetShellVarContext current
  IfFileExists "$INSTDIR\my-rivian-data.exe" 0 new_install
    MessageBox MB_OK|MB_ICONSTOP "This exact build is already installed. Close the app and uninstall that build before reinstalling." /SD IDOK
    SetErrorLevel 10
    Abort
  new_install:
FunctionEnd

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "${SOURCE}\*"
  ; Run this bundled verifier under a process-only policy. Group Policy still
  ; applies; no user/machine policy is written and every payload hash is checked.
  nsExec::ExecToStack '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\verify.ps1"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    FileOpen $2 "$TEMP\my-rivian-data-${VERSION}-${REVISION}-verify.log" w
    FileWrite $2 "$1$\r$\n"
    FileClose $2
    RMDir /r "$INSTDIR"
    MessageBox MB_OK|MB_ICONSTOP "Installed payload integrity check failed. The previous build remains available." /SD IDOK
    SetErrorLevel 11
    Abort
  ${EndIf}
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\My Rivian Data"
  CreateShortCut "$SMPROGRAMS\My Rivian Data\My Rivian Data.lnk" "$INSTDIR\my-rivian-data.exe"
  CreateShortCut "$SMPROGRAMS\My Rivian Data\My Rivian Data ${VERSION}-${REVISION}.lnk" "$INSTDIR\my-rivian-data.exe"
  CreateShortCut "$SMPROGRAMS\My Rivian Data\Uninstall ${VERSION}-${REVISION}.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "${REGKEY}" "DisplayName" "My Rivian Data ${VERSION}-${REVISION}"
  WriteRegStr HKCU "${REGKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${REGKEY}" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
  WriteRegStr HKCU "${REGKEY}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKCU "${REGKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${REGKEY}" "NoRepair" 1
  WriteRegStr HKCU "Software\MyRivianData" "CurrentDirectory" "$INSTDIR"
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  ReadRegStr $0 HKCU "Software\MyRivianData" "CurrentDirectory"
  ${If} $0 == $INSTDIR
    Delete "$SMPROGRAMS\My Rivian Data\My Rivian Data.lnk"
    DeleteRegKey HKCU "Software\MyRivianData"
  ${EndIf}
  Delete "$SMPROGRAMS\My Rivian Data\My Rivian Data ${VERSION}-${REVISION}.lnk"
  Delete "$SMPROGRAMS\My Rivian Data\Uninstall ${VERSION}-${REVISION}.lnk"
  RMDir "$SMPROGRAMS\My Rivian Data"
  DeleteRegKey HKCU "${REGKEY}"
  RMDir /r "$INSTDIR"
  IfFileExists "$INSTDIR\my-rivian-data.exe" 0 uninstall_complete
    MessageBox MB_OK|MB_ICONEXCLAMATION "Close the running app, then remove this build's remaining folder. Other builds were retained." /SD IDOK
    SetErrorLevel 12
  uninstall_complete:
SectionEnd
