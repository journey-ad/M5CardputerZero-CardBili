# APPLaunch 启动器与装机

## 架构
- systemd 用户服务 `APPLaunch.service`（`/usr/lib/systemd/user/APPLaunch.service`），以设备用户运行，进程 `M5CardputerZero-APPLaunch`，用 LVGL 9.5 直接写 `/dev/fb0`。内置 App Store 在同一进程内（屏幕名 Home）。
- launcher 只打开 `/dev/fb0` 与 `/dev/input/event3`；不收 uinput 注入。

## 应用 = desktop 文件
`/usr/share/APPLaunch/applications/*.desktop`：
```ini
[Desktop Entry]
Type=Application
Name=MyApp
Name[zh_CN]=我的应用
Exec=/usr/share/APPLaunch/bin/M5CardputerZero-MyApp
Icon=share/images/myapp_100.png
Terminal=false
Categories=Media;
```
- Exec 安全策略 `cp0_desktop_exec_policy`：拒绝 shell 元字符；Exec 必须落在允许根（含 `/usr/bin`、`/bin`、`/usr/share/APPLaunch`）；不安全会在日志 `applications_load: skip %s (unsafe Exec)` 被跳过。重名/重复 Exec/无效 Icon 也会被跳过。
- 图标放 `/usr/share/APPLaunch/share/images/<name>_100.png`（及 `_80.png`）。

## 启停（改完 desktop/重启才生效）
```sh
export XDG_RUNTIME_DIR=/run/user/1000
export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user stop|start|restart APPLaunch.service
```
- 停掉 launcher 后可直接写 `/dev/fb0` 并保留；`systemctl --user start` 恢复 UI。
- 打开外部 app：launcher exec 子进程并暂停自身，返回时日志 `[cp0] Returned to launcher`。

## 装机套路（自研 app）
一键部署（scp + 远程 install + 用户级重启 + md5 校验）在仓库根执行 `bash skill/cardputer-zero/scripts/deploy.sh`（默认部署 CardBili，可传二进制路径）。手工分步时注意：`install` 必须在**设备上**执行，本机 `sudo` 写不到设备；用 ssh 远程执行：
```sh
source .secrets/device.env
# 1) 传产物到设备
scp app "$DEVICE_USER@$DEVICE_IP:/var/tmp/app"
# 2) 远程安装（sudo 密码走 stdin，DEVICE_SUDO_PASS 见 device.env）
printf '%s\n' "$DEVICE_SUDO_PASS" | ssh "$DEVICE_USER@$DEVICE_IP" \
  'sudo -S install -m0755 /var/tmp/app /usr/share/APPLaunch/bin/M5CardputerZero-MyApp'
# 3) 用户级重启 launcher（APPLaunch 是 user service，非系统级）
ssh "$DEVICE_USER@$DEVICE_IP" \
  'export XDG_RUNTIME_DIR=/run/user/$(id -u); export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus; systemctl --user restart APPLaunch.service'
```
校验安装：`ssh ... 'md5sum /usr/share/APPLaunch/bin/M5CardputerZero-MyApp'` 与本地一致。
注意：重启 launcher 会回到 Home（App Store），CardBili 需在设备上重新进入才能看到；SSH 无法模拟按键。

## 确认已注册（可选）
读 launcher 进程内存扫描字符串（sudo）：在 `/proc/<pid>/mem` 的 heap 段搜应用名，出现次数 >0 表示 desktop 已加载。已有脚本思路（`scan.py`）。仅作辅助，最终以真机翻页为准。

## 参考：单独运行 AppStore（模仿其 UI 风格用）
```sh
systemctl --user stop APPLaunch.service
APPLAUNCH_LINUX_FBDEV_DEVICE=/dev/fb0 \
APPLAUNCH_LINUX_KEYBOARD_DEVICE=/dev/input/by-path/platform-3f804000.i2c-event \
  /usr/share/APPLaunch/bin/M5CardputerZero-AppStore &
# 约 25s 到 Home 列表页；抓帧后 kill，再 start launcher
```
其底栏：5 个等宽彩色项（无数字，按位置对应 4~8 键）。

## AppStore registry
- `https://cardputer-zero-repo.oss-cn-shenzhen.aliyuncs.com/packages/cn/registry.json`（可用）
- `d["apps"]` 为应用 list：uuid、share_code、title、summary、description、categories、author、version、download{type:"deb",package,url}
- 缓存：`~/.cache/cardputerzero-appstore/{downloads/*.deb, icons/*.png}`
