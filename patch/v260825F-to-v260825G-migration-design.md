# 《璇玑录》v260825F→v260825G 数据迁移设计

## 目标

将现有用户数据根目录从 `%LOCALAPPDATA%\XuanjiLu` 迁移到软件安装目录下的 `%LOCALAPPDATA%\璇玑录\data`。目标根目录包含 `xuanji-lu.sqlite3`、`images\` 与 `backups\`；Windows Credential Manager 中的密钥不移动，仍由系统管理。

## 补丁迁移流程

F→G 补丁只接受内部版本 `0.1.5`。在备份 F 主程序之后，补丁检查旧数据目录和新数据目录：当旧目录存在且新目录不存在时，使用同一 `%LOCALAPPDATA%` 卷内的目录重命名将整个 `XuanjiLu` 目录移动为 `$INSTDIR\data`。目录重命名为同卷原子操作，不会逐文件复制造成部分移动状态。

若写入 G 主程序失败，补丁恢复 F 主程序；若本次已移动数据目录，则将 `$INSTDIR\data` 重命名回 `%LOCALAPPDATA%\XuanjiLu`。若新数据目录已存在，补丁不覆盖它；这使因中断后重试、或手工安装后首次运行完成迁移的情况保持安全。

## 应用侧兼容

G 启动时以可执行文件所在目录的 `data\` 作为唯一运行数据根目录。若该目录尚无数据库但旧 `%LOCALAPPDATA%\XuanjiLu` 存在，应用会先复制旧数据库、图库与备份到新目录，再从新目录打开数据库；复制成功前绝不删除旧目录。该兼容分支覆盖直接安装完整包、或补丁未能完成迁移的恢复场景。

## 原图查看权限

`opener:allow-open-path` 仅允许调用命令，并不自动授予任何路径范围。G 会将权限收敛至 `$EXE/data/images/**/*`，并将 asset 协议范围同步至相同目录。因此，系统默认查看器只能被请求打开软件安装目录下、专用图库中的本地文件；其他任意用户路径不会被授予打开权限。

## 迁移边界

迁移完成后，数据库中的图片路径因整个目录树被重命名而自动保持有效：

```text
%LOCALAPPDATA%\XuanjiLu\images\202608\image.png
→
%LOCALAPPDATA%\璇玑录\data\images\202608\image.png
```

不过 SQLite 中保存的是绝对路径，因此补丁需要在同一事务中将 `divination_records.image_url` 与 `gallery_items.image_url` 中旧根目录前缀替换为新根目录前缀。应用侧也在启动时执行同样的幂等路径前缀修复，以覆盖直接安装或中断恢复情形。
