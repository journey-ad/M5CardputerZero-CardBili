#!/bin/sh
# 在指定设备上执行命令
set -eu
DIR=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
ENV_FILE=${DEVICE_ENV_FILE:-.secrets/device.env}
if [ -f "$ENV_FILE" ]; then
    set -a
    # shellcheck disable=SC1090
    . "$ENV_FILE"
    set +a
fi
DEVICE_HOST=${DEVICE_HOST:-${DEVICE_USER:+$DEVICE_USER@}${DEVICE_IP:-}}
: "${DEVICE_HOST:?请配置 DEVICE_HOST 或 DEVICE_USER 与 DEVICE_IP}"
if [ -n "${DEVICE_PASS:-}" ]; then
    export DEVICE_PASS
    export SSH_ASKPASS="$DIR/askpass.sh" SSH_ASKPASS_REQUIRE=force
fi
exec ssh "$DEVICE_HOST" "$@"
