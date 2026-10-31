Unicode True
!define MUI_ICON "assets\brand\stilldock.ico"
!define MUI_UNICON "assets\brand\stilldock.ico"
!include "MUI2.nsh"
!include "x64.nsh"
!ifndef APP_VERSION
  !define APP_VERSION "0.5.0"
!endif
!ifndef APP_EXE
  !define APP_EXE "target\release\xhs-browser.exe"
!endif
!ifndef WEBVIEW2_SETUP
  !define WEBVIEW2_SETUP "vendor\MicrosoftEdgeWebview2Setup.exe"
!endif
Name "原帧 StillDock"
OutFile "dist\StillDock-${APP_VERSION}-windows-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\XhsBrowser"
RequestExecutionLevel user
SetCompressor /SOLID lzma
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\xhs-browser.exe"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"

Section "安装"
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "此版本需要 64 位 Windows 10 或更新版本。"
    Abort
  ${EndIf}
  ; Clean up shortcuts belonging to the previous Shiguang preview only.
  Delete "$DESKTOP\拾光小红书浏览器.lnk"
  Delete "$SMPROGRAMS\拾光小红书浏览器\拾光.lnk"
  Delete "$SMPROGRAMS\拾光小红书浏览器\卸载.lnk"
  RMDir "$SMPROGRAMS\拾光小红书浏览器"
  SetOutPath "$INSTDIR"
  File /oname=xhs-browser.exe "${APP_EXE}"
  File "LICENSE"
  File "README.md"
  File "NOTICE.md"
  SetOutPath "$INSTDIR\assets\brand"
  File "assets\brand\stilldock-256.png"
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$DESKTOP\原帧小红书浏览器.lnk" "$INSTDIR\xhs-browser.exe"
  CreateDirectory "$SMPROGRAMS\原帧小红书浏览器"
  CreateShortcut "$SMPROGRAMS\原帧小红书浏览器\原帧.lnk" "$INSTDIR\xhs-browser.exe"
  CreateShortcut "$SMPROGRAMS\原帧小红书浏览器\卸载.lnk" "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\XhsBrowserRust" "DisplayName" "原帧 StillDock"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\XhsBrowserRust" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\XhsBrowserRust" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\XhsBrowserRust" "DisplayIcon" "$INSTDIR\xhs-browser.exe,0"
  SetRegView 32
  ReadRegStr $0 HKLM "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  StrCmp $0 "" 0 runtime_done
  ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  StrCmp $0 "" 0 runtime_done
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=MicrosoftEdgeWebview2Setup.exe "${WEBVIEW2_SETUP}"
  DetailPrint "正在安装 Microsoft WebView2，需要联网……"
  ExecWait '$\"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe$\" /silent /install' $1
  StrCmp $1 0 runtime_done
    MessageBox MB_ICONEXCLAMATION "WebView2 安装未完成。请联网后安装 Microsoft Edge WebView2 Runtime，再启动本程序。"
  runtime_done:
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\原帧小红书浏览器.lnk"
  Delete "$SMPROGRAMS\原帧小红书浏览器\原帧.lnk"
  Delete "$SMPROGRAMS\原帧小红书浏览器\卸载.lnk"
  RMDir "$SMPROGRAMS\原帧小红书浏览器"
  Delete "$INSTDIR\xhs-browser.exe"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\NOTICE.md"
  Delete "$INSTDIR\assets\brand\stilldock-256.png"
  RMDir "$INSTDIR\assets\brand"
  RMDir "$INSTDIR\assets"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\XhsBrowserRust"
  ; Browser profiles, history and downloaded media are deliberately retained.
SectionEnd
