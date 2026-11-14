#!/bin/sh
# 一键部署 CardBili 到 CardputerZero 并重启 APPLaunch（用户级 systemd 服务）
# 用法: ./deploy.sh [本地二进制路径]
# 默认部署 <项目根>/cardbili/target/aarch64-unknown-linux-musl/release/cardbili
set -e
DIR=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$DIR/../../.." && pwd)
ENV_FILE="$ROOT/.secrets/device.env"
[ -f "$ENV_FILE" ] && . "$ENV_FILE"
DEVICE_IP=${DEVICE_IP:?请在 .secrets/device.env 配置 DEVICE_IP}
DEVICE_USER=${DEVICE_USER:?请在 .secrets/device.env 配置 DEVICE_USER}
DEVICE_SUDO_PASS=${DEVICE_SUDO_PASS:?请在 .secrets/device.env 配置 DEVICE_SUDO_PASS}

BIN=${1:-$ROOT/cardbili/target/aarch64-unknown-linux-musl/release/cardbili}
TARGET=/usr/share/APPLaunch/bin/M5CardputerZero-CardBili
REMOTE_TMP=/var/tmp/cardbili
SSH_OPTS="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null"
export SSH_ASKPASS="$DIR/askpass.sh" SSH_ASKPASS_REQUIRE=force

[ -f "$BIN" ] || { echo "找不到二进制: $BIN"; exit 1; }

echo "==> 上传 $BIN"
scp -q $SSH_OPTS "$BIN" "$DEVICE_USER@$DEVICE_IP:$REMOTE_TMP"

echo "==> 安装到 $TARGET 并校验 md5"
LOCAL_MD5=$(md5 -q "$BIN")
REMOTE_MD5=$(printf '%s\n' "$DEVICE_SUDO_PASS" | ssh $SSH_OPTS "$DEVICE_USER@$DEVICE_IP" \
  "sudo -S install -m0755 $REMOTE_TMP $TARGET >/dev/null 2>&1 && md5sum $TARGET | cut -d' ' -f1")
echo "local=$LOCAL_MD5 remote=$REMOTE_MD5"
[ "$LOCAL_MD5" = "$REMOTE_MD5" ] || { echo "!! md5 不一致，部署失败"; exit 1; }

echo "==> 重启 APPLaunch（用户级服务，非系统级）"
ssh $SSH_OPTS "$DEVICE_USER@$DEVICE_IP" \
  'export XDG_RUNTIME_DIR=/run/user/$(id -u); export DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u)/bus; systemctl --user restart APPLaunch.service; sleep 1; systemctl --user is-active APPLaunch.service'
echo "==> 完成：launcher 已回到 Home，CardBili 需在设备上重新进入"
