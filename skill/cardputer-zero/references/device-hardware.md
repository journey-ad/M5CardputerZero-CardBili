# 硬件与系统规格

## 计算/系统
- SoC：aarch64，4×Cortex-A53（约 Pi Zero 2 级）；RAM 352MB（launcher 在跑时可用约 145MB，停掉 launcher 约 250MB）；swap=zram0 352M。
- OS：Debian 13 (trixie)，kernel 6.18.39+rpt-rpi-v8；SD 卡 29G（用约 6G）。
- GPU 显存 `vcgencmd get_mem gpu` = 128M；有 `/dev/vchiq` 与 `/dev/video0,10-23,31`（硬件 H.264 解码器节点存在，但 ffmpeg 的 h264_v4l2m2m 实测在本机输出为空/失败，需软件解码）。

## 屏幕 / 显示
- 面板：内核 `panel-mipi-dbi` SPI 面板。`/dev/fb0` == `/dev/fb_lcd`，name=`panel-mipi-dbid`，**320x170，16bpp，RGB565，line_length(stride)=640**（=320*2 紧密排列），整帧 108800 字节。
- DRM：`/dev/dri/card1`（`cardputer-zero-internal`），connector `card1-SPI-1` = connected，mode 320x170，enabled；设备用户在 video/render 组，免 root 可开。另有 `/dev/dri/card0` + `renderD128`。
- **内部 320x170 面板不是 wayland 输出**。系统另一套 wayland：labwc 合成器 `wayland-0` = `NOOP-1 "Headless output 2" 1920x1080`（HDMI/虚拟桌面，可用 grim 截图），与面板无关。

## 输入
- 键盘：`/dev/input/event3`（tca8418c），符号链接 `/dev/input/by-path/platform-3f804000.i2c-event`。EV 位=13（无 EV_REL，无鼠标）。
- 键位映射文件：`/usr/share/keymaps/tca8418_keypad_m5stack_keymap.map`（loadkeys 格式，仅映射标点/符号）。详见 key-layout.md。
- 其它输入：event2=gpio_ir_recv，event4=ES8389 音频，event0/1=HDMI。

## 外设
- I2C `/dev/i2c-0..11`；SPI `/dev/spidev0.1` `/dev/spidev0.2`；音频 `/dev/snd`；`/dev/dri/card0 card1 renderD128`。
- 电池：`/sys/class/power_supply/bq27220-0/{capacity,status}`；WiFi：`/proc/net/wireless`。

## 音频
- PipeWire + pipewire-pulse 在跑；sink = `alsa_output.platform-sound.stereo-fallback`。
- `aplay -D hw:1,0` 报 "Device or resource busy"；用 `paplay` 或 ffmpeg `-f pulse default`。

## 已装工具 / 库
- python3.13.5、Pillow 11.1、numpy 2.2.4、pygame 2.6.1
- ffmpeg 7.1.5、ffprobe、ffplay；curl 8.14.1(OpenSSL)、wget；pactl；grim、wlr-randr、scrot
- gcc 14.2、make 4.4.1、git 2.47、pkg-config；`apt install rustc cargo` = 1.85.1
- **缺**：mpv、python3-evdev、paramiko、cmake、xmake、flatpak、docker
- `apt-get` 可用（Debian trixie + archive.raspberrypi 源）

## 字体
- `/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf`（family "Droid Sans Fallback"，CJK 全，但缺拉丁/数字字形）
- `/usr/share/fonts/opentype/noto/NotoSansCJK-*.ttc`（Regular 19.48MB；face0=Noto Sans CJK JP，face2=Noto Sans CJK SC）
- zpix 像素字体（小屏首选）：`/usr/share/fonts/truetype/zpix/zpix.ttf`（family "Zpix"，4.68MB），装后 `fc-cache -f`
- 无 wqy。
