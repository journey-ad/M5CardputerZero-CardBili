# 访问设备（SSH / sudo / scp）

设备地址与口令属于本地环境敏感信息，见项目根 `.secrets/device.env`（已 git 忽略，不入库）：`DEVICE_IP` / `DEVICE_USER` / `DEVICE_PASS`（SSH 登录口令）/ `DEVICE_SUDO_PASS`（sudo 口令，可能与 SSH 不同，以 device.env 为准）。IP 由手机热点临时分配，重连可能变化，以手机热点「已连接设备」列表为准。本机（Mac）无 sshpass/pamiko，`expect` 因无 pty 不可用，故用 **SSH_ASKPASS** 免交互。

## askpass.sh
```sh
#!/bin/sh
DIR=$(cd "$(dirname "$0")" && pwd)
ENV_FILE="$DIR/../../../.secrets/device.env"
[ -f "$ENV_FILE" ] && . "$ENV_FILE"
echo "${DEVICE_PASS:?请先在 .secrets/device.env 配置 DEVICE_PASS}"
```
`chmod +x askpass.sh`

## r.sh（远程执行包装）
```sh
#!/bin/sh
DIR=$(cd "$(dirname "$0")" && pwd)
ENV_FILE="$DIR/../../../.secrets/device.env"
[ -f "$ENV_FILE" ] && . "$ENV_FILE"
SSH_ASKPASS="$DIR/askpass.sh" SSH_ASKPASS_REQUIRE=force \
  ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  "$DEVICE_USER"@"$DEVICE_IP" "$@"
```
用法：`./r.sh 'uname -a'`（传单条远程命令字符串）。

## scp
```sh
source .secrets/device.env
SSH_ASKPASS=$PWD/askpass.sh SSH_ASKPASS_REQUIRE=force \
  scp -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  local "$DEVICE_USER@$DEVICE_IP:/tmp/"
```

## sudo（远程）
`install` 等命令必须在**设备上**执行（本机 `sudo` 写不到设备），用 ssh 远程跑，sudo 密码走 stdin：
```sh
printf '%s\n' "$DEVICE_SUDO_PASS" | ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  "$DEVICE_USER@$DEVICE_IP" 'sudo -S install -m0755 /var/tmp/bin /usr/share/APPLaunch/bin/NAME'
```

## 备注
- 首次连接会写 known_hosts；用 `UserKnownHostsFile=/dev/null` 规避。
- 设备联网正常（tuna/aliyun 镜像均可达，约 3.6MB/s）。
- 本机代理 `http://127.0.0.1:7890` 用于 Mac 侧访问 GitHub（见 debugging-playbook）。
- 设备地址与口令统一在 `.secrets/device.env`，不写进仓库。
