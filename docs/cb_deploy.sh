#!/bin/bash
# 设备 sudo 口令从项目根 .secrets/device.env 读取（该目录不入库）
DIR=$(cd "$(dirname "$0")" && pwd)
ENV_FILE="$DIR/../.secrets/device.env"
[ -f "$ENV_FILE" ] && . "$ENV_FILE"
SUDO_PASS=${DEVICE_SUDO_PASS:-${DEVICE_PASS:?请先在 .secrets/device.env 配置 DEVICE_PASS}}
sp(){ echo "$SUDO_PASS" | sudo -S "$@" 2>/dev/null; }
sp install -m0755 /tmp/cardbili /usr/share/APPLaunch/bin/M5CardputerZero-CardBili
sp install -m0644 /tmp/icon_cardbili_100.png /usr/share/APPLaunch/share/images/cardbili_100.png
sp install -m0644 /tmp/icon_cardbili_80.png  /usr/share/APPLaunch/share/images/cardbili_80.png
sp install -m0644 /tmp/cardbili.desktop /usr/share/APPLaunch/applications/cardbili.desktop
echo "bin:"; ls -l /usr/share/APPLaunch/bin/M5CardputerZero-CardBili
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user restart APPLaunch.service; sleep 2
echo "active: $(systemctl --user is-active APPLaunch.service)"
P=$(pgrep -f M5CardputerZero-APPLaunch | head -1); echo "launcher pid=$P"
