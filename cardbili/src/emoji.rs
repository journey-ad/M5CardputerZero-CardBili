use crate::{AppWindow, EmojiFont, EmojiPlacement};
use i_slint_core::textlayout::{FontMetrics, TextLayout, TextParagraphLayout, TextShaper};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::ops::ControlFlow;

pub(crate) const SIZE: usize = 12;
pub(crate) const SPACING: i16 = 1;
pub(crate) const ADVANCE: u16 = SIZE as u16 + 2 * SPACING as u16;
pub(crate) const PLACEHOLDER: char = '\u{fffc}';
static COLOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/emoji_color.bin"));
static MONO: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/emoji_mono.bin"));
include!(concat!(env!("OUT_DIR"), "/emoji_font.rs"));

#[derive(Clone, Copy)]
pub(crate) enum Token {
    Character(char),
    Emoji(u16),
}

// 外部表情图使用 65536 起的字形编号
const EXTERNAL_BASE: i32 = 0x1_0000;

#[derive(Default)]
struct Emotes {
    images: Vec<Image>,
    index: HashMap<String, usize>,
}

thread_local! {
    static EMOTES: RefCell<Emotes> = RefCell::new(Emotes::default());
}

pub(crate) fn has_emote(name: &str) -> bool {
    EMOTES.with(|emotes| emotes.borrow().index.contains_key(name))
}

pub(crate) fn register_emote(ui: &AppWindow, name: String, (w, h, raw): crate::cache::Pixels) {
    let image = Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        &raw, w, h,
    ));
    EMOTES.with(|emotes| {
        let mut emotes = emotes.borrow_mut();
        if let Some(&index) = emotes.index.get(&name) {
            emotes.images[index] = image;
        } else {
            let index = emotes.images.len();
            emotes.images.push(image);
            emotes.index.insert(name, index);
        }
    });
    let global = ui.global::<EmojiFont>();
    global.set_revision(global.get_revision() + 1);
}

// 最长匹配完整表情，VS16 使用表情字形，VS15 保留文字字形
pub(crate) fn tokens(mut text: &str) -> impl Iterator<Item = Token> + '_ {
    std::iter::from_fn(move || loop {
        let first = text.chars().next()?;
        if matches!(first, '\u{fe0e}' | '\u{fe0f}' | '\u{200d}') {
            text = &text[first.len_utf8()..];
            continue;
        }
        let mut matched = None;
        if text[first.len_utf8()..].starts_with('\u{fe0e}') {
            text = &text[first.len_utf8() + '\u{fe0e}'.len_utf8()..];
            return Some(Token::Character(first));
        }
        let first_text = &text[..first.len_utf8()];
        let index = SEQUENCES.partition_point(|entry| entry.0 < first_text);
        if !SEQUENCES
            .get(index)
            .is_some_and(|entry| entry.0.starts_with(first))
        {
            text = &text[first.len_utf8()..];
            return Some(Token::Character(first));
        }
        let mut prefix = String::new();
        for (offset, character) in text.char_indices() {
            if character == '\u{fe0f}' {
                if let Some((glyph, bytes)) = matched {
                    if bytes == offset {
                        matched = Some((glyph, offset + character.len_utf8()));
                    }
                }
                continue;
            }
            prefix.push(character);
            match SEQUENCES.binary_search_by(|entry| entry.0.cmp(prefix.as_str())) {
                Ok(index) => matched = Some((SEQUENCES[index].1, offset + character.len_utf8())),
                Err(index) => {
                    if !SEQUENCES
                        .get(index)
                        .is_some_and(|entry| entry.0.starts_with(&prefix))
                    {
                        break;
                    }
                }
            }
        }
        if let Some((glyph, bytes)) = matched {
            text = &text[bytes..];
            return Some(Token::Emoji(glyph));
        }
        text = &text[first.len_utf8()..];
        return Some(Token::Character(first));
    })
}

fn prepare(text: &str) -> (String, Vec<(usize, i32)>) {
    let mut prepared = String::with_capacity(text.len());
    let mut emoji = Vec::new();
    EMOTES.with(|emotes| {
        let emotes = emotes.borrow();
        for chunk in text.split_inclusive(']') {
            let matched = chunk.rfind('[').and_then(|start| {
                emotes
                    .index
                    .get(&chunk[start..])
                    .map(|&index| (start, index))
            });
            let end = matched.map_or(chunk.len(), |(start, _)| start);
            for token in tokens(&chunk[..end]) {
                match token {
                    Token::Character(character) => prepared.push(character),
                    Token::Emoji(glyph) => {
                        emoji.push((prepared.len(), i32::from(glyph)));
                        prepared.push(PLACEHOLDER);
                    }
                }
            }
            if let Some((_, index)) = matched {
                emoji.push((prepared.len(), EXTERNAL_BASE + index as i32));
                prepared.push(PLACEHOLDER);
            }
        }
    });
    (prepared, emoji)
}

pub(crate) fn pixel(
    glyph: u16,
    offset: usize,
    colored: bool,
    foreground: Rgba8Pixel,
) -> Rgba8Pixel {
    let offset = usize::from(glyph) * SIZE * SIZE + offset;
    // 单色字库缺少的组合保留原始彩色字形
    if colored || !MONOCHROME[usize::from(glyph)] {
        let pixel = &COLOR[offset * 4..offset * 4 + 4];
        Rgba8Pixel::new(pixel[0], pixel[1], pixel[2], pixel[3])
    } else {
        Rgba8Pixel::new(foreground.r, foreground.g, foreground.b, MONO[offset])
    }
}

pub(crate) fn install(ui: &AppWindow) {
    let global = ui.global::<EmojiFont>();
    global.on_monochrome(|glyph| MONOCHROME.get(glyph as usize).copied().unwrap_or(false));
    global.on_prepare(|text, _revision| prepare(&text).0.into());
    global.on_layout(
        |text, _revision, size, width, height, horizontal, vertical, wrap, overflow| {
            let (text, emoji) = prepare(&text);
            if emoji.is_empty() || width <= 0.0 || height <= 0.0 {
                return Default::default();
            }
            let font = crate::fonts::LayoutFont::new(size);
            let placeholder = font.glyph_for_char(PLACEHOLDER).unwrap().glyph_id.unwrap();
            let paragraph = TextParagraphLayout {
                string: &text,
                layout: TextLayout {
                    font: &font,
                    letter_spacing: None,
                },
                max_width: width as i16,
                max_height: height as i16,
                horizontal_alignment: horizontal,
                vertical_alignment: vertical,
                wrap,
                overflow,
                single_line: false,
            };
            let mut placements = Vec::new();
            let _ = paragraph.layout_lines::<()>(
                |glyphs, x, y, _, _| {
                    let y = y + (font.height() - SIZE as i16) / 2;
                    for glyph in glyphs {
                        if glyph.glyph_id != placeholder
                            || y + SIZE as i16 <= 0
                            || y >= height as i16
                        {
                            continue;
                        }
                        if let Ok(index) =
                            emoji.binary_search_by_key(&glyph.text_byte_offset, |entry| entry.0)
                        {
                            placements.push(EmojiPlacement {
                                glyph: emoji[index].1,
                                x: f32::from(x + glyph.x + SPACING),
                                y: f32::from(y),
                            });
                        }
                    }
                    ControlFlow::Continue(())
                },
                None,
            );
            std::rc::Rc::new(slint::VecModel::from(placements)).into()
        },
    );
    // 小图缓存最多 128 个，像素数据约占 72 KiB
    let cache = RefCell::new(VecDeque::<(u16, bool, Image)>::new());
    global.on_glyph(move |glyph, colored, _revision| {
        if glyph >= EXTERNAL_BASE {
            return EMOTES.with(|emotes| {
                emotes
                    .borrow()
                    .images
                    .get((glyph - EXTERNAL_BASE) as usize)
                    .cloned()
                    .unwrap_or_default()
            });
        }
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache
            .iter()
            .position(|entry| entry.0 == glyph as u16 && entry.1 == colored)
        {
            let entry = cache.remove(index).unwrap();
            let image = entry.2.clone();
            cache.push_back(entry);
            return image;
        }
        let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(SIZE as u32, SIZE as u32);
        for (offset, target) in buffer.make_mut_slice().iter_mut().enumerate() {
            *target = pixel(
                glyph as u16,
                offset,
                colored,
                Rgba8Pixel::new(255, 255, 255, 255),
            );
        }
        let image = Image::from_rgba8(buffer);
        if cache.len() == 128 {
            cache.pop_front();
        }
        cache.push_back((glyph as u16, colored, image.clone()));
        image
    });
}
