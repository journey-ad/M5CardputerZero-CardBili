#!/bin/sh
# 设备口令从项目根 .secrets/device.env 读取（该目录不入库）
DIR=$(cd "$(dirname "$0")" && pwd)
ENV_FILE="$DIR/../../../.secrets/device.env"
[ -f "$ENV_FILE" ] && . "$ENV_FILE"
echo "${DEVICE_PASS:?请先在 .secrets/device.env 配置 DEVICE_PASS}"
