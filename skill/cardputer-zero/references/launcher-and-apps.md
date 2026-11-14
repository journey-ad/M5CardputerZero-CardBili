# APPLaunch 与安装

APPLaunch 使用 systemd 用户服务，应用通过 `/usr/share/APPLaunch/applications/*.desktop` 注册

```ini
[Desktop Entry]
Type=Application
Name=MyApp
Exec=/usr/share/APPLaunch/bin/MyApp
Icon=share/images/myapp_100.png
Terminal=false
Categories=Media;
```

可执行文件位于 `bin/`，图标位于 `share/images/`。已验证的启动器会检查 Exec 路径与 shell 元字符，优先使用允许目录中的直接可执行路径

## 服务管理

在设备当前登录用户下执行：

```sh
XDG_RUNTIME_DIR=/run/user/$(id -u)
export XDG_RUNTIME_DIR
export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus
systemctl --user status APPLaunch.service
```

按当前任务使用 `stop`、`start` 或 `restart`。重启会返回启动器，需要重新选择应用

## CardBili 安装

使用 `scripts/deploy.sh /path/to/cardbili-<version>-aarch64-linux-musl.tar.gz`。脚本接收发布包，上传到设备临时目录，核对 SHA-256，并通过交互终端运行包内 `install.sh`

安装内容包括主程序、解码器、desktop 文件、两种尺寸图标和许可证。sudo 授权由设备终端提示，SSH 凭据按 [access.md](access.md) 配置

安装后检查文件权限、用户服务状态和 journal，最终在设备启动器中确认图标并打开应用
