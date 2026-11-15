#!/bin/sh
# SSH_ASKPASS 通过环境读取口令
set -eu
case "${1:-}" in
    *[Pp]assword*) printf '%s\n' "${DEVICE_PASS:?请配置 DEVICE_PASS}" ;;
    *) exit 1 ;;
esac
