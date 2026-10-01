Unicode true
RequestExecutionLevel user

!include "MUI2.nsh"
!include "LogicLib.nsh"

!define PRODUCTNAME "璇玑录"
!define MANUFACTURER "天琴工作室（Lyra Studio）"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define INSTALLKEY "Software\${MANUFACTURER}\${PRODUCTNAME}"
!define BASE_VERSION "0.1.14"
!define TARGET_VERSION "0.1.15"
!define MAIN_BINARY "xuanji-lu.exe"

Name "璇玑录 v260827A → v260827B 补丁"
OutFile "../release/璇玑录-v260827A-to-v260827B-Patch.exe"
InstallDir "$LOCALAPPDATA\${PRODUCTNAME}"
ShowInstDetails show

Var ExistingVersion
Var BackupPath

!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"

Function .onInit
  ReadRegStr $INSTDIR HKCU "${INSTALLKEY}" ""
  StrCmp $INSTDIR "" 0 install_path_found
    ReadRegStr $INSTDIR HKCU "${UNINSTKEY}" "InstallLocation"

  install_path_found:
  StrCmp $INSTDIR "" 0 version_check
    MessageBox MB_ICONSTOP "未检测到《璇玑录》v260827A 的安装路径。请先安装 v260827A，再运行此补丁。"
    Abort

  version_check:
  ReadRegStr $ExistingVersion HKCU "${UNINSTKEY}" "DisplayVersion"
  StrCmp $ExistingVersion "${BASE_VERSION}" 0 wrong_version

  StrCpy $0 $INSTDIR 1
  StrCmp $0 '"' 0 main_binary_check
    StrCpy $INSTDIR $INSTDIR -1 1

  main_binary_check:
  IfFileExists "$INSTDIR\${MAIN_BINARY}" version_ok
    MessageBox MB_ICONSTOP "已找到安装目录：$INSTDIR$\r$\n但找不到主程序 ${MAIN_BINARY}。请确认《璇玑录》已完全退出，或重新安装 v260827A。"
    Abort

  wrong_version:
    MessageBox MB_ICONSTOP "此补丁只允许从《璇玑录》v260827A（内部版本 ${BASE_VERSION}）升级。当前安装版本：$ExistingVersion。"
    Abort

  version_ok:
FunctionEnd

Section "升级主程序"
  DetailPrint "正在备份 v260827A 主程序…"
  StrCpy $BackupPath "$INSTDIR\xuanji-lu-v260827A-backup.exe"
  Delete "$BackupPath"
  CopyFiles /SILENT "$INSTDIR\${MAIN_BINARY}" "$BackupPath"
  IfErrors backup_failed

  DetailPrint "正在写入 v260827B 主程序…"
  SetOutPath "$INSTDIR"
  SetOverwrite on
  File /oname=${MAIN_BINARY} "../src-tauri/target/x86_64-pc-windows-msvc/release/${MAIN_BINARY}"
  IfErrors write_failed

  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${TARGET_VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAIN_BINARY}$\""
  Delete "$BackupPath"
  DetailPrint "升级完成。已修复 Edge 音色回显，并让星语档案中的历史占问和训练点评显示语音播放/停止及 MP3 另存图标；用户档案、图片文件、SQLite 数据库、备份、Profile 与 Windows 凭据均未改动。"
  Goto done

  backup_failed:
    MessageBox MB_ICONSTOP "无法备份当前主程序。请完全退出《璇玑录》后重试。"
    Abort

  write_failed:
    DetailPrint "写入失败，正在回滚 v260827A 主程序…"
    CopyFiles /SILENT "$BackupPath" "$INSTDIR\${MAIN_BINARY}"
    MessageBox MB_ICONSTOP "补丁写入失败，已尝试恢复 v260827A。请完全退出《璇玑录》后重试。"
    Abort

  done:
SectionEnd
