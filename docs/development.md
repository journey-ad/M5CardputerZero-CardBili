# 开发说明

## 目录

```text
cardbili/                 Rust 应用、Slint 界面、字体与图标
  src/api/                Bilibili 接口
  src/app/                页面状态、输入与消息处理
  src/net/session/        会话、设备信息与登录态存储
  src/player/             音视频同步与音量控制
  build/                  位图字体生成与缓存
  ffmpeg/                 精简解码器及参数管道入口
scripts/                  构建、打包与设备安装
.github/workflows/        检查、设备构建与 Release 草稿
skill/cardputer-zero/     可独立复制的设备操作 skill
docs/                     安装、开发、发布说明与公开截图
.secrets/                 本地配置，只提交空值模板
dist/                     本地构建与打包产物，Git 忽略
```

## 工具链

使用当前稳定版 Rust，通过 `Cargo.lock` 固定依赖；CI 与本地构建均使用 `--locked`

macOS Apple Silicon：

```sh
brew install llvm python
python3 -m venv .secrets/venv
.secrets/venv/bin/pip install Pillow
rustup target add aarch64-unknown-linux-musl
```

Linux 的示例以 Ubuntu 24.04 为基础：

```sh
sudo apt-get update
sudo apt-get install -y clang llvm python3-pil pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev
rustup target add aarch64-unknown-linux-musl
```

安装并启动 Docker。x86_64 Linux 构建 ARM64 FFmpeg 时还需要配置 Docker 的 ARM64 模拟执行；CI 直接使用 ARM64 runner

## 设备构建与打包

从仓库根执行：

```sh
bash scripts/build-device.sh
python3 scripts/package-release.py
```

macOS 使用上述虚拟环境时，将最后一行改为 `.secrets/venv/bin/python scripts/package-release.py`

构建脚本自动定位当前 Rust 工具链的 `rust-lld`，macOS 自动使用 Homebrew LLVM；可通过 `CC_aarch64_unknown_linux_musl` 与 `AR_aarch64_unknown_linux_musl` 指定编译器和归档器

产物为 `cardbili/target/aarch64-unknown-linux-musl/release/cardbili` 与 `dist/ffmpeg/cardbili-ffmpeg`。打包生成设备包、含 vendored Cargo 依赖与 FFmpeg 源码的源码包，以及 SHA-256 校验文件

musl 的 dl 符号由 libc 提供，脚本在构建目录生成空 libdl.a，以满足依赖的链接参数

Slint 使用 `~1.8`。`build.rs` 在构建时栅格化图标，`build/ui_fonts.rs` 生成完整中文位图字体，启动时由 `fonts::register` 注册。修改字体后通过构建脚本重新生成缓存

## 桌面预览

```sh
cd cardbili
cargo run --locked --features desktop
```

桌面窗口适合查看界面、导航与布局。macOS 播放使用 PATH 中的 FFmpeg；Linux 播放需要与应用同目录的 `cardbili-ffmpeg` 和 `pacat`。设备帧缓冲、实体键盘和音频延迟仍需真机验证

## 调试入口

| 环境变量 | 用途 |
| --- | --- |
| `BILI_TAB=0..4` | 推荐、热门、动态、我的、设置 |
| `BILI_VIEW=0..2` | 列表、三列、两列 |
| `BILI_SEL=<序号>` | 初始选中项 |
| `BILI_DETAIL=<BV号>` | 打开详情 |
| `BILI_AUTOPLAY=<BV号>`、`BILI_START=<秒>` | 播放与起播位置 |
| `BILI_ABOUT=1`、`BILI_HELP=1` | 关于页、按键说明 |
| `BILI_TEST_LOGIN=1` | 登录页，需要应用凭据 |
| `BILI_FORCE=1` | 强制刷新首页数据 |
| `BILI_DSCROLL=<像素>`、`BILI_DSEND=1` | 详情滚动位置、滚动到底 |
| `BILI_SECONDS=<正数>` | 定时退出 |
| `BILI_DUMP=1` | 输出列表加载信息 |
| `BILI_KEYS=TAB,RIGHT,ENTER` | 桌面预览的顺序按键 |
| `BILI_HOLD=RIGHT`、`BILI_HOLD_MS=1200` | 桌面预览长按 |
| `APPLAUNCH_LINUX_FBDEV_DEVICE` / `BILI_FB` | 设备帧缓冲路径 |
| `APPLAUNCH_LINUX_KEYBOARD_DEVICE` / `BILI_KBD` | 键盘设备路径 |

布尔调试开关按变量是否存在判断，关闭时使用 `unset`。网络调试会访问真实接口，测试账号操作前确认当前登录账号

## 本地数据

- 配置：`${XDG_CONFIG_HOME:-$HOME/.config}/cardbili/`，包含 `settings.json` 与 `auth.dat`
- 状态：`${XDG_STATE_HOME:-$HOME/.local/state}/cardbili/`，包含 `master.key` 与可选的轮转播放日志
- 登录态：XChaCha20-Poly1305 认证加密，主密钥独立存储；Unix 文件权限为 `0600`

能够同时读取登录态和主密钥的进程可以恢复凭据。提交问题反馈时，检查日志中的账号、视频地址及 Cookie，并避开登录二维码和个人页面

## 提交前验证

```sh
cd cardbili
cargo check --locked --features desktop
cd ..
shellcheck scripts/*.sh skill/cardputer-zero/scripts/*.sh
actionlint
gitleaks git --redact --log-opts=--all
```

修改应用逻辑时验证对应页面；涉及设备输入、安装或播放的改动，附上运行环境与真机结果
