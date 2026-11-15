use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;
use ttf_parser::{colr, Face, GlyphId, RgbaColor};

// Mona 12px 原始像素网格，构建时合成 COLRv0 图层
const SIZE: usize = 12;

pub fn generate() {
    let mono_data = std::fs::read("fonts/Mona12Emoji.ttf").unwrap();
    let color_data = std::fs::read("fonts/Mona12ColorEmoji.ttf").unwrap();
    let mono_face = Face::parse(&mono_data, 0).unwrap();
    let color_face = Face::parse(&color_data, 0).unwrap();
    let mono_font = fontdue::Font::from_bytes(mono_data.as_slice(), Default::default()).unwrap();
    let mono = sequences(&mono_face);
    let color = sequences(&color_face);
    let mut glyphs = BTreeMap::new();
    let mut color_pixels = Vec::new();
    let mut mono_pixels = Vec::new();
    let mut monochrome = Vec::new();
    let mut code = String::from("static SEQUENCES: &[(&str, u16)] = &[\n");
    for (sequence, color_id) in &color {
        if !color_face.is_color_glyph(*color_id) {
            continue;
        }
        let mono_id = mono.get(sequence).copied();
        let count = glyphs.len();
        let index = *glyphs
            .entry((mono_id.map(|id| id.0), color_id.0))
            .or_insert_with(|| {
                let mut painter = Painter {
                    face: &color_face,
                    outline: None,
                    pixels: tiny_skia::Pixmap::new(SIZE as u32, SIZE as u32).unwrap(),
                };
                color_face
                    .paint_color_glyph(
                        *color_id,
                        0,
                        RgbaColor::new(255, 255, 255, 255),
                        &mut painter,
                    )
                    .unwrap();
                let mut mask = [0; SIZE * SIZE];
                if let Some(id) = mono_id {
                    let (metrics, bitmap) = mono_font.rasterize_indexed(id.0, SIZE as f32);
                    place(metrics, &bitmap, 10, |offset, alpha| mask[offset] = alpha);
                }
                // 按可见字形居中，奇数高度与 Fusion 中文字形保持相同的半像素中心
                center_rows(painter.pixels.data_mut(), 4);
                center_rows(&mut mask, 1);
                monochrome.push(mono_id.is_some());
                color_pixels.extend_from_slice(painter.pixels.data());
                mono_pixels.extend_from_slice(&mask);
                count
            });
        writeln!(code, "({sequence:?}, {index}),").unwrap();
    }
    code.push_str("];\n");
    writeln!(code, "static MONOCHROME: &[bool] = &{monochrome:?};").unwrap();
    let output = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(output.join("emoji_font.rs"), code).unwrap();
    std::fs::write(output.join("emoji_color.bin"), color_pixels).unwrap();
    std::fs::write(output.join("emoji_mono.bin"), mono_pixels).unwrap();
}

fn center_rows(pixels: &mut [u8], channels: usize) {
    let stride = SIZE * channels;
    let occupied = |row: &[u8]| {
        row.chunks_exact(channels)
            .any(|pixel| pixel[channels - 1] > 0)
    };
    let Some(top) = pixels.chunks_exact(stride).position(occupied) else {
        return;
    };
    let bottom = pixels.chunks_exact(stride).rposition(occupied).unwrap() + 1;
    let height = bottom - top;
    let target = (SIZE - height).div_ceil(2);
    pixels.copy_within(top * stride..bottom * stride, target * stride);
    pixels[..target * stride].fill(0);
    pixels[(target + height) * stride..].fill(0);
}

// cmap 提供单字符，GSUB 的连字表提供肤色、国旗与 ZWJ 组合
fn sequences(face: &Face<'_>) -> BTreeMap<String, GlyphId> {
    let mut result = BTreeMap::new();
    let mut characters = BTreeMap::new();
    for table in face.tables().cmap.unwrap().subtables {
        if !table.is_unicode() {
            continue;
        }
        table.codepoints(|code| {
            let Some(character) = char::from_u32(code) else {
                return;
            };
            let Some(id) = face.glyph_index(character) else {
                return;
            };
            characters.insert(id.0, character);
            if !character.is_ascii() && !matches!(character, '\u{200d}' | '\u{fe0e}' | '\u{fe0f}') {
                result.insert(character.to_string(), id);
            }
        });
    }
    for lookup in face.tables().gsub.unwrap().lookups {
        for table in lookup
            .subtables
            .into_iter::<ttf_parser::gsub::SubstitutionSubtable<'_>>()
        {
            let ttf_parser::gsub::SubstitutionSubtable::Ligature(table) = table else {
                panic!("Mona 字体需要连字替换表");
            };
            for (&id, &character) in &characters {
                let Some(index) = table.coverage.get(GlyphId(id)) else {
                    continue;
                };
                for ligature in table.ligature_sets.get(index).unwrap() {
                    let mut sequence = character.to_string();
                    for component in ligature.components {
                        sequence.push(characters[&component.0]);
                    }
                    result.insert(sequence.replace('\u{fe0f}', ""), ligature.glyph);
                }
            }
        }
    }
    result
}

fn place(
    metrics: fontdue::Metrics,
    bitmap: &[u8],
    baseline: i32,
    mut pixel: impl FnMut(usize, u8),
) {
    for y in 0..metrics.height {
        for x in 0..metrics.width {
            let px = metrics.xmin + x as i32;
            let py = baseline - metrics.ymin - metrics.height as i32 + y as i32;
            assert!((0..SIZE as i32).contains(&px) && (0..SIZE as i32).contains(&py));
            pixel(
                py as usize * SIZE + px as usize,
                bitmap[y * metrics.width + x],
            );
        }
    }
}

struct Painter<'a> {
    face: &'a Face<'a>,
    outline: Option<tiny_skia::Path>,
    pixels: tiny_skia::Pixmap,
}

impl colr::Painter<'_> for Painter<'_> {
    fn outline_glyph(&mut self, glyph: GlyphId) {
        let mut outline = Outline(tiny_skia::PathBuilder::new());
        self.face.outline_glyph(glyph, &mut outline);
        self.outline = outline.0.finish();
    }
    fn paint(&mut self, paint: colr::Paint<'_>) {
        let colr::Paint::Solid(color) = paint else {
            panic!("Mona 需要纯色图层")
        };
        if let Some(path) = &self.outline {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(color.red, color.green, color.blue, color.alpha);
            paint.anti_alias = false;
            let scale = SIZE as f32 / f32::from(self.face.units_per_em());
            let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, -scale, 0.0, 11.0);
            self.pixels
                .fill_path(path, &paint, tiny_skia::FillRule::Winding, transform, None);
        }
    }
    fn push_clip(&mut self) {
        unreachable!("COLRv0")
    }
    fn push_clip_box(&mut self, _: colr::ClipBox) {
        unreachable!("COLRv0")
    }
    fn pop_clip(&mut self) {
        unreachable!("COLRv0")
    }
    fn push_layer(&mut self, _: colr::CompositeMode) {
        unreachable!("COLRv0")
    }
    fn pop_layer(&mut self) {
        unreachable!("COLRv0")
    }
    fn push_translate(&mut self, _: f32, _: f32) {
        unreachable!("COLRv0")
    }
    fn push_scale(&mut self, _: f32, _: f32) {
        unreachable!("COLRv0")
    }
    fn push_rotate(&mut self, _: f32) {
        unreachable!("COLRv0")
    }
    fn push_skew(&mut self, _: f32, _: f32) {
        unreachable!("COLRv0")
    }
    fn push_transform(&mut self, _: ttf_parser::Transform) {
        unreachable!("COLRv0")
    }
    fn pop_transform(&mut self) {
        unreachable!("COLRv0")
    }
}

struct Outline(tiny_skia::PathBuilder);

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0.quad_to(x1, y1, x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0.cubic_to(x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        self.0.close();
    }
}
