#!/usr/bin/env python3
"""比较 framebuffer 提交频率与 SPI 实际发送量"""

import argparse
import json
import mmap
import os
from pathlib import Path
import statistics
import time


def measure(fps, seconds):
    width, height = 320, 170
    size = width * height * 2
    counter = Path("/sys/bus/spi/devices/spi0.0/statistics/bytes_tx")
    # 灰色背景中的窄条平移，每次提交完整 RGB565 帧
    frames = []
    for x in range(0, width - 8, 8):
        row = b"\x08\x42" * x + b"\x10\x84" * 8 + b"\x08\x42" * (width - x - 8)
        frames.append(row * height)
    with open("/dev/fb0", "r+b", buffering=0) as device:
        with mmap.mmap(device.fileno(), size) as display:
            start = time.monotonic()
            next_frame = start
            timings = []
            count = 0
            sent = None
            while time.monotonic() - start < seconds + 2:
                now = time.monotonic()
                if sent is None and now - start >= 2:
                    sent = int(counter.read_text())
                    measured_at = now
                begin = time.monotonic()
                display[:] = frames[count % len(frames)]
                os.fdatasync(device.fileno())
                if sent is not None:
                    timings.append((time.monotonic() - begin) * 1000)
                count += 1
                next_frame += 1 / fps
                time.sleep(max(0, next_frame - time.monotonic()))
            elapsed = time.monotonic() - measured_at
            byte_count = int(counter.read_text()) - sent
    timings.sort()
    return {
        "requested_fps": fps,
        "seconds": elapsed,
        "submitted_fps": len(timings) / elapsed,
        "spi_bytes": byte_count,
        "spi_full_frames_per_second": byte_count / size / elapsed,
        "submit_mean_ms": statistics.mean(timings),
        "submit_p95_ms": timings[int((len(timings) - 1) * 0.95)],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=int, default=10)
    args = parser.parse_args()
    for rate in [30, 60]:
        print(json.dumps(measure(rate, args.seconds)), flush=True)
