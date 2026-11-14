# 硬件与系统

项目验证过的环境是 ARM64 Debian、APPLaunch、320 × 170 SPI 面板与 PulseAudio 兼容音频服务。内核版本、内存、设备编号和工具版本以当前设备为准

## 只读检查

```sh
uname -a
cat /etc/os-release
free -h
ls -l /dev/fb* /dev/input/by-path/ /dev/dri/
cat /proc/bus/input/devices
systemctl --user status APPLaunch.service
pactl info
command -v pacat pactl python3
```

## 已验证的设备参数

| 对象 | 参数 |
| --- | --- |
| 内置屏幕 | `/dev/fb0` 或 `/dev/fb_lcd`，320 × 170、RGB565 小端、stride 640 |
| 键盘 | `/dev/input/by-path/platform-3f804000.i2c-event`，曾对应 event3 |
| 帧大小 | 108800 字节 |
| 电池 | `/sys/class/power_supply/bq27220-0/` |
| 音频 | PipeWire Pulse 或 PulseAudio，使用 `pacat` 与 `pactl` |

使用 framebuffer ioctl 或 `fbset -i` 核对几何参数，检查运行用户对帧缓冲和输入设备的读写权限。HDMI 或 Wayland 截图反映对应显示输出，内置 SPI 屏幕按帧缓冲单独读取

设备上的 Python Pillow 用于抓帧。FFmpeg 精简版随 CardBili 发布包安装；通用设备调试可先确认系统 `ffmpeg -version`
