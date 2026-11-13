#!/bin/sh
# 远程执行包装: ./r.sh 'command'
DIR=$(cd "$(dirname "$0")" && pwd)
# 设备地址/账号从项目根 .secrets/device.env 读取（该目录不入库），可用环境变量覆盖
ENV_FILE="$DIR/../../../.secrets/device.env"
[ -f "$ENV_FILE" ] && . "$ENV_FILE"
DEVICE_IP=${DEVICE_IP:?请先在 .secrets/device.env 配置 DEVICE_IP}
DEVICE_USER=${DEVICE_USER:?请先在 .secrets/device.env 配置 DEVICE_USER}
SSH_ASKPASS="$DIR/askpass.sh" SSH_ASKPASS_REQUIRE=force \
  ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  "$DEVICE_USER"@"$DEVICE_IP" "$@"
