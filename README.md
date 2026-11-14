# CardBili

面向 M5Stack Cardputer Zero 的非官方 Bilibili 客户端，使用 Rust 与 Slint，在 320 × 170 屏幕上提供视频浏览、播放与弹幕

适用于运行 aarch64 Linux 和 APPLaunch 的 Cardputer Zero；ESP32 系列 Cardputer 需要不同的运行环境

## 功能

- 推荐、热门、动态，以及历史记录、收藏和关注列表
- 视频详情、评论、分 P 与合集浏览
- H.264 / AAC 播放、进度跳转、倍速与弹幕
- 列表、两列和三列布局，支持键盘操作与外观设置
- TV/HD 扫码登录，需要自行配置应用凭据
- macOS / Linux 桌面预览，便于开发界面

## 界面

现有开发版本的设置与关于页面，更多截图说明见 [截图说明](docs/screenshots/README.md)

| 设置 | 关于 |
| --- | --- |
| ![CardBili 设置页面](docs/screenshots/settings.png) | ![CardBili 关于页面](docs/screenshots/about.png) |

## 安装

设备需要 APPLaunch、PulseAudio 或 PipeWire Pulse 服务，以及 `pacat` / `pactl` 和 CA 证书

下载 Release 中配套的设备包、源码包与 `SHA256SUMS`，校验后在设备上解压、安装：

```sh
sha256sum -c SHA256SUMS
tar -xzf cardbili-0.1.0-aarch64-linux-musl.tar.gz
bash cardbili-0.1.0-aarch64-linux-musl/install.sh
```

安装脚本会请求设备的 sudo 授权，并重启当前用户的 APPLaunch 服务。版本号随 Release 更新，完整步骤和 SSH 部署见 [安装说明](docs/installation.md)

## 开发

```sh
rustup target add aarch64-unknown-linux-musl
bash scripts/build-device.sh
python3 scripts/package-release.py
```

需要 Rust、LLVM、Docker 和 Python Pillow。平台依赖、桌面预览、目录结构与调试入口见 [开发说明](docs/development.md)

| 按键 | 操作 |
| --- | --- |
| Z / X / C / F | 左 / 下 / 右 / 上 |
| Tab / Shift+Tab | 切换页签 |
| V | 切换列表布局 |
| Enter / Space | 播放 / 查看详情 |
| Esc | 返回，视频列表中长按 2 秒退出 |
| 屏幕下方 4–8 号物理键 | 执行对应位置的底栏操作，以当前提示为准 |

## 文档与工具

- [安装与登录配置](docs/installation.md)
- [开发与调试](docs/development.md)
- [CI 与发布](docs/releasing.md)
- [第三方许可证](THIRD_PARTY_NOTICES.md)
- [Cardputer Zero skill](skill/cardputer-zero/SKILL.md)：设备访问、显示、按键、部署与调试

skill 可整体复制到支持 `SKILL.md` 的工具目录，保留 `references/` 与 `scripts/`。连接信息从当前项目的 `.secrets/device.env` 或 `DEVICE_ENV_FILE` 指定的文件读取

## 许可证与服务

本项目原创代码、文档和 skill 使用 [MIT](LICENSE)。Slint、FFmpeg、字体与图标保留各自许可证；默认设备发布包中的 CardBili 可执行文件按 GPL-3.0-only 分发，FFmpeg 按 LGPLv3 分发，并提供对应源码，详见 [第三方许可证](THIRD_PARTY_NOTICES.md)

本项目与 Bilibili、M5Stack 无隶属关系。网络功能依赖 Bilibili 接口与账号权限，接口变动可能影响使用；视频、封面及弹幕等内容的权利归各自权利人
