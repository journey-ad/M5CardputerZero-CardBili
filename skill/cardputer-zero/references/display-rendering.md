# 显示与截图

## 帧缓冲

已验证的内置面板参数为 320 × 170、16bpp、RGB565 小端、line_length 640，整帧 108800 字节

```python
pixel = ((red >> 3) << 11) | ((green >> 2) << 5) | (blue >> 3)
```

先核对帧缓冲 geometry 和 line_length，再通过 mmap 或 pwrite 写入。CardBili 使用常驻 `Rgb565Pixel` 缓冲，并在 `draw_if_needed` 返回 true 时提交该缓冲；下一帧继续复用已有像素

## 截图

将 skill 的 `scripts/fbcap.py` 复制到设备，安装 Python Pillow 后执行：

```sh
python3 /tmp/fbcap.py /tmp/shot.png --dev /dev/fb0 --w 320 --h 170
```

脚本适用于紧密排列的 RGB565 帧缓冲，输出 PNG。其他 stride 或像素格式需要按设备参数调整。放大查看使用最近邻插值，可保留像素边界

调试截图保存在项目忽略的目录。公开图片前检查登录二维码、账号昵称、UID、个人列表和错误详情
