#!/usr/bin/env bash
# 在设备上安装解压后的发布包
set -euo pipefail
PACKAGE=$(cd "${1:-$(dirname "$0")}" && pwd)
for file in M5CardputerZero-CardBili cardbili-ffmpeg cardbili.desktop cardbili_100.png cardbili_80.png; do
    test -f "$PACKAGE/$file" || { printf '缺少文件：%s\n' "$file" >&2; exit 1; }
done
test -d "$PACKAGE/licenses"
sudo -v
sudo install -Dm755 "$PACKAGE/M5CardputerZero-CardBili" /usr/share/APPLaunch/bin/M5CardputerZero-CardBili
sudo install -Dm755 "$PACKAGE/cardbili-ffmpeg" /usr/share/APPLaunch/bin/cardbili-ffmpeg
sudo install -Dm644 "$PACKAGE/cardbili.desktop" /usr/share/APPLaunch/applications/cardbili.desktop
for size in 100 80; do
    sudo install -Dm644 "$PACKAGE/cardbili_$size.png" "/usr/share/APPLaunch/share/images/cardbili_$size.png"
done
sudo mkdir -p /usr/share/doc/cardbili
sudo cp -R "$PACKAGE/licenses/." /usr/share/doc/cardbili/
sudo install -m644 "$PACKAGE/THIRD_PARTY_NOTICES.md" /usr/share/doc/cardbili/
XDG_RUNTIME_DIR="/run/user/$(id -u)"
export XDG_RUNTIME_DIR
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
systemctl --user restart APPLaunch.service
systemctl --user is-active APPLaunch.service
printf '安装完成，请在设备启动器中打开 CardBili\n'
