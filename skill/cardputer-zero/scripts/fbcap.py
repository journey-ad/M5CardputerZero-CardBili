#!/usr/bin/env python3
"""从 CardputerZero 的 /dev/fb0 抓一帧存 PNG。
用法: python3 fbcap.py /tmp/shot.png [--dev /dev/fb0] [--w 320] [--h 170]
依赖: numpy, PIL
"""
import sys, os
import numpy as np
from PIL import Image

def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/fbcap.png"
    dev = "/dev/fb0"; w = 320; h = 170
    a = sys.argv[2:]
    for i, v in enumerate(a):
        if v == "--dev": dev = a[i+1]
        elif v == "--w": w = int(a[i+1])
        elif v == "--h": h = int(a[i+1])
    n = w * h * 2
    fd = os.open(dev, os.O_RDONLY)
    try:
        data = os.pread(fd, n, 0)
    finally:
        os.close(fd)
    if len(data) < n:
        raise SystemExit(f"short read {len(data)} < {n}")
    v = np.frombuffer(data, dtype="<u2").reshape(h, w)
    r = ((v >> 11) & 0x1F).astype(np.uint8) << 3
    g = ((v >> 5) & 0x3F).astype(np.uint8) << 2
    b = (v & 0x1F).astype(np.uint8) << 3
    img = np.dstack([r, g, b])
    Image.fromarray(img, "RGB").save(out)
    print(f"saved {out} {w}x{h}")

if __name__ == "__main__":
    main()
