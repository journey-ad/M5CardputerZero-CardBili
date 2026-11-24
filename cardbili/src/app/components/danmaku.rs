use crate::{danmaku, AppWindow, DmRow};
use slint::ComponentHandle as _;
use slint::{Image, Model, VecModel};
use std::collections::VecDeque;
use std::rc::Rc;

const CACHE_BYTES: usize = 2 * 1024 * 1024;

struct CachedImage {
    text: String,
    color: u32,
    image: Image,
    bytes: usize,
}

#[derive(Default)]
struct DmCache {
    entries: VecDeque<CachedImage>,
    bytes: usize,
}

impl DmCache {
    fn image(&mut self, text: &str, color: u32, color_emoji: bool) -> Image {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.color == color && entry.text == text)
        {
            let entry = self.entries.remove(index).unwrap();
            let image = entry.image.clone();
            self.entries.push_back(entry);
            return image;
        }

        let image = danmaku::bitmap::bake(text, color, color_emoji);
        let size = image.size();
        let bytes = size.width as usize * size.height as usize * 4
            + text.len()
            + std::mem::size_of::<CachedImage>();
        if bytes <= CACHE_BYTES && size.width > 0 {
            // 队首保存最久未使用的图片，按内存预算逐项淘汰
            while self.bytes + bytes > CACHE_BYTES {
                if let Some(entry) = self.entries.pop_front() {
                    self.bytes -= entry.bytes;
                }
            }
            self.entries.push_back(CachedImage {
                text: text.to_owned(),
                color,
                image: image.clone(),
                bytes,
            });
            self.bytes += bytes;
        }
        image
    }
}

pub(in crate::app) struct DanmakuView {
    model: Rc<VecModel<DmRow>>,
    active: Vec<usize>,
    cache: DmCache,
    color_emoji: bool,
}

impl DanmakuView {
    pub(in crate::app) fn new(ui: &AppWindow, color_emoji: bool) -> Self {
        let model = Rc::new(VecModel::default());
        ui.global::<crate::Danmaku>().set_rows(model.clone().into());
        Self {
            model,
            active: Vec::new(),
            cache: DmCache::default(),
            color_emoji,
        }
    }

    pub(in crate::app) fn clear(&mut self) {
        self.hide();
        self.cache = DmCache::default();
    }

    pub(in crate::app) fn set_color_emoji(&mut self, enabled: bool) {
        self.color_emoji = enabled;
        // 切换样式后重新合成活动弹幕与缓存图片
        self.clear();
    }

    pub(in crate::app) fn hide(&mut self) {
        self.active.clear();
        self.model.set_vec(Vec::new());
    }

    // 把引擎算出的弹幕行写进 Slint 模型
    pub(in crate::app) fn update(&mut self, engine: &danmaku::Engine, time: f64) {
        self.update_rows(engine.rows_at(time));
    }

    pub(in crate::app) fn update_rows(&mut self, rows: Vec<danmaku::Row<'_>>) {
        for index in (0..self.active.len()).rev() {
            if !rows.iter().any(|row| row.id == self.active[index]) {
                self.active.remove(index);
                self.model.remove(index);
            }
        }

        for (index, row) in rows.iter().enumerate() {
            if self.active.get(index) == Some(&row.id) {
                // 活动弹幕复用文字和图片，滚动时更新坐标
                let mut item = self.model.row_data(index).unwrap();
                if item.x != row.x || item.y != row.y {
                    item.x = row.x;
                    item.y = row.y;
                    self.model.set_row_data(index, item);
                }
                continue;
            }

            let image = self.cache.image(row.text, row.color, self.color_emoji);
            self.active.insert(index, row.id);
            self.model.insert(
                index,
                DmRow {
                    text: row.text.into(),
                    x: row.x,
                    y: row.y,
                    color: slint::Color::from_rgb_u8(
                        (row.color >> 16) as u8,
                        (row.color >> 8) as u8,
                        row.color as u8,
                    ),
                    img: image,
                    own: row.own,
                },
            );
        }
    }
}
