// 轻量弹幕引擎：加载时一次性分配赛道，按播放时间取出可见弹幕
pub(crate) mod bitmap;

use std::collections::{HashMap, VecDeque};

use crate::player::WIDTH;

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
    pub own: bool,
}

pub struct Row<'a> {
    pub id: usize,
    pub text: &'a str,
    pub x: f32,
    pub y: f32,
    pub color: u32,
    pub own: bool,
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
    echo_t0: Option<f64>, // 刚发送时的本次回显时间
    last_seen: f64, // 组内最后一次出现时间，用于合并判定
    len: f64,       // 文本像素宽度
    count: u32,     // 合并次数
    color: u32,
    own: bool,
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
            let len = bitmap::width(&d.text);
            if let Some(g) = groups.last_mut() {
                if g.kind == kind && g.text == d.text && g.own == d.own && d.t - g.last_seen <= MERGE_WINDOW {
                    g.count += 1;
                    g.last_seen = d.t;
                    continue;
                }
            }
            groups.push(Group {
                kind,
                text: d.text,
                t0: d.t,
                echo_t0: None,
                last_seen: d.t,
                len,
                count: 1,
                color: d.color,
                own: d.own,
                lane: None,
            });
        }

        for group in &mut groups {
            if group.count > 1 {
                group.text = format!("{} ×{}", group.text, group.count);
                group.len = bitmap::width(&group.text);
            }
        }

        assign_lanes(&mut groups);
        groups.retain(|group| group.lane.is_some());
        let mut engine = Engine { groups: Vec::new(), segments: HashMap::new() };
        for group in groups {
            engine.insert(group);
        }
        engine
    }

    fn insert(&mut self, group: Group) {
        let index = self.groups.len();
        let first = time_slice(group.t0);
        self.groups.push(group);
        self.index(index, first);
    }

    fn index(&mut self, index: usize, first: usize) {
        let group = &self.groups[index];
        // 每个时间片保存可见弹幕的下标，跨分段弹幕同时索引到相邻分段
        let last = time_slice(group.echo_t0.unwrap_or(group.t0) + group.lifetime());
        for slice in first..=last {
            let segment = self.segments.entry(slice / SLICES_PER_SEGMENT)
                .or_insert_with(|| Segment { slices: vec![Vec::new(); SLICES_PER_SEGMENT] });
            segment.slices[slice % SLICES_PER_SEGMENT].push(index);
        }
    }

    pub fn push_own(&mut self, text: String, time: f64, echo_time: f64) {
        let rows = self.rows_at(echo_time);
        let lane = (2..MAX_ROWS).find(|lane| {
            !rows.iter().any(|row| row.y == (AREA_TOP + *lane as f64 * LINE_H) as f32)
        }).unwrap_or(MAX_ROWS - 1);
        self.insert(Group {
            kind: Kind::Scroll, len: bitmap::width(&text), text,
            t0: time, last_seen: time, count: 1, color: 0xffffff,
            echo_t0: Some(echo_time),
            own: true, lane: Some(lane),
        });
    }

    pub fn retain_own(&mut self, previous: Self) {
        for group in previous.groups.into_iter().filter(|group| group.own) {
            if let Some(index) = self.groups.iter().position(|existing| existing.own && existing.text == group.text
                && (existing.t0 - group.t0).abs() <= MERGE_WINDOW)
            {
                let existing = &mut self.groups[index];
                let first = time_slice(existing.echo_t0.unwrap_or(existing.t0) + existing.lifetime()) + 1;
                existing.echo_t0 = group.echo_t0;
                self.index(index, first);
            } else {
                self.insert(group);
            }
        }
    }

    pub fn clear_echoes(&mut self) {
        for group in &mut self.groups {
            group.echo_t0 = None;
        }
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
            if let Some(row) = group.row(index, t) {
                rows.push(row);
            }
        }
        rows
    }
}

impl Group {
    fn row(&self, index: usize, t: f64) -> Option<Row<'_>> {
        let group = self;
        let age = t - group.echo_t0.unwrap_or(group.t0);
        if age < 0.0 || age > group.lifetime() {
            return None;
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
        Some(Row {
            id: index,
            text: &group.text,
            x: x as f32,
            y: y as f32,
            color: group.color,
            own: group.own,
        })
    }

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

#[derive(Default)]
struct Lanes {
    up: [Option<(Kind, f64, f64)>; MAX_ROWS],
    bottom: [Option<(Kind, f64, f64)>; BOTTOM_LANES],
    active: Vec<(f64, f64)>,
}

impl Lanes {
    fn assign(&mut self, group: &mut Group, first_lane: usize) {
        self.active.retain(|(start, life)| start + life > group.t0);
        if self.active.len() >= MAX_LIVE {
            return;
        }
        let pool = if group.kind == Kind::Bottom {
            &mut self.bottom[..]
        } else {
            &mut self.up[..]
        };
        for (lane, occupant) in pool.iter_mut().enumerate().skip(first_lane) {
            let free = match *occupant {
                None => true,
                Some((kind, start, len)) => {
                    let elapsed = group.t0 - start;
                    if kind == Kind::Scroll && group.kind == Kind::Scroll {
                        // 前一条弹幕的右边缘已进入屏幕并留出间距
                        elapsed >= LIFE * (len + GAP) / (WIDTH as f64 + len)
                    } else if kind == Kind::Scroll {
                        elapsed >= LIFE
                    } else if group.kind == Kind::Scroll {
                        elapsed >= 0.4
                    } else {
                        elapsed >= FIX_SECS
                    }
                }
            };
            if free {
                group.lane = Some(lane);
                *occupant = Some((group.kind, group.t0, group.len));
                self.active.push((group.t0, group.lifetime()));
                break;
            }
        }
    }
}

// 按时间顺序分配轨道，每条弹幕的轨道保持固定
fn assign_lanes(groups: &mut [Group]) {
    let mut lanes = Lanes::default();
    for group in groups {
        lanes.assign(group, if group.own && group.kind != Kind::Bottom { 2 } else { 0 });
        if group.own && group.lane.is_none() {
            group.lane = Some(MAX_ROWS - 1);
        }
    }
}

#[derive(Default)]
pub struct LiveEngine {
    groups: VecDeque<(usize, Group)>,
    lanes: Lanes,
    next_id: usize,
}

impl LiveEngine {
    pub fn push_own(&mut self, text: String, time: f64, echo_time: f64) {
        let dm = Dm { t: time, mode: 1, color: 0xffffff, text, own: true };
        let _ = self.try_push(dm, time, 2);
        let group = &mut self.groups.back_mut().unwrap().1;
        group.echo_t0 = Some(echo_time);
        self.lanes.up[group.lane.unwrap()] = Some((group.kind, echo_time, group.len));
    }

    pub fn push(&mut self, dm: Dm, time: f64) {
        if self
            .groups
            .iter()
            .any(|(_, group)| group.text == dm.text && group.own == dm.own
                && time - group.echo_t0.unwrap_or(group.t0) <= if dm.own { LIFE } else { MERGE_WINDOW })
        {
            return;
        }
        let first_lane = if dm.own { 2 } else { 0 };
        let _ = self.try_push(dm, time, first_lane);
    }

    // 返回尚未入场的弹幕，调用方可保留后再次尝试
    pub fn try_push(&mut self, dm: Dm, time: f64, first_lane: usize) -> Result<(), Dm> {
        self.groups
            .retain(|(_, group)| time < group.echo_t0.unwrap_or(group.t0) + group.lifetime());
        let Some(kind) = kind_of(dm.mode) else {
            return Ok(());
        };
        if self.groups.len() >= MAX_LIVE && !dm.own {
            return Err(dm);
        }
        let mut group = Group {
            kind,
            len: bitmap::width(&dm.text),
            text: dm.text,
            color: dm.color,
            own: dm.own,
            t0: time,
            echo_t0: None,
            last_seen: time,
            count: 1,
            lane: None,
        };
        self.lanes.assign(&mut group, first_lane);
        if group.own && group.lane.is_none() {
            group.lane = Some(MAX_ROWS - 1);
        }
        if group.lane.is_some() {
            self.groups.push_back((self.next_id, group));
            self.next_id = self.next_id.wrapping_add(1);
            Ok(())
        } else {
            Err(Dm {
                text: group.text,
                ..dm
            })
        }
    }

    pub fn rows_at(&self, time: f64) -> Vec<Row<'_>> {
        self.groups
            .iter()
            .filter_map(|(id, group)| group.row(*id, time))
            .collect()
    }
}

#[cfg(test)]
mod tests;
