#!/usr/bin/env python3
"""将紧密排列的 RGB565 帧缓冲保存为 PNG，依赖 Pillow"""

import argparse
import os

from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", nargs="?", default="/tmp/fbcap.png")
    parser.add_argument("--dev", default="/dev/fb0")
    parser.add_argument("--w", type=int, default=320)
    parser.add_argument("--h", type=int, default=170)
    args = parser.parse_args()
    if args.w <= 0 or args.h <= 0:
        parser.error("宽度和高度必须为正数")
    expected = args.w * args.h * 2
    fd = os.open(args.dev, os.O_RDONLY)
    try:
        data = os.pread(fd, expected, 0)
    finally:
        os.close(fd)
    if len(data) != expected:
        raise SystemExit(f"帧数据不足：{len(data)} / {expected} 字节")
    pixels = bytearray(args.w * args.h * 3)
    for index in range(args.w * args.h):
        value = int.from_bytes(data[index * 2:index * 2 + 2], "little")
        pixels[index * 3:index * 3 + 3] = bytes((
            ((value >> 11) & 31) * 255 // 31,
            ((value >> 5) & 63) * 255 // 63,
            (value & 31) * 255 // 31,
        ))
    Image.frombytes("RGB", (args.w, args.h), bytes(pixels)).save(args.output)
    print(f"已保存 {args.output}，{args.w} × {args.h}")


if __name__ == "__main__":
    main()
