#!/usr/bin/env bash
# 构建、打包、校验并安装到 CardputerZero 真机
# 用法：scripts/install-device.sh [--skip-build] [--with-ffmpeg] [--host=IP]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
HOST_OVERRIDE=""
SKIP_BUILD=0
SKIP_FFMPEG=1
for arg in "$@"; do
    case "$arg" in
        --skip-build) SKIP_BUILD=1 ;;
        --with-ffmpeg) SKIP_FFMPEG=0 ;;
        --host=*) HOST_OVERRIDE="${arg#--host=}" ;;
        *) echo "未知参数: $arg" >&2; exit 2 ;;
    esac
done

ENV_FILE="$ROOT/.secrets/device.env"
if [[ -f $ENV_FILE ]]; then
    # shellcheck disable=SC1090
    source "$ENV_FILE"
fi
DEVICE_IP=${HOST_OVERRIDE:-${DEVICE_IP:-}}
: "${DEVICE_IP:?请设置 DEVICE_IP 或写入 .secrets/device.env}"
: "${DEVICE_USER:?请设置 DEVICE_USER}"
: "${DEVICE_PASS:?请设置 DEVICE_PASS}"
DEVICE_SUDO_PASS=${DEVICE_SUDO_PASS:-$DEVICE_PASS}
export DEVICE_PASS

ASKPASS="$ROOT/.agents/skills/cardputer-zero/scripts/askpass.sh"
SSH=(ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "$DEVICE_USER@$DEVICE_IP")
SCP=(scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null)
export SSH_ASKPASS="$ASKPASS" SSH_ASKPASS_REQUIRE=force

if [[ $SKIP_BUILD == 0 ]]; then
    echo "== 构建 =="
    SKIP_FFMPEG=$SKIP_FFMPEG "$ROOT/scripts/build-device.sh"
    echo "== 打包 =="
    python3 "$ROOT/scripts/package-release.py"
fi

DEB=""
for candidate in "$ROOT"/dist/cardbili_*_arm64.deb; do
    [[ -f $candidate ]] || continue
    if [[ -z $DEB || $candidate -nt $DEB ]]; then
        DEB=$candidate
    fi
done
[[ -n $DEB ]] || { echo "未找到 DEB 包" >&2; exit 1; }
echo "== 校验 $DEB =="
python3 "$ROOT/scripts/check-release.py" --deb "$DEB"

REMOTE=/var/tmp/$(basename "$DEB")
echo "== 上传到 $DEVICE_IP =="
"${SCP[@]}" "$DEB" "$DEVICE_USER@$DEVICE_IP:$REMOTE"

echo "== 安装 =="
printf '%s\n' "$DEVICE_SUDO_PASS" | "${SSH[@]}" "sudo -S bash -c 'DEBIAN_FRONTEND=noninteractive apt-get install -y --reinstall $REMOTE'"

echo "== 重启应用与启动器 =="
# shellcheck disable=SC2016
"${SSH[@]}" 'sudo -n pkill -f M5CardputerZero-CardBili 2>/dev/null || pkill -f M5CardputerZero-CardBili 2>/dev/null || true; export XDG_RUNTIME_DIR=/run/user/$(id -u); export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus; systemctl --user reset-failed APPLaunch.service 2>/dev/null || true; systemctl --user start APPLaunch.service 2>/dev/null || true'
echo "完成"
