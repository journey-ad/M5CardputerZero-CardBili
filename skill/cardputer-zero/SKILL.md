---
name: cardputer-zero
description: 'Work on the M5Stack CardputerZero (Cardputer Zero) handheld Linux device: SSH access, 320x170 RGB565 framebuffer rendering, evdev keyboard input, APPLaunch launcher and app installation, ffmpeg/Bilibili video playback, and cross-compiling Rust+Slint apps from macOS (aarch64-unknown-linux-musl). Use when developing, debugging or deploying anything on the CardputerZero (device address and credentials come from the project's gitignored .secrets/device.env; the IP is assigned by phone hotspot and may change).'
license: MIT
allowed-tools: Bash Read Write Edit
---

# Cardputer Zero 开发技能

面向 M5Stack CardputerZero 的应用开发与调试经验汇总。含硬件/显示/输入参数、启动器与装机流程、视频与 B 站接口、Mac→设备 Rust+Slint 交叉编译、调试套路与踩坑记录。设备 IP、账号、口令等敏感信息统一见项目根 `.secrets/device.env`（该目录已 git 忽略）。

## 何时使用
- 要给 Cardputer Zero 写/改应用、写 `/dev/fb0`、处理键盘输入
- 要装成 APPLaunch 桌面应用（图标 + desktop）
- 要做 B 站播放器 / 调 B 站接口 / ffmpeg 解码
- 要在 Mac 上交叉编译 Rust、Slint 程序给设备跑

## 黄金规则（先记这些，能省很多时间）
1. **屏幕不是 wayland 输出**：内部 320x170 面板由 launcher 直写 `/dev/fb0`（= `/dev/fb_lcd`），`wlr-randr` 只看到无头 1920x1080。第三方程序也必须直接 mmap 写 `/dev/fb0`。RGB565 小端、stride=640、整帧 108800 字节。
2. **没有方向键**：方向键是 `Z`=左、`X`=下、`C`=右、`F`=上；数字键 `1`~`0` 实为 F1~F10（自定义 keycode，见 `references/key-layout.md`）。不要假设标准 evdev 数字码 2..11。
3. **launcher 只打开 `/dev/fb0` 和 `/dev/input/event3`**，且不收 uinput 注入；SSH 无法模拟按键，按键相关必须真机手按验证。
4. **安装应用 = 可执行放到 `/usr/share/APPLaunch/bin/` + 图标 `share/images/` + `applications/*.desktop`**，然后 `systemctl --user restart APPLaunch.service`（需 export XDG_RUNTIME_DIR / DBUS_SESSION_BUS_ADDRESS）。
5. **Rust+Slint 交叉编译**要 `EmbedFiles` + `software-renderer-systemfonts` + 内嵌 zpix 字体，且渲染循环用常驻 buffer + 仅在 `draw_if_needed` 为真时写屏，否则静态页会黑屏。

## 访问设备
见 `references/access.md`（SSH_ASKPASS 免交互登录、r.sh / scp、sudo）。

## 参考资料（按需读）
- `references/device-hardware.md` — 硬件规格、屏幕/DRM、输入设备、外设、电源、已装工具
- `references/key-layout.md` — 物理键盘布局、自定义 keycode 映射、方向键、底栏按键对齐坐标
- `references/launcher-and-apps.md` — APPLaunch 架构、desktop 格式、Exec 策略、装机与内存扫描
- `references/display-rendering.md` — fb0 RGB565 写屏、fbcap 抓帧、DRM 信息、wayland 坑
- `references/video-and-bilibili.md` — ffmpeg 播放配方、B 站取流/WBI 签名/扫码登录、音频
- `references/rust-slint-crosscompile.md` — Mac 交叉编译 aarch64-musl、Slint 配置、中文字体、常见错误
- `references/debugging-playbook.md` — 抓帧调试循环、黑屏/字体子集/僵尸进程等教训
- `scripts/fbcap.py` — 从设备 `/dev/fb0` 抓一帧存 PNG
- `scripts/r.sh` / `scripts/askpass.sh` — 免交互 SSH 包装

## 备注
这是本机自用技能；设备地址与口令不写入本文件，统一放在项目根 `.secrets/device.env`（已 git 忽略），请勿外传。
