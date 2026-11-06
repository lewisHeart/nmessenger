Unicode true
!include "MUI2.nsh"
!include "nsDialogs.nsh"
!include "LogicLib.nsh"

Name "NMessenger ${VERSION}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\NMessenger"
RequestExecutionLevel user

!define MUI_ABORTWARNING
!define MUI_ICON "${PROJECT_ROOT}\packaging\windows\icon.ico"
!define MUI_UNICON "${PROJECT_ROOT}\packaging\windows\icon.ico"

; Variables for user options
Var Dialog
Var ChkDesktop
Var ChkStartMenu
Var ChkStartup

Var OptDesktop
Var OptStartMenu
Var OptStartup

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PROJECT_ROOT}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY

Page custom TasksPageCreate TasksPageLeave

!insertmacro MUI_PAGE_INSTFILES

!define MUI_FINISHPAGE_RUN "$INSTDIR\nmessenger.exe"
!define MUI_FINISHPAGE_RUN_TEXT $(FinishRunText)
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages
!insertmacro MUI_LANGUAGE "PortugueseBR"
!insertmacro MUI_LANGUAGE "English"

; Localized Strings
LangString TasksTitle ${LANG_PORTUGUESEBR} "Tarefas Adicionais"
LangString TasksTitle ${LANG_ENGLISH} "Additional Tasks"

LangString TasksSubtitle ${LANG_PORTUGUESEBR} "Selecione as opções adicionais para configurar atalhos e inicialização."
LangString TasksSubtitle ${LANG_ENGLISH} "Select additional options to configure shortcuts and startup."

LangString TasksHeader ${LANG_PORTUGUESEBR} "Quais tarefas adicionais você gostaria que o instalador realizasse?"
LangString TasksHeader ${LANG_ENGLISH} "Which additional tasks would you like Setup to perform?"

LangString ChkDesktopText ${LANG_PORTUGUESEBR} "Criar um atalho na Área de Trabalho"
LangString ChkDesktopText ${LANG_ENGLISH} "Create a Desktop shortcut"

LangString ChkStartMenuText ${LANG_PORTUGUESEBR} "Criar atalho no Menu Iniciar"
LangString ChkStartMenuText ${LANG_ENGLISH} "Create a Start Menu shortcut"

LangString ChkStartupText ${LANG_PORTUGUESEBR} "Iniciar o NMessenger automaticamente com o Windows"
LangString ChkStartupText ${LANG_ENGLISH} "Start NMessenger automatically with Windows"

LangString FinishRunText ${LANG_PORTUGUESEBR} "Executar o NMessenger agora"
LangString FinishRunText ${LANG_ENGLISH} "Launch NMessenger now"

Function .onInit
  ; Defaults: Desktop shortcut enabled, Start Menu shortcut enabled, Auto-start disabled
  StrCpy $OptDesktop "1"
  StrCpy $OptStartMenu "1"
  StrCpy $OptStartup "0"
FunctionEnd

Function TasksPageCreate
  !insertmacro MUI_HEADER_TEXT $(TasksTitle) $(TasksSubtitle)
  nsDialogs::Create 1018
  Pop $Dialog
  ${If} $Dialog == error
    Abort
  ${EndIf}

  ${NSD_CreateLabel} 0 0 100% 24u $(TasksHeader)
  Pop $0

  ${NSD_CreateCheckbox} 10u 26u 90% 12u $(ChkDesktopText)
  Pop $ChkDesktop
  ${If} $OptDesktop == "1"
    ${NSD_Check} $ChkDesktop
  ${EndIf}

  ${NSD_CreateCheckbox} 10u 42u 90% 12u $(ChkStartMenuText)
  Pop $ChkStartMenu
  ${If} $OptStartMenu == "1"
    ${NSD_Check} $ChkStartMenu
  ${EndIf}

  ${NSD_CreateCheckbox} 10u 58u 90% 12u $(ChkStartupText)
  Pop $ChkStartup
  ${If} $OptStartup == "1"
    ${NSD_Check} $ChkStartup
  ${EndIf}

  nsDialogs::Show
FunctionEnd

Function TasksPageLeave
  ${NSD_GetState} $ChkDesktop $OptDesktop
  ${NSD_GetState} $ChkStartMenu $OptStartMenu
  ${NSD_GetState} $ChkStartup $OptStartup
FunctionEnd

Section "Install"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  
  File /oname=nmessenger.exe "${BINARY}"
  File /oname=LICENSE "${PROJECT_ROOT}\LICENSE"
  File /oname=README.md "${PROJECT_ROOT}\README.md"
  
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  
  ; Register in Windows Programs & Features
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "DisplayName" "NMessenger"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "Publisher" "NMessenger Project"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "DisplayIcon" "$INSTDIR\nmessenger.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger" "NoRepair" 1

  ; Start Menu Shortcuts
  ${If} $OptStartMenu == 1
    CreateDirectory "$SMPROGRAMS\NMessenger"
    CreateShortCut "$SMPROGRAMS\NMessenger\NMessenger.lnk" "$INSTDIR\nmessenger.exe" "" "$INSTDIR\nmessenger.exe" 0
    CreateShortCut "$SMPROGRAMS\NMessenger\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  ${EndIf}

  ; Desktop Shortcut
  ${If} $OptDesktop == 1
    CreateShortCut "$DESKTOP\NMessenger.lnk" "$INSTDIR\nmessenger.exe" "" "$INSTDIR\nmessenger.exe" 0
  ${EndIf}

  ; Windows Auto-start
  ${If} $OptStartup == 1
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "NMessenger" '"$INSTDIR\nmessenger.exe"'
  ${EndIf}
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  
  ; Remove shortcuts
  Delete "$SMPROGRAMS\NMessenger\NMessenger.lnk"
  Delete "$SMPROGRAMS\NMessenger\Uninstall.lnk"
  RMDir "$SMPROGRAMS\NMessenger"
  Delete "$DESKTOP\NMessenger.lnk"
  
  ; Remove startup entry
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "NMessenger"
  
  ; Remove application files
  Delete "$INSTDIR\nmessenger.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"
  
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\NMessenger"
SectionEnd
