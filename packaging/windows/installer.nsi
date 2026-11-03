Unicode true
!include "MUI2.nsh"

Name "NMessenger ${VERSION}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\NMessenger"
RequestExecutionLevel user

!define MUI_ABORTWARNING
!ifmacrodef MUI_ICON
!endif

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PROJECT_ROOT}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "PortugueseBR"

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  
  File /oname=nmessenger.exe "${BINARY}"
  File /oname=LICENSE "${PROJECT_ROOT}\LICENSE"
  File /oname=README.md "${PROJECT_ROOT}\README.md"
  
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "DisplayName" "NMessenger"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "Publisher" "NMessenger Project"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "DisplayIcon" "$INSTDIR\nmessenger.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "NoRepair" 1
  
  CreateDirectory "$SMPROGRAMS\NMessenger"
  CreateShortCut "$SMPROGRAMS\NMessenger\NMessenger.lnk" "$INSTDIR\nmessenger.exe" "" "$INSTDIR\nmessenger.exe" 0
  CreateShortCut "$SMPROGRAMS\NMessenger\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortCut "$DESKTOP\NMessenger.lnk" "$INSTDIR\nmessenger.exe" "" "$INSTDIR\nmessenger.exe" 0
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  
  Delete "$SMPROGRAMS\NMessenger\NMessenger.lnk"
  Delete "$SMPROGRAMS\NMessenger\Uninstall.lnk"
  RMDir "$SMPROGRAMS\NMessenger"
  Delete "$DESKTOP\NMessenger.lnk"
  
  Delete "$INSTDIR\nmessenger.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"
  
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger"
SectionEnd
