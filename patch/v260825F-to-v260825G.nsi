Unicode true
RequestExecutionLevel user

!include "MUI2.nsh"
!include "LogicLib.nsh"

!define PRODUCTNAME "璇玑录"
!define MANUFACTURER "天琴工作室（Lyra Studio）"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
!define INSTALLKEY "Software\${MANUFACTURER}\${PRODUCTNAME}"
!define BASE_VERSION "0.1.5"
!define TARGET_VERSION "0.1.6"
!define MAIN_BINARY "xuanji-lu.exe"
!define LEGACY_DATA "$LOCALAPPDATA\XuanjiLu"
!define TARGET_DATA "$INSTDIR\data"

Name "璇玑录 v260825F → v260825G 数据迁移补丁"
OutFile "../release/璇玑录-v260825F-to-v260825G-Patch.exe"
InstallDir "$LOCALAPPDATA\${PRODUCTNAME}"
ShowInstDetails show

Var ExistingVersion
Var BackupPath
Var DataMoved

!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"

Function .onInit
  ReadRegStr $INSTDIR HKCU "${INSTALLKEY}" ""
  StrCmp $INSTDIR "" 0 install_path_found
    ReadRegStr $INSTDIR HKCU "${UNINSTKEY}" "InstallLocation"

  install_path_found:
  StrCmp $INSTDIR "" 0 version_check
    MessageBox MB_ICONSTOP "未检测到《璇玑录》v260825F 的安装路径。请先安装 v260825F，再运行此补丁。"
    Abort

  version_check:
  ReadRegStr $ExistingVersion HKCU "${UNINSTKEY}" "DisplayVersion"
  StrCmp $ExistingVersion "${BASE_VERSION}" 0 wrong_version

  StrCpy $0 $INSTDIR 1
  StrCmp $0 '"' 0 main_binary_check
    StrCpy $INSTDIR $INSTDIR -1 1

  main_binary_check:
  IfFileExists "$INSTDIR\${MAIN_BINARY}" version_ok
    MessageBox MB_ICONSTOP "已找到安装目录：$INSTDIR$\r$\n但找不到主程序 ${MAIN_BINARY}。请确认《璇玑录》已完全退出，或重新安装 v260825F。"
    Abort

  wrong_version:
    MessageBox MB_ICONSTOP "此补丁只允许从《璇玑录》v260825F（内部版本 ${BASE_VERSION}）升级。当前安装版本：$ExistingVersion。"
    Abort

  version_ok:
FunctionEnd

Section "迁移数据并升级主程序"
  StrCpy $DataMoved "0"
  DetailPrint "正在备份 v260825F 主程序…"
  StrCpy $BackupPath "$INSTDIR\xuanji-lu-v260825F-backup.exe"
  Delete "$BackupPath"
  CopyFiles /SILENT "$INSTDIR\${MAIN_BINARY}" "$BackupPath"
  IfErrors backup_failed

  ; 仅在新数据根尚无数据库、且旧数据根存在时移动整个目录。
  ; 两者都在 %LOCALAPPDATA% 同一卷上，Rename 是原子目录移动。
  IfFileExists "${TARGET_DATA}\*.*" data_ready
  IfFileExists "${LEGACY_DATA}\xuanji-lu.sqlite3" 0 data_ready
    ; 仅移除空 data 目录，非空目录绝不覆盖，交由 G 首次启动兼容复制处理。
    RMDir "${TARGET_DATA}"
    DetailPrint "正在将本地档案和图库迁移到软件目录 data…"
    Rename "${LEGACY_DATA}" "${TARGET_DATA}"
    IfErrors data_migration_failed
    StrCpy $DataMoved "1"

  data_ready:
  DetailPrint "正在写入 v260825G 主程序…"
  SetOutPath "$INSTDIR"
  SetOverwrite on
  File /oname=${MAIN_BINARY} "../src-tauri/target/x86_64-pc-windows-msvc/release/${MAIN_BINARY}"
  IfErrors write_failed

  WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${TARGET_VERSION}"
  WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$\"$INSTDIR\${MAIN_BINARY}$\""
  Delete "$BackupPath"
  DetailPrint "升级完成。数据库、图库和备份已迁移至 $INSTDIR\data；Windows 凭据未改动。"
  Goto done

  backup_failed:
    MessageBox MB_ICONSTOP "无法备份当前主程序。请完全退出《璇玑录》后重试。"
    Abort

  data_migration_failed:
    MessageBox MB_ICONSTOP "无法迁移本地档案与图库。旧数据保持原位置，主程序未写入。请确认软件已完全退出后重试。"
    Abort

  write_failed:
    DetailPrint "写入失败，正在回滚 v260825F 主程序…"
    CopyFiles /SILENT "$BackupPath" "$INSTDIR\${MAIN_BINARY}"
    StrCmp $DataMoved "1" 0 +3
      DetailPrint "正在回滚本地数据目录…"
      Rename "${TARGET_DATA}" "${LEGACY_DATA}"
    MessageBox MB_ICONSTOP "补丁写入失败，已尝试恢复 v260825F 和原数据目录。请完全退出《璇玑录》后重试。"
    Abort

  done:
SectionEnd
