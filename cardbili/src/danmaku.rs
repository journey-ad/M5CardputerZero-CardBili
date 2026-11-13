// 轻量弹幕引擎：加载时一次性分配赛道，按播放时间取出可见弹幕
use std::collections::HashMap;

use crate::player::WIDTH;

const FONT: f64 = 12.0; // 弹幕字号，单位为像素
const LINE_H: f64 = 14.0; // 行高
const AREA_TOP: f64 = 2.0; // 上部弹幕起始位置，标题栏显示时会覆盖此区域
const AREA_BOTTOM: f64 = 168.0; // 底部固定弹幕的下边缘
const MAX_ROWS: usize = 7; // 滚动弹幕和顶部固定弹幕共用的行数
const BOTTOM_LANES: usize = 4; // 底部固定弹幕专用轨道数
const MAX_LIVE: usize = 24; // 同屏最多 24 条，只限制新弹幕入场
const LIFE: f64 = 6.0; // 滚动弹幕横穿屏幕的秒数
const GAP: f64 = 20.0; // 同一轨道中相邻弹幕的最小间距
const FIX_SECS: f64 = 4.0; // 顶部/底部固定弹幕停留时间
const MERGE_WINDOW: f64 = 1.2; // 同文本合并的时间窗口，单位为秒
const SEGMENT_SECONDS: usize = 180;
const SLICES_PER_SECOND: usize = 10;
const SLICES_PER_SEGMENT: usize = SEGMENT_SECONDS * SLICES_PER_SECOND;

#[derive(Clone)]
pub struct Dm {
    pub t: f64,
    pub mode: u8,
    pub color: u32,
    pub text: String,
}

pub struct Row<'a> {
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub color: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Scroll,
    Top,
    Bottom,
}

fn kind_of(mode: u8) -> Option<Kind> {
    match mode {
        1..=3 => Some(Kind::Scroll),
        4 => Some(Kind::Bottom),
        5 => Some(Kind::Top),
        _ => None,
    }
}

// 合并后的弹幕组在加载时分配轨道
struct Group {
    kind: Kind,
    text: String,
    t0: f64,        // 首次出现时间，入场基准
    last_seen: f64, // 组内最后一次出现时间，用于合并判定
    len: f64,       // 文本像素宽度
    count: u32,     // 合并次数
    color: u32,
    lane: Option<usize>, // 分配成功的轨道下标
}

struct Segment {
    slices: Vec<Vec<usize>>,
}

pub struct Engine {
    groups: Vec<Group>,
    segments: HashMap<usize, Segment>,
}

impl Engine {
    pub fn new(items: Vec<Dm>) -> Self {
        let mut items: Vec<Dm> = items
            .into_iter()
            .filter(|d| {
                d.t.is_finite()
                    && d.t >= 0.0
                    && !d.text.trim().is_empty()
                    && kind_of(d.mode).is_some()
            })
            .collect();
        items.sort_by(|a, b| a.t.total_cmp(&b.t));

        // 同种类同文本、间隔很近的弹幕合并成一条，并累计次数
        let mut groups: Vec<Group> = Vec::new();
        for d in items {
            let kind = match kind_of(d.mode) {
                Some(k) => k,
                None => continue,
            };
            let len = crate::util::est_w(&d.text, FONT);
            if let Some(g) = groups.last_mut() {
                if g.kind == kind && g.text == d.text && d.t - g.last_seen <= MERGE_WINDOW {
                    g.count += 1;
                    g.last_seen = d.t;
                    continue;
                }
            }
            groups.push(Group {
                kind,
                text: d.text,
                t0: d.t,
                last_seen: d.t,
                len,
                count: 1,
                color: d.color,
                lane: None,
            });
        }

        assign_lanes(&mut groups);
        groups.retain(|group| group.lane.is_some());
        let mut segments: HashMap<usize, Segment> = HashMap::new();
        for (index, group) in groups.iter_mut().enumerate() {
            if group.count > 1 {
                group.text = format!("{} ×{}", group.text, group.count);
                group.len = crate::util::est_w(&group.text, FONT);
            }

            // 每个时间片保存可见弹幕的下标，跨分段弹幕同时索引到相邻分段
            let first = time_slice(group.t0);
            let last = time_slice(group.t0 + group.lifetime());
            for slice in first..=last {
                let segment = segments
                    .entry(slice / SLICES_PER_SEGMENT)
                    .or_insert_with(|| Segment {
                        slices: vec![Vec::new(); SLICES_PER_SEGMENT],
                    });
                segment.slices[slice % SLICES_PER_SEGMENT].push(index);
            }
        }
        Engine { groups, segments }
    }

    // 按绝对播放时间查询时间片，快进和快退共用同一份索引
    pub fn rows_at(&self, t: f64) -> Vec<Row<'_>> {
        if !t.is_finite() || t < 0.0 {
            return Vec::new();
        }
        let slice = time_slice(t);
        let Some(segment) = self.segments.get(&(slice / SLICES_PER_SEGMENT)) else {
            return Vec::new();
        };
        let indices = &segment.slices[slice % SLICES_PER_SEGMENT];
        let mut rows = Vec::with_capacity(indices.len());
        for &index in indices {
            let group = &self.groups[index];
            let age = t - group.t0;
            if age < 0.0 || age > group.lifetime() {
                continue;
            }
            let width = WIDTH as f64;
            let x = match group.kind {
                Kind::Scroll => width - (width + group.len) * age / LIFE,
                Kind::Top | Kind::Bottom => (width - group.len).max(0.0) / 2.0,
            };
            let lane = group.lane.unwrap_or_default() as f64;
            let y = match group.kind {
                Kind::Bottom => AREA_BOTTOM - (lane + 1.0) * LINE_H,
                _ => AREA_TOP + lane * LINE_H,
            };
            rows.push(Row {
                text: &group.text,
                x: x as f32,
                y: y as f32,
                w: group.len as f32,
                color: group.color,
            });
        }
        rows
    }
}

impl Group {
    fn lifetime(&self) -> f64 {
        match self.kind {
            Kind::Scroll => LIFE,
            Kind::Top | Kind::Bottom => FIX_SECS,
        }
    }
}

fn time_slice(time: f64) -> usize {
    (time * SLICES_PER_SECOND as f64).floor() as usize
}

// 按时间顺序分配轨道，每条弹幕的轨道保持固定
fn assign_lanes(groups: &mut [Group]) {
    let width = WIDTH as f64;
    // 每条赛道记录当前占用者：种类、入场时间、文本宽度
    let mut up: Vec<Option<(Kind, f64, f64)>> = vec![None; MAX_ROWS];
    let mut bot: Vec<Option<(Kind, f64, f64)>> = vec![None; BOTTOM_LANES];
    // 已上屏且未结束的弹幕，用于限制新弹幕入场
    let mut live: Vec<(f64, f64)> = Vec::new();
    for g in groups.iter_mut() {
        // 只统计当前仍在屏幕上的弹幕；满了就丢弃这一条新弹幕，已上屏的不受影响
        live.retain(|(t0, life)| t0 + life > g.t0);
        if live.len() >= MAX_LIVE {
            continue;
        }
        let pool = if g.kind == Kind::Bottom {
            &mut bot
        } else {
            &mut up
        };
        let mut chosen = None;
        for (i, occupant) in pool.iter().enumerate() {
            let free = match *occupant {
                None => true,
                Some((pk, pt, plen)) => {
                    let dt = g.t0 - pt;
                    if pk == Kind::Scroll && g.kind == Kind::Scroll {
                        // 前一条弹幕的右边缘已进入屏幕并留出间距
                        dt >= LIFE * (plen + GAP) / (width + plen)
                    } else if pk == Kind::Scroll {
                        // 固定弹幕等待前一条滚动弹幕移出屏幕
                        dt >= LIFE
                    } else {
                        // 等待前一条固定弹幕结束
                        dt >= FIX_SECS
                    }
                }
            };
            if free {
                chosen = Some(i);
                break;
            }
        }
        if let Some(i) = chosen {
            g.lane = Some(i);
            pool[i] = Some((g.kind, g.t0, g.len));
            let life = match g.kind {
                Kind::Scroll => LIFE,
                Kind::Top | Kind::Bottom => FIX_SECS,
            };
            live.push((g.t0, life));
        }
    }
}

// 描边位图使用外部字体，加载失败时由 Slint Text 绘制
static FONT_FACE: std::sync::OnceLock<Option<fontdue::Font>> = std::sync::OnceLock::new();

fn font_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    if let Ok(p) = std::env::var("BILI_FONT") {
        if !p.is_empty() {
            paths.push(std::path::PathBuf::from(p));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("zpix.ttf"));
        }
    }
    // 开发机上直接跑 target/debug 时也从工程目录找
    if let Some(dir) = option_env!("CARGO_MANIFEST_DIR") {
        paths.push(std::path::Path::new(dir).join("fonts").join("zpix.ttf"));
    }
    paths
}

fn font() -> Option<&'static fontdue::Font> {
    FONT_FACE
        .get_or_init(|| {
            for p in font_paths() {
                if let Ok(bytes) = std::fs::read(&p) {
                    if let Ok(f) =
                        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
                    {
                        return Some(f);
                    }
                }
            }
            None
        })
        .as_ref()
}

pub fn font_available() -> bool {
    font().is_some()
}

pub struct Bitmap {
    pub px: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

// 将弹幕渲染为带 1px 黑色描边的 RGBA 位图
pub fn bake(text: &str, color: u32) -> Option<Bitmap> {
    let f = font()?;
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let px = FONT as f32;
    let lm = f.horizontal_line_metrics(px)?;
    let ascent = lm.ascent;
    let pad = 1i32;

    // 先收集字形度量，算出整句的真实包围盒
    let mut glyphs = Vec::with_capacity(chars.len());
    let mut advances = Vec::with_capacity(chars.len());
    let mut pen = 0.0f32;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_top = f32::INFINITY;
    let mut max_bottom = f32::NEG_INFINITY;
    for &ch in &chars {
        let m = f.metrics(ch, px);
        if m.width > 0 && m.height > 0 {
            let left = pen + m.xmin as f32;
            let right = left + m.width as f32;
            min_x = min_x.min(left);
            max_x = max_x.max(right);
            let top = ascent - (m.ymin + m.height as i32) as f32;
            let bottom = ascent - m.ymin as f32;
            min_top = min_top.min(top);
            max_bottom = max_bottom.max(bottom);
        }
        // 字体自然前进宽度比 12px 略大，封顶到字号，避免汉字之间出现缝
        let adv = m.advance_width.min(px);
        advances.push(adv);
        pen += adv;
        glyphs.push(m);
    }
    if !min_x.is_finite() || !min_top.is_finite() {
        return None;
    }

    let w = (max_x - min_x).ceil() as i32 + pad * 2;
    let h = (max_bottom - min_top).ceil() as i32 + pad * 2;
    if w <= pad * 2 || h <= pad * 2 {
        return None;
    }

    // 硬边覆盖率：像素字体按 12px 栅格化后基本只有 0/255，阈值化消除灰边和虚影
    let mut cov = vec![0u8; (w * h) as usize];
    let x_origin = pad as f32 - min_x;
    let baseline = (pad as f32 - min_top + ascent).round() as i32;
    let mut pen = 0.0f32;
    for (i, &ch) in chars.iter().enumerate() {
        let m = glyphs[i];
        if m.width > 0 && m.height > 0 {
            let (_, bitmap) = f.rasterize(ch, px);
            let gx = (x_origin + pen + m.xmin as f32).round() as i32;
            let gy = baseline - m.ymin - m.height as i32;
            for row in 0..m.height {
                for col in 0..m.width {
                    if bitmap[row * m.width + col] >= 128 {
                        let x = gx + col as i32;
                        let y = gy + row as i32;
                        if x >= 0 && y >= 0 && x < w && y < h {
                            cov[(y * w + x) as usize] = 255;
                        }
                    }
                }
            }
        }
        pen += advances[i];
    }

    // 3x3 膨胀一圈，得到实心黑色描边
    let mut dil = vec![0u8; cov.len()];
    for y in 0..h {
        for x in 0..w {
            let mut hit = false;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && ny >= 0 && nx < w && ny < h && cov[(ny * w + nx) as usize] > 0 {
                        hit = true;
                    }
                }
            }
            if hit {
                dil[(y * w + x) as usize] = 255;
            }
        }
    }

    let cr = ((color >> 16) & 0xff) as u8;
    let cg = ((color >> 8) & 0xff) as u8;
    let cb = (color & 0xff) as u8;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for i in 0..cov.len() {
        // 位图保留完整覆盖率，显示透明度由界面设置控制
        let a = 255;
        let (r, g, b, al) = if cov[i] > 0 {
            (cr, cg, cb, a)
        } else if dil[i] > 0 {
            (0, 0, 0, a)
        } else {
            (0, 0, 0, 0)
        };
        rgba[i * 4] = r;
        rgba[i * 4 + 1] = g;
        rgba[i * 4 + 2] = b;
        rgba[i * 4 + 3] = al;
    }
    Some(Bitmap {
        px: rgba,
        w: w as u32,
        h: h as u32,
    })
}
