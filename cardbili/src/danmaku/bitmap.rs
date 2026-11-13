use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

struct Glyph {
    code: u32,
    offset: u32,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
    advance: u16,
}

include!(concat!(env!("OUT_DIR"), "/danmaku_font.rs"));
static PIXELS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/danmaku_font.bin"));

fn glyph(character: char) -> &'static Glyph {
    GLYPHS
        .binary_search_by_key(&(character as u32), |glyph| glyph.code)
        .map(|index| &GLYPHS[index])
        .unwrap_or(&MISSING_GLYPH)
}

struct Bounds {
    left: i32,
    top: i32,
    width: usize,
    height: usize,
}

fn bounds(text: &str) -> Option<Bounds> {
    let mut left = i32::MAX;
    let mut right = i32::MIN;
    let mut bottom = i32::MAX;
    let mut top = i32::MIN;
    let mut pen = 0;
    for character in text.chars() {
        let glyph = glyph(character);
        if glyph.width > 0 && glyph.height > 0 {
            left = left.min(pen + i32::from(glyph.x));
            right = right.max(pen + i32::from(glyph.x) + i32::from(glyph.width));
            bottom = bottom.min(i32::from(glyph.y));
            top = top.max(i32::from(glyph.y) + i32::from(glyph.height));
        }
        pen += i32::from(glyph.advance);
    }
    (left < right && bottom < top).then(|| Bounds {
        left,
        top,
        width: (right - left + 2) as usize,
        height: (top - bottom + 2) as usize,
    })
}

pub(super) fn width(text: &str) -> f64 {
    bounds(text).map_or(0.0, |bounds| bounds.width as f64)
}

// 整条弹幕合成一次，图片包含前景颜色和 1px 黑色描边
pub(crate) fn bake(text: &str, color: u32) -> Image {
    let Some(bounds) = bounds(text) else {
        return Image::default();
    };
    let mut coverage = vec![false; bounds.width * bounds.height];
    let mut pen = 0;
    for character in text.chars() {
        let glyph = glyph(character);
        let gx = (1 + pen + i32::from(glyph.x) - bounds.left) as usize;
        let gy = (1 + bounds.top - i32::from(glyph.y) - i32::from(glyph.height)) as usize;
        for y in 0..usize::from(glyph.height) {
            for x in 0..usize::from(glyph.width) {
                let bit = y * usize::from(glyph.width) + x;
                if PIXELS[glyph.offset as usize + bit / 8] & (1 << (bit % 8)) != 0 {
                    coverage[(gy + y) * bounds.width + gx + x] = true;
                }
            }
        }
        pen += i32::from(glyph.advance);
    }

    let mut buffer =
        SharedPixelBuffer::<Rgba8Pixel>::new(bounds.width as u32, bounds.height as u32);
    let pixels = buffer.make_mut_slice();
    // 覆盖图向周围 8 个方向扩展一次，外侧预留 1px
    for y in 1..bounds.height - 1 {
        for x in 1..bounds.width - 1 {
            if coverage[y * bounds.width + x] {
                for row in y - 1..=y + 1 {
                    for column in x - 1..=x + 1 {
                        pixels[row * bounds.width + column] = Rgba8Pixel::new(0, 0, 0, 255);
                    }
                }
            }
        }
    }
    let foreground = Rgba8Pixel::new((color >> 16) as u8, (color >> 8) as u8, color as u8, 255);
    for (pixel, covered) in pixels.iter_mut().zip(coverage) {
        if covered {
            *pixel = foreground;
        }
    }
    Image::from_rgba8(buffer)
}
