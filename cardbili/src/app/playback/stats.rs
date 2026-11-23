use super::super::*;
use std::ffi::CStr;
use std::time::Instant;

use slint::{Color, ComponentHandle as _, ModelRc, VecModel};

use crate::{MeterSample, StatLine};

// 技术统计图表的阈值与配色
const CHART_HEIGHT: f32 = 11.0;
const SLOTS: usize = 150;
const BANDWIDTH_THRESHOLDS: [f64; 11] = [
    0.0,
    18_750.0,
    37_500.0,
    81_250.0,
    128_000.0,
    256_000.0,
    512_000.0,
    2_048_000.0,
    8_192_000.0,
    32_768_000.0,
    131_072_000.0,
];
const BANDWIDTH_COLORS: [[u8; 3]; 11] = [
    [0, 0, 0],
    [213, 62, 79],
    [244, 109, 67],
    [253, 174, 97],
    [254, 224, 139],
    [230, 245, 152],
    [171, 221, 164],
    [102, 194, 165],
    [50, 136, 189],
    [18, 69, 136],
    [255, 255, 255],
];
// 视频队列容量为 500ms
const BUFFER_THRESHOLDS: [f64; 6] = [0.0, 0.05, 0.1, 0.2, 0.35, 0.5];
const BUFFER_COLORS: [[u8; 3]; 6] = [
    [0, 0, 0],
    [253, 174, 97],
    [230, 245, 152],
    [102, 194, 165],
    [50, 136, 189],
    [255, 255, 255],
];

// 媒体连接速度与待播放视频帧时长的历史采样
pub(in crate::app) struct StatsState {
    connection: Vec<Option<f64>>,
    buffer: Vec<Option<f64>>,
    cursor: usize,
    last_sample: Option<(u64, Instant, u64)>,
    last_update: Option<Instant>,
}

impl Default for StatsState {
    fn default() -> Self {
        Self {
            connection: vec![None; SLOTS],
            buffer: vec![None; SLOTS],
            cursor: 0,
            last_sample: None,
            last_update: None,
        }
    }
}

impl StatsState {
    fn record(
        &mut self,
        now: Instant,
        input: Option<(u64, u64)>,
        buffer_seconds: Option<f64>,
    ) -> Option<f64> {
        let speed = input.and_then(|(generation, bytes)| {
            let (previous_generation, at, previous) = self.last_sample?;
            let elapsed = now.duration_since(at).as_secs_f64();
            (generation == previous_generation && bytes >= previous && elapsed > 0.0)
                .then(|| (bytes - previous) as f64 / elapsed)
        });
        self.last_sample = input.map(|(generation, bytes)| (generation, now, bytes));
        for offset in 0..2 {
            let index = (self.cursor + offset) % SLOTS;
            self.connection[index] = speed;
            self.buffer[index] = buffer_seconds;
        }
        self.cursor = (self.cursor + 2) % SLOTS;
        speed
    }
}

fn color(rgb: [u8; 3]) -> Color {
    Color::from_rgb_u8(rgb[0], rgb[1], rgb[2])
}

fn sample(value: f64, thresholds: &[f64], colors: &[[u8; 3]]) -> MeterSample {
    let last = thresholds.len() - 1;
    if value >= thresholds[last] {
        return MeterSample {
            base: color(colors[last]),
            top: color(colors[last]),
            fill: CHART_HEIGHT,
        };
    }
    let mut band = 0;
    for index in 0..last {
        if value >= thresholds[index] {
            band = index;
        }
    }
    let low = thresholds[band];
    let high = thresholds[band + 1];
    let ratio = ((value - low) / (high - low)).clamp(0.0, 1.0);
    MeterSample {
        base: color(colors[band]),
        top: color(colors[band + 1]),
        fill: ratio as f32 * CHART_HEIGHT,
    }
}

fn build(
    history: &[Option<f64>],
    cursor: usize,
    thresholds: &[f64],
    colors: &[[u8; 3]],
) -> ModelRc<MeterSample> {
    let mut items: Vec<MeterSample> = Vec::with_capacity(SLOTS);
    for index in 0..SLOTS {
        match history[index] {
            Some(value) if index != cursor => items.push(sample(value, thresholds, colors)),
            _ => items.push(MeterSample::default()),
        }
    }
    ModelRc::new(VecModel::from(items))
}

fn bitrate_text(value: f64) -> String {
    if !value.is_finite() || value <= 0.0 {
        return "0 Kbps".to_string();
    }
    if value >= 1_000_000.0 {
        format!("{:.2} Mbps", value / 1_000_000.0)
    } else {
        format!("{:.0} Kbps", value / 1_000.0)
    }
}

fn date_text() -> String {
    #[cfg(unix)]
    use libc::strftime;
    #[cfg(windows)]
    unsafe extern "C" {
        fn strftime(
            buffer: *mut libc::c_char,
            size: libc::size_t,
            format: *const libc::c_char,
            time: *const libc::tm,
        ) -> libc::size_t;
    }

    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        #[cfg(unix)]
        libc::localtime_r(&now, &mut tm);
        #[cfg(windows)]
        libc::localtime_s(&mut tm, &now);
        let mut buffer = [0 as libc::c_char; 128];
        strftime(
            buffer.as_mut_ptr(),
            buffer.len(),
            b"%a %b %d %Y %H:%M:%S GMT%z (%Z)\0".as_ptr() as *const libc::c_char,
            &tm,
        );
        CStr::from_ptr(buffer.as_ptr())
            .to_string_lossy()
            .into_owned()
    }
}

fn line(label: &str, value: String) -> StatLine {
    StatLine {
        label: label.into(),
        value: value.into(),
    }
}

impl App {
    pub(in crate::app) fn update_net_stats(&mut self) {
        let player = self.ui.global::<crate::Player>();
        if !player.get_stats_on() {
            self.stats.last_sample = None;
            self.stats.last_update = None;
            return;
        }
        if self.live.is_none() && self.current_play.is_none() {
            return;
        }
        let now = Instant::now();
        if self
            .stats
            .last_update
            .is_some_and(|last| now.duration_since(last).as_secs_f64() < 1.0)
        {
            return;
        }
        self.stats.last_update = Some(now);
        let snapshot = self.player.as_ref().map(|player| player.statistics());
        let input = self
            .player
            .as_ref()
            .zip(snapshot.as_ref())
            .and_then(|(player, stats)| {
                stats
                    .decoder
                    .received_bytes
                    .map(|bytes| (player.gen, bytes))
            });
        let buffer_seconds = snapshot.as_ref().map(|stats| stats.frames.buffer_seconds());
        let speed = self.stats.record(now, input, buffer_seconds);
        let (id, tech, live) = if let Some(info) = &self.live {
            (line("Room ID", info.room_id.to_string()), &info.tech, true)
        } else if let Some(info) = &self.current_play {
            (
                line("Video Info", format!("{}:{}", info.bvid, info.cid)),
                &info.tech,
                false,
            )
        } else {
            return;
        };
        let fps = snapshot
            .as_ref()
            .and_then(|stats| stats.decoder.source_fps)
            .map(|fps| format!("{fps:.2}"))
            .unwrap_or_else(|| {
                if tech.fps.is_empty() {
                    "-".into()
                } else {
                    tech.fps.clone()
                }
            });
        let frames = snapshot.as_ref().map_or_else(
            || "- dropped of -".into(),
            |stats| {
                format!(
                    "{} dropped of {}",
                    stats.frames.dropped, stats.frames.received
                )
            },
        );
        let mut head = vec![
            id,
            line(
                "Resolution",
                format!("{}@{fps}, {frames}", tech.quality.replace('×', "x")),
            ),
            line(
                "Codec",
                format!("video/{}, audio/{}", tech.vcodec, tech.acodec),
            ),
            line(
                if live { "Stream Host" } else { "Video Host" },
                tech.vhost.clone(),
            ),
        ];
        if !live {
            head.push(line("Audio Host", tech.ahost.clone()));
        }
        let bitrate = snapshot
            .as_ref()
            .and_then(|stats| stats.decoder.source_bitrate)
            .unwrap_or((tech.vbandwidth + tech.abandwidth) as f64);
        let bitrate = if bitrate > 0.0 {
            bitrate_text(bitrate)
        } else {
            "-".into()
        };
        let debug = self.player.as_ref().zip(snapshot.as_ref()).map_or_else(
            || format!("loading; retries:{}", self.live_recovery.attempts),
            |(player, stats)| {
                let state = if player.finished {
                    "ended"
                } else if player.paused {
                    "paused"
                } else if player.stalled() {
                    "stalled"
                } else if stats.frames.waiting {
                    "waiting"
                } else {
                    "playing"
                };
                format!(
                    "{state}; queue:{}/{}; errors:{}\nretries:{}; bitrate:{bitrate}",
                    stats.frames.queued,
                    stats.frames.capacity,
                    stats.decoder.errors,
                    self.live_recovery.attempts,
                )
            },
        );
        let tail = vec![
            line("Player", format!("CardBili/v{}", env!("CARGO_PKG_VERSION"))),
            line("Date", date_text()),
        ];
        player.set_connection_samples(build(
            &self.stats.connection,
            self.stats.cursor,
            &BANDWIDTH_THRESHOLDS,
            &BANDWIDTH_COLORS,
        ));
        player.set_buffer_samples(build(
            &self.stats.buffer,
            self.stats.cursor,
            &BUFFER_THRESHOLDS,
            &BUFFER_COLORS,
        ));
        player.set_connection_value(
            speed
                .map_or_else(|| "-".into(), |speed| bitrate_text(speed * 8.0))
                .into(),
        );
        player.set_buffer_value(
            buffer_seconds
                .map_or_else(|| "-".into(), |seconds| format!("{seconds:.2}s"))
                .into(),
        );
        player.set_stat_head(ModelRc::new(VecModel::from(head)));
        player.set_stat_tail(ModelRc::new(VecModel::from(tail)));
        player.set_stat_mystery(line("Debug", debug));
    }
}

#[cfg(test)]
mod tests;
