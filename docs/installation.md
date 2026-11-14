# 安装与设备连接

## 设备要求

- 运行 aarch64 Linux 的 Cardputer Zero，320 × 170 RGB565 内置屏幕
- APPLaunch 用户服务，可访问帧缓冲与键盘设备
- PulseAudio 或 PipeWire Pulse 服务，以及 `pulseaudio-utils` 提供的 `pacat` / `pactl`
- `ca-certificates`，供 HTTPS 请求校验证书

系统设备节点和权限随镜像变化，可参考 [硬件检查](../skill/cardputer-zero/references/device-hardware.md)

## 从 Release 安装

在同一目录下载设备包、源码包与 `SHA256SUMS`，在设备上执行：

```sh
sha256sum -c SHA256SUMS
tar -xzf cardbili-0.1.0-aarch64-linux-musl.tar.gz
bash cardbili-0.1.0-aarch64-linux-musl/install.sh
```

替换文件名中的版本号。脚本使用当前 SSH 用户的 sudo 授权，将应用、解码器、图标与启动项安装到 `/usr/share/APPLaunch/`，许可证安装到 `/usr/share/doc/cardbili/`，随后重启该用户的 APPLaunch 服务

## 从开发机部署

先在 `~/.ssh/config` 中配置设备别名：

```sshconfig
Host cardputer
    HostName cardputer.local
    User your-device-user
```

执行 `ssh cardputer`，核对设备主机指纹并完成首次连接。推荐使用 SSH 密钥，也可以在终端交互输入口令

```sh
cp .secrets/device.env.example .secrets/device.env
chmod 600 .secrets/device.env
bash skill/cardputer-zero/scripts/deploy.sh dist/cardbili-0.1.0-aarch64-linux-musl.tar.gz
```

`DEVICE_HOST` 支持 SSH 别名或 `用户名@地址`；现有的 `DEVICE_IP` / `DEVICE_USER` 配置同样可用。脚本校验上传包的 SHA-256，再运行设备端安装脚本；sudo 密码由设备终端提示输入

在其他项目使用 skill 时，从该项目根执行，或通过 `DEVICE_ENV_FILE=/绝对路径/device.env` 指定可信的本地配置文件

## 扫码登录

匿名模式可浏览接口允许的内容。TV/HD 登录需要使用者提供应用凭据，程序读取 `BILI_APP_KEY` 与 `BILI_APP_SECRET`，发布包包含空值模板

开发机直接运行时：

```sh
cp .secrets/bilibili.env.example .secrets/bilibili.env
chmod 600 .secrets/bilibili.env
# 编辑文件并填写自己的应用凭据
set -a
. .secrets/bilibili.env
set +a
cd cardbili
cargo run --locked --features desktop
```

设备从 APPLaunch 启动时，在设备的 `~/.config/cardbili/bilibili.env` 填写同名变量，权限设为 `0600`。执行 `systemctl --user edit APPLaunch.service`，加入：

```ini
[Service]
EnvironmentFile=%h/.config/cardbili/bilibili.env
```

然后执行 `systemctl --user daemon-reload` 与 `systemctl --user restart APPLaunch.service`。该配置会将应用凭据传入启动器及其子进程，适用于个人设备

登录二维码请在设备上查看。分享截图、日志或备份时，保护应用凭据、`auth.dat`、`master.key` 和账号 Cookie
