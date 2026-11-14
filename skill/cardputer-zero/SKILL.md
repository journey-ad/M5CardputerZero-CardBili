---
name: cardputer-zero
description: 为运行 Linux 与 APPLaunch 的 M5Stack Cardputer Zero 开发、调试和部署应用，处理 SSH 连接、RGB565 帧缓冲、实体键盘、Rust/Slint 构建及音视频播放。适用于 Cardputer Zero 设备任务
license: MIT
---

# Cardputer Zero

适用于 aarch64 Linux 设备，屏幕为 320 × 170。先检查设备镜像、节点和服务，再使用对应参考；文档中的实测参数来自项目开发设备

## 操作入口

- 连接设备或执行命令：读 [access.md](references/access.md)，使用 `scripts/r.sh`
- 安装应用或管理启动器：读 [launcher-and-apps.md](references/launcher-and-apps.md)
- 检查硬件与系统：读 [device-hardware.md](references/device-hardware.md)
- 渲染或截图：读 [display-rendering.md](references/display-rendering.md)，使用 `scripts/fbcap.py`
- 按键映射：读 [key-layout.md](references/key-layout.md)
- Rust / Slint 构建：读 [rust-slint-crosscompile.md](references/rust-slint-crosscompile.md)
- 播放与 Bilibili 登录：读 [video-and-bilibili.md](references/video-and-bilibili.md)
- 黑屏、缺字或播放故障：读 [debugging-playbook.md](references/debugging-playbook.md)

## 设备约束

1. 内置屏幕使用帧缓冲，已验证配置为 RGB565 小端、320 × 170、stride 640；截图读取对应帧缓冲节点
2. 方向键对应 Z / X / C / F，屏幕下方数字键使用设备自定义键码，按实际事件确认映射
3. APPLaunch 是当前设备用户的 systemd 服务，使用 `systemctl --user`，运行时目录按 `id -u` 获取
4. 调试直接写屏前停止 APPLaunch，结束后恢复；精确记录并终止自己启动的进程
5. 设备连接信息由当前项目 `.secrets/device.env` 或 `DEVICE_ENV_FILE` 提供，SSH 使用正常的主机密钥检查

## 使用边界

复制完整 skill 目录即可复用。`scripts/deploy.sh` 接收 CardBili 发布包并运行其中的安装脚本，使用前检查目标设备和包来源；其他应用按各自的安装约定处理

技能中的命令用于当前用户请求的工作。设备重启、安装系统包、修改服务或登录账号时，沿用用户已授权的范围。截图和日志在公开前检查账号标识、登录二维码与凭据
