#!/usr/bin/env python3
"""将桌面截图嵌入 Cardputer Zero 外壳，生成主图和 WebP 动图"""

from pathlib import Path

from PIL import Image, ImageSequence


ROOT = Path(__file__).resolve().parents[1]
SCREENSHOTS = ROOT / "docs/screenshots"
OUTPUT = ROOT / "docs/media"
SCREEN_SIZE = (320, 170)
SCREEN_SCALE = 2
DEMO_SCREEN_SCALE = 1
PAGE_SPEED = 2
VIDEO_SPEED = 4 / 3
SOURCE_OPENING = (270, 38, 826, 332)
PAGES = (
    ("home", 2500),
    ("popular", 1800),
    ("dynamic", 1800),
    ("mine", 1800),
    ("settings", 1800),
    ("author", 2200),
    ("author-roxy", 2200),
    ("episodes", 1800),
    ("detail", 2200),
    ("comments", 1800),
)


def make_frame(screen_scale):
    source = Image.open(ROOT / "docs/C154-CardputerZero-frame.png").convert("RGBA")
    left, top, right, bottom = SOURCE_OPENING
    width = SCREEN_SIZE[0] * screen_scale
    height = SCREEN_SIZE[1] * screen_scale
    scale_x = width / (right - left)
    scale_y = height / (bottom - top)
    offset = (round(left * scale_x), round(top * scale_y))
    size = (round(source.width * scale_x), round(source.height * scale_y))
    frame = source.transform(
        size,
        Image.Transform.AFFINE,
        (
            1 / scale_x,
            0,
            left - offset[0] / scale_x,
            0,
            1 / scale_y,
            top - offset[1] / scale_y,
        ),
        Image.Resampling.BICUBIC,
    )
    return frame, offset, (width, height)


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    frame, offset, screen_size = make_frame(SCREEN_SCALE)

    def compose(screenshot):
        if screenshot.size != SCREEN_SIZE:
            raise ValueError("截图尺寸应为 320×170")
        image = Image.new("RGBA", frame.size, "white")
        screen = screenshot.convert("RGBA").resize(screen_size, Image.Resampling.NEAREST)
        image.paste(screen, offset)
        image.alpha_composite(frame)
        return image.convert("RGB")

    with Image.open(SCREENSHOTS / "home.png") as screenshot:
        compose(screenshot).save(OUTPUT / "cardbili-hero.png", optimize=True)

    frame, offset, screen_size = make_frame(DEMO_SCREEN_SCALE)
    frames = []
    durations = []
    for page, duration in PAGES:
        with Image.open(SCREENSHOTS / f"{page}.png") as screenshot:
            frames.append(compose(screenshot))
        durations.append(round(duration / PAGE_SPEED))

    with Image.open(SCREENSHOTS / "player.webp") as recording:
        elapsed = 0
        scaled_elapsed = 0
        for index, screenshot in enumerate(ImageSequence.Iterator(recording)):
            screenshot.load()
            duration = screenshot.info["duration"]
            if index < recording.n_frames - 1:
                elapsed += duration
                next_elapsed = round(elapsed / VIDEO_SPEED)
                duration = next_elapsed - scaled_elapsed
                scaled_elapsed = next_elapsed
            durations.append(duration)
            frames.append(compose(screenshot))

    frames[0].save(
        OUTPUT / "cardbili-demo.webp",
        save_all=True,
        append_images=frames[1:],
        duration=durations,
        loop=0,
        lossless=True,
        minimize_size=True,
        method=6,
    )
    print(f"已生成主图，WebP 动图尺寸：{frame.width}×{frame.height}")
    print(f"屏幕区域：{screen_size[0]}×{screen_size[1]}，位置：{offset}")


if __name__ == "__main__":
    main()
