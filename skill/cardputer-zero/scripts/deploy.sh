#!/usr/bin/env bash
# 上传发布包并在设备上运行安装脚本
set -euo pipefail
DIR=$(cd "$(dirname "$0")" && pwd)
PACKAGE=${1:?用法：deploy.sh 发布包.tar.gz}
ENV_FILE=${DEVICE_ENV_FILE:-.secrets/device.env}
if [[ -f "$ENV_FILE" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    set +a
fi
DEVICE_HOST=${DEVICE_HOST:-${DEVICE_USER:+$DEVICE_USER@}${DEVICE_IP:-}}
: "${DEVICE_HOST:?请配置 DEVICE_HOST 或 DEVICE_USER 与 DEVICE_IP}"
export DEVICE_HOST
if [[ -n "${DEVICE_PASS:-}" ]]; then
    export DEVICE_PASS
    export SSH_ASKPASS="$DIR/askpass.sh" SSH_ASKPASS_REQUIRE=force
fi
test -f "$PACKAGE"
REMOTE_DIR=$(ssh "$DEVICE_HOST" 'mktemp -d /var/tmp/cardbili.XXXXXXXX')
[[ "$REMOTE_DIR" =~ ^/var/tmp/cardbili\.[a-zA-Z0-9]+$ ]]
trap 'ssh "$DEVICE_HOST" "rm -rf -- '"'"'$REMOTE_DIR'"'"'"' EXIT
scp "$PACKAGE" "$DEVICE_HOST:$REMOTE_DIR/package.tar.gz"
LOCAL_SHA=$(shasum -a 256 "$PACKAGE" | cut -d ' ' -f 1)
# shellcheck disable=SC2029
REMOTE_SHA=$(ssh "$DEVICE_HOST" "sha256sum '$REMOTE_DIR/package.tar.gz'" | cut -d ' ' -f 1)
[[ "$LOCAL_SHA" == "$REMOTE_SHA" ]] || { printf '发布包校验失败\n' >&2; exit 1; }
ssh -t "$DEVICE_HOST" "cd '$REMOTE_DIR' && tar -xzf package.tar.gz && bash cardbili-*/install.sh"
