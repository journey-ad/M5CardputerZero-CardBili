# 显示与写屏

## 直接写 framebuffer（首选）
- `/dev/fb0`（= `/dev/fb_lcd`）320x170，RGB565 小端，line_length 640，整帧 **108800 字节**。
- 简单方案：`os.pwrite(fd, frame_bytes, 0)` 即可正确显示（紧密排列）。
- mmap 方案：`FBIOGET_VSCREENINFO=0x4600`、`FBIOGET_FSCREENINFO=0x4602` 取 geometry；按 `line_length` 逐行或整块写。

## RGB565 打包（numpy）
```python
r = (R >> 3) << 11
g = (G >> 2) << 5
b = (B >> 3)
rgb565 = (r | g | b).astype('<u2')   # 小端
```

## 抓帧调试（fbcap.py）
```sh
python3 /tmp/fbcap.py /tmp/shot.png   # 读 /dev/fb0 108800B -> RGB565 解包 -> PNG
```
见 `scripts/fbcap.py`。放大查看：PIL `resize((960,510), Image.NEAREST).save('shot_big.png')`。

## 重要坑
- **静态页黑屏**：渲染循环不要每帧新建全零 buffer 并无条件写屏；要分配常驻 buffer，且只有 `draw_if_needed` 返回 true 时才把 buffer 写到 `/dev/fb0`。否则未重绘的帧会用零缓冲覆盖屏幕变黑。
- 面板不是 wayland 输出，不要试图用 wayland 截图/窗口方案；`wlr-randr` 只会看到 NOOP 1920x1080。
- DRM 也可用（`/dev/dri/card1`，connector card1-SPI-1 mode 320x170），但最省事仍是 fb0 直写。
