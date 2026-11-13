// 字形结构与当前锁定的 Slint 1.8 软件渲染器一致
use slint::private_unstable_api::re_exports::{
    BitmapFont, BitmapGlyph, BitmapGlyphs, CharacterMapEntry, Slice,
};

static PIXELS: [u8; include_bytes!(concat!(env!("OUT_DIR"), "/ui_fonts.bin")).len()] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/ui_fonts.bin"));
include!(concat!(env!("OUT_DIR"), "/ui_fonts.rs"));

const fn glyph_pixels(offset: usize, length: usize) -> Slice<'static, u8> {
    Slice::from_slice(PIXELS.split_at(offset).1.split_at(length).0)
}

pub fn register(window: &dyn slint::platform::WindowAdapter) {
    for font in FONTS {
        window.renderer().register_bitmap_font(font);
    }
}
