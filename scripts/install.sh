#!/usr/bin/env bash
# 在设备上通过 APT 安装或升级 Debian 包
set -euo pipefail
PACKAGE=${1:?用法：install.sh 安装包.deb}
test -f "$PACKAGE"
PACKAGE="$(cd "$(dirname "$PACKAGE")" && pwd)/$(basename "$PACKAGE")"
sudo apt install "$PACKAGE"
printf '安装完成，请在设备启动器中打开 CardBili\n'
