#!/usr/bin/env python3
"""从全字型重新生成 fusion8.ttf 子集，补充字形时改 CHARS 后重跑"""
from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTFont

FONTS = Path(__file__).resolve().parents[1] / "cardbili" / "fonts"
SOURCE = FONTS / "fusion8-full.ttf"
OUTPUT = FONTS / "fusion8.ttf"
# 8px 只用于播放器时间、"弹"标记、卡片统计与观看时间
CHARS = "".join(chr(code) for code in range(0x20, 0x7F)) + "万亿今天昨年月日位已看完弹"

font = TTFont(SOURCE)
options = subset.Options()
options.name_IDs = ["*"]
options.name_legacy = True
options.name_languages = ["*"]
options.layout_features = ["*"]
options.notdef_outline = True
options.recalc_bounds = True
options.recalc_timestamp = False
subsetter = subset.Subsetter(options=options)
subsetter.populate(text=CHARS)
subsetter.subset(font)
font.save(OUTPUT)
print(f"{OUTPUT.name}: {len(TTFont(OUTPUT).getBestCmap())} glyphs")
