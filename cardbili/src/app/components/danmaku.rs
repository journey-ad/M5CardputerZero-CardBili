use crate::{danmaku, AppWindow};
use slint::{ModelRc, VecModel};
use std::collections::HashMap;

// 弹幕描边位图按文本和颜色缓存
pub(in crate::app) struct DmCache {
    map: HashMap<u64, slint::Image>,
}

impl DmCache {
    pub(in crate::app) fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn image(&mut self, text: &str, color: u32) -> Option<slint::Image> {
        let mut key = 0xcbf2_9ce4_8422_2325u64;
        for b in text.as_bytes() {
            key ^= *b as u64;
            key = key.wrapping_mul(0x100_0000_01b3);
        }
        key ^= (color as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        if let Some(img) = self.map.get(&key) {
            return Some(img.clone());
        }
        let bmp = danmaku::bake(text, color)?;
        let mut pb = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(bmp.w, bmp.h);
        pb.make_mut_bytes().copy_from_slice(&bmp.px);
        let img = slint::Image::from_rgba8(pb);
        if self.map.len() >= 256 {
            self.map.clear();
        }
        self.map.insert(key, img.clone());
        Some(img)
    }
}

// 把引擎算出的弹幕行写进 Slint 模型
pub(in crate::app) fn render_danmaku(
    ui: &AppWindow,
    engine: &danmaku::Engine,
    t: f64,
    cache: &mut DmCache,
) {
    let baked = danmaku::font_available();
    ui.set_dm_baked(baked);
    let mut rows: Vec<crate::DmRow> = Vec::new();
    for r in engine.rows_at(t) {
        // 字体可用时生成描边位图，其余情况使用文字和半透明背景
        let img = if baked {
            cache.image(r.text, r.color)
        } else {
            None
        };
        let use_img = img.is_some();
        rows.push(crate::DmRow {
            text: if use_img { "".into() } else { r.text.into() },
            x: r.x,
            y: r.y,
            w: r.w,
            color: slint::Color::from_rgb_u8(
                ((r.color >> 16) & 0xff) as u8,
                ((r.color >> 8) & 0xff) as u8,
                (r.color & 0xff) as u8,
            ),
            img: img.unwrap_or_default(),
        });
    }
    ui.set_dm_rows(ModelRc::new(VecModel::from(rows)));
}
