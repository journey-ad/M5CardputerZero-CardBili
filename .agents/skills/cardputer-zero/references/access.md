# SSH 连接

优先使用用户已有的 SSH 配置与密钥，例如 `~/.ssh/config` 中的 `Host cardputer`。首次连接在终端执行 `ssh cardputer` 并核对主机指纹

## 配置

在调用脚本的项目根创建 `.secrets/device.env`，加入 `.gitignore` 并设为 `0600`：

```sh
DEVICE_HOST=cardputer
DEVICE_PASS=
```

`DEVICE_HOST` 支持别名或 `用户名@地址`，也兼容 `DEVICE_USER` 与 `DEVICE_IP`。`DEVICE_PASS` 留空时使用 SSH 默认认证；填写后由 `SSH_ASKPASS` 读取环境变量

从其他目录调用时设置 `DEVICE_ENV_FILE` 为绝对路径。配置文件作为 shell 文件读取，使用自己维护的可信文件

## 执行命令

```sh
/path/to/cardputer-zero/scripts/r.sh 'uname -m; id; systemctl --user is-active APPLaunch.service'
```

脚本保留 SSH 的默认主机密钥检查。设备重装导致指纹变化时，先核实设备身份，再更新对应的 known_hosts 记录

## 复制文件与 sudo

```sh
scp app cardputer:/var/tmp/app
ssh -t cardputer 'sudo install -m755 /var/tmp/app /usr/share/APPLaunch/bin/MyApp'
```

设备上的 sudo 由交互终端提示输入。CardBili 发布包使用 `scripts/deploy.sh /path/to/package.tar.gz`，脚本上传、校验 SHA-256 并在设备上运行安装程序
