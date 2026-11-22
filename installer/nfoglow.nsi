; SPDX-License-Identifier: GPL-3.0-or-later
; Copyright (C) 2026 DigitGram
;
; The setup of NfoGlow for Windows. A page asks for whom: all users (C:\Program Files\NfoGlow, the registry entries in
; HKEY_LOCAL_MACHINE; for administrators, chosen at first) or only me (%LOCALAPPDATA%\Programs\NfoGlow,
; HKEY_CURRENT_USER; no administrator needed, like install.sh on Linux). An administrator is asked by Windows once at
; the start. An update keeps the choice of the last install. Removing NfoGlow asks whether the settings in
; %APPDATA%\NfoGlow go too (no by default, like install.sh --uninstall without --purge).
;   makensis -DVERSION=1.0.0 installer/nfoglow.nsi      (after cargo build --release)
;   -DEXE=<path> takes nfoglow.exe from elsewhere (a cross build: target/x86_64-pc-windows-gnu/release/nfoglow.exe)
Unicode true
!ifndef VERSION
  !error "Give the version: makensis -DVERSION=1.0.0 installer/nfoglow.nsi"
!endif
!ifndef EXE
  !define EXE "..\target\release\nfoglow.exe"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\NfoGlow"
!define PROGID "NfoGlow.nfo"
!define TYPES ".nfo .diz .ans .asc .srr"   ; what the arrow keys step through in a folder; a ZIP stays with Explorer

Name "NfoGlow"
OutFile "..\target\NfoGlow-Setup-${VERSION}.exe"
SetCompressor /SOLID lzma
ManifestDPIAware true
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=0 "ProductName" "NfoGlow"
VIAddVersionKey /LANG=0 "FileDescription" "NfoGlow Setup"
VIAddVersionKey /LANG=0 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "CompanyName" "DigitGram"
VIAddVersionKey /LANG=0 "LegalCopyright" "© 2026 DigitGram, GPL-3.0-or-later"

!define MULTIUSER_EXECUTIONLEVEL Highest
!define MULTIUSER_MUI
!define MULTIUSER_INSTALLMODE_COMMANDLINE   ; /AllUsers or /CurrentUser for a silent install
!define MULTIUSER_INSTALLMODE_INSTDIR "NfoGlow"
!define MULTIUSER_USE_PROGRAMFILES64
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_KEY "Software\NfoGlow"
!define MULTIUSER_INSTALLMODE_INSTDIR_REGISTRY_VALUENAME "InstallDir"
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_KEY "Software\NfoGlow"   ; also tells the uninstaller which install it is
!define MULTIUSER_INSTALLMODE_DEFAULT_REGISTRY_VALUENAME "InstallDir"
!include "MultiUser.nsh"
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
!define MUI_ICON "..\assets\nfoglow.ico"
!define MUI_UNICON "..\assets\nfoglow.ico"
!define MUI_COMPONENTSPAGE_NODESC
!define MUI_FINISHPAGE_RUN "$INSTDIR\nfoglow.exe"
!insertmacro MULTIUSER_PAGE_INSTALLMODE
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; the languages of NfoGlow; the setup speaks the one of Windows, English when it has none of them
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "Spanish"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "Italian"
!insertmacro MUI_LANGUAGE "Japanese"
!insertmacro MUI_LANGUAGE "Polish"
!insertmacro MUI_LANGUAGE "PortugueseBR"
!insertmacro MUI_LANGUAGE "Ukrainian"
!insertmacro MUI_LANGUAGE "SimpChinese"

LangString OPEN_WITH ${LANG_ENGLISH} "Open NFO files with NfoGlow (${TYPES})"
LangString OPEN_WITH ${LANG_GERMAN} "NFO-Dateien mit NfoGlow öffnen (${TYPES})"
LangString OPEN_WITH ${LANG_SPANISH} "Abrir los archivos NFO con NfoGlow (${TYPES})"
LangString OPEN_WITH ${LANG_FRENCH} "Ouvrir les fichiers NFO avec NfoGlow (${TYPES})"
LangString OPEN_WITH ${LANG_ITALIAN} "Apri i file NFO con NfoGlow (${TYPES})"
LangString OPEN_WITH ${LANG_JAPANESE} "NFOファイルをNfoGlowで開く (${TYPES})"
LangString OPEN_WITH ${LANG_POLISH} "Otwieraj pliki NFO w NfoGlow (${TYPES})"
LangString OPEN_WITH ${LANG_PORTUGUESEBR} "Abrir arquivos NFO com o NfoGlow (${TYPES})"
LangString OPEN_WITH ${LANG_UKRAINIAN} "Відкривати файли NFO у NfoGlow (${TYPES})"
LangString OPEN_WITH ${LANG_SIMPCHINESE} "用 NfoGlow 打开 NFO 文件 (${TYPES})"
LangString DESKTOP ${LANG_ENGLISH} "Shortcut on the desktop"
LangString DESKTOP ${LANG_GERMAN} "Verknüpfung auf dem Desktop"
LangString DESKTOP ${LANG_SPANISH} "Acceso directo en el escritorio"
LangString DESKTOP ${LANG_FRENCH} "Raccourci sur le bureau"
LangString DESKTOP ${LANG_ITALIAN} "Collegamento sul desktop"
LangString DESKTOP ${LANG_JAPANESE} "デスクトップにショートカット"
LangString DESKTOP ${LANG_POLISH} "Skrót na pulpicie"
LangString DESKTOP ${LANG_PORTUGUESEBR} "Atalho na área de trabalho"
LangString DESKTOP ${LANG_UKRAINIAN} "Ярлик на робочому столі"
LangString DESKTOP ${LANG_SIMPCHINESE} "桌面快捷方式"
LangString FILE_TYPE ${LANG_ENGLISH} "NFO file"
LangString FILE_TYPE ${LANG_GERMAN} "NFO-Datei"
LangString FILE_TYPE ${LANG_SPANISH} "Archivo NFO"
LangString FILE_TYPE ${LANG_FRENCH} "Fichier NFO"
LangString FILE_TYPE ${LANG_ITALIAN} "File NFO"
LangString FILE_TYPE ${LANG_JAPANESE} "NFOファイル"
LangString FILE_TYPE ${LANG_POLISH} "Plik NFO"
LangString FILE_TYPE ${LANG_PORTUGUESEBR} "Arquivo NFO"
LangString FILE_TYPE ${LANG_UKRAINIAN} "Файл NFO"
LangString FILE_TYPE ${LANG_SIMPCHINESE} "NFO 文件"

LangString PURGE ${LANG_ENGLISH} "Also delete your NfoGlow settings (theme, effects, recent files)?"
LangString PURGE ${LANG_GERMAN} "Auch Ihre NfoGlow-Einstellungen löschen (Farbschema, Effekte, zuletzt geöffnete Dateien)?"
LangString PURGE ${LANG_SPANISH} "¿Eliminar también la configuración de NfoGlow (tema, efectos, archivos recientes)?"
LangString PURGE ${LANG_FRENCH} "Supprimer aussi les réglages de NfoGlow (thème, effets, fichiers récents) ?"
LangString PURGE ${LANG_ITALIAN} "Eliminare anche le impostazioni di NfoGlow (tema, effetti, file recenti)?"
LangString PURGE ${LANG_JAPANESE} "NfoGlowの設定（テーマ、エフェクト、最近使ったファイル）も削除しますか？"
LangString PURGE ${LANG_POLISH} "Usunąć również ustawienia NfoGlow (motyw, efekty, ostatnie pliki)?"
LangString PURGE ${LANG_PORTUGUESEBR} "Excluir também as configurações do NfoGlow (tema, efeitos, arquivos recentes)?"
LangString PURGE ${LANG_UKRAINIAN} "Також видалити налаштування NfoGlow (тему, ефекти, нещодавні файли)?"
LangString PURGE ${LANG_SIMPCHINESE} "同时删除 NfoGlow 的设置（主题、效果、最近使用的文件）吗？"

LangString CLOSE_APP ${LANG_ENGLISH} "NfoGlow is still running. Close all NfoGlow windows, then click Retry."
LangString CLOSE_APP ${LANG_GERMAN} "NfoGlow läuft noch. Schließen Sie alle NfoGlow-Fenster und klicken Sie dann auf Wiederholen."
LangString CLOSE_APP ${LANG_SPANISH} "NfoGlow sigue en ejecución. Cierre todas las ventanas de NfoGlow y pulse Reintentar."
LangString CLOSE_APP ${LANG_FRENCH} "NfoGlow est toujours en cours d'exécution. Fermez toutes les fenêtres de NfoGlow, puis cliquez sur Réessayer."
LangString CLOSE_APP ${LANG_ITALIAN} "NfoGlow è ancora in esecuzione. Chiudi tutte le finestre di NfoGlow, poi fai clic su Riprova."
LangString CLOSE_APP ${LANG_JAPANESE} "NfoGlowが実行中です。すべてのNfoGlowのウィンドウを閉じてから、［再試行］をクリックしてください。"
LangString CLOSE_APP ${LANG_POLISH} "NfoGlow jest nadal uruchomiony. Zamknij wszystkie okna NfoGlow, a następnie kliknij Ponów."
LangString CLOSE_APP ${LANG_PORTUGUESEBR} "O NfoGlow ainda está em execução. Feche todas as janelas do NfoGlow e clique em Tentar novamente."
LangString CLOSE_APP ${LANG_UKRAINIAN} "NfoGlow усе ще запущено. Закрийте всі вікна NfoGlow і натисніть «Повторити»."
LangString CLOSE_APP ${LANG_SIMPCHINESE} "NfoGlow 仍在运行。请关闭所有 NfoGlow 窗口，然后单击“重试”。"

; one registry change for each file type: ${TYPES} has no loop in NSIS, so the macro names them
!macro ForEachType DO
  !insertmacro ${DO} ".nfo"
  !insertmacro ${DO} ".diz"
  !insertmacro ${DO} ".ans"
  !insertmacro ${DO} ".asc"
  !insertmacro ${DO} ".srr"
!macroend
!macro SupportedType EXT
  WriteRegStr SHCTX "Software\Classes\Applications\nfoglow.exe\SupportedTypes" "${EXT}" ""
!macroend
!macro Associate EXT
  WriteRegStr SHCTX "Software\Classes\${EXT}\OpenWithProgids" "${PROGID}" ""
  WriteRegStr SHCTX "Software\Classes\${EXT}" "" "${PROGID}"
!macroend
!macro Dissociate EXT
  DeleteRegValue SHCTX "Software\Classes\${EXT}\OpenWithProgids" "${PROGID}"
  ReadRegStr $0 SHCTX "Software\Classes\${EXT}" ""
  StrCmp $0 "${PROGID}" 0 +2
    DeleteRegValue SHCTX "Software\Classes\${EXT}" ""
  DeleteRegKey /ifempty SHCTX "Software\Classes\${EXT}\OpenWithProgids"
  DeleteRegKey /ifempty SHCTX "Software\Classes\${EXT}"
!macroend

; A running nfoglow.exe cannot be replaced or deleted: Windows keeps it open for writing. So before touching it, try to open
; it for writing (no change is made). If that fails: a normal setup asks to close NfoGlow (Retry / Cancel; Cancel stops
; before anything has changed), a silent one ends NfoGlow, as there is nobody to ask. ID makes the labels of each use apart.
!macro CloseApp ID
  IfFileExists "$INSTDIR\nfoglow.exe" 0 ${ID}_done
  ${ID}_retry:
  ClearErrors
  FileOpen $0 "$INSTDIR\nfoglow.exe" a
  IfErrors 0 ${ID}_free
    IfSilent ${ID}_kill
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(CLOSE_APP)" IDRETRY ${ID}_retry
    Abort
  ${ID}_kill:
    nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM nfoglow.exe'
    Pop $0
    Sleep 1500
    ClearErrors
    FileOpen $0 "$INSTDIR\nfoglow.exe" a
    IfErrors 0 ${ID}_free
      Abort
  ${ID}_free:
  FileClose $0
  ${ID}_done:
!macroend

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "NfoGlow needs a 64-bit Windows."
    Abort
  ${EndIf}
  SetRegView 64   ; the setup is a 32-bit program: else Windows files the HKLM entries under WOW6432Node
  !insertmacro MULTIUSER_INIT
FunctionEnd

Function un.onInit
  SetRegView 64
  !insertmacro MULTIUSER_UNINIT
FunctionEnd

Section "NfoGlow" SecMain
  SectionIn RO
  !insertmacro CloseApp inst
  SetOutPath "$INSTDIR"
  File "/oname=nfoglow.exe" "${EXE}"
  File "..\assets\nfo-file.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  File "/oname=THIRD-PARTY-LICENSES.txt" "..\THIRD-PARTY-LICENSES.txt"
  SetOutPath "$INSTDIR\licenses"
  File "/oname=font-CC-BY-SA-4.0.txt" "..\assets\LICENSE.txt"
  File "/oname=font-README.md" "..\assets\font-README.md"
  File "/oname=music-README.md" "..\assets\music\README.md"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\NfoGlow.lnk" "$INSTDIR\nfoglow.exe"
  WriteRegStr SHCTX "Software\NfoGlow" "InstallDir" "$INSTDIR"
  ; "Open with" lists NfoGlow for these types, and Win+R knows "nfoglow"
  WriteRegStr SHCTX "Software\Classes\Applications\nfoglow.exe" "FriendlyAppName" "NfoGlow"
  WriteRegStr SHCTX "Software\Classes\Applications\nfoglow.exe\shell\open\command" "" '"$INSTDIR\nfoglow.exe" "%1"'
  !insertmacro ForEachType SupportedType
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\nfoglow.exe" "" "$INSTDIR\nfoglow.exe"
  ; Settings > Apps lists it, with its own uninstall
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "DisplayName" "NfoGlow"
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "Publisher" "DigitGram"
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "URLInfoAbout" "https://github.com/DigitGram/nfoglow-windows"
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\nfoglow.exe"
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr SHCTX "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD SHCTX "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD SHCTX "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD SHCTX "${UNINSTALL_KEY}" "EstimatedSize" $0
SectionEnd

; Windows 10 and 11 still ask once which app opens a type, when another one was chosen there before: no program
; may decide that for the user
Section "$(OPEN_WITH)" SecAssociate
  WriteRegStr SHCTX "Software\Classes\${PROGID}" "" "$(FILE_TYPE)"
  WriteRegStr SHCTX "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\nfo-file.ico"
  WriteRegStr SHCTX "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\nfoglow.exe" "%1"'
  !insertmacro ForEachType Associate
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'   ; Explorer shows the new icons at once
SectionEnd

Section /o "$(DESKTOP)" SecDesktop
  CreateShortcut "$DESKTOP\NfoGlow.lnk" "$INSTDIR\nfoglow.exe"
SectionEnd

Section "Uninstall"
  !insertmacro CloseApp uninst
  Delete "$INSTDIR\nfoglow.exe"
  Delete "$INSTDIR\nfo-file.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD-PARTY-LICENSES.txt"
  RMDir /r "$INSTDIR\licenses"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"   ; only when empty: nothing of yours is deleted
  Delete "$SMPROGRAMS\NfoGlow.lnk"
  Delete "$DESKTOP\NfoGlow.lnk"
  !insertmacro ForEachType Dissociate
  DeleteRegKey SHCTX "Software\Classes\${PROGID}"
  DeleteRegKey SHCTX "Software\Classes\Applications\nfoglow.exe"
  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\App Paths\nfoglow.exe"
  DeleteRegKey SHCTX "${UNINSTALL_KEY}"
  DeleteRegKey SHCTX "Software\NfoGlow"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ; the settings stay unless asked otherwise ("No" is the default button; a silent uninstall keeps them). They are the
  ; uninstalling user's own, also when NfoGlow was installed for all users
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "$(PURGE)" /SD IDNO IDNO keep
    SetShellVarContext current
    RMDir /r "$APPDATA\NfoGlow"
  keep:
SectionEnd
