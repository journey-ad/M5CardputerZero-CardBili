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
// 系统接收速度的历史采样
pub(in crate::app) struct StatsState {
    connection: Vec<Option<f64>>,
    cursor: usize,
    last_sample: Option<(Instant, u64)>,
    last_update: Option<Instant>,
}

impl Default for StatsState {
    fn default() -> Self {
        Self {
            connection: vec![None; SLOTS],
            cursor: 0,
            last_sample: None,
            last_update: None,
        }
    }
}

// 系统网卡的累计接收字节
#[cfg(target_os = "linux")]
fn network_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/net/dev").ok()?;
    let mut total = 0;
    for line in text.lines().skip(2) {
        let (interface, counters) = line.split_once(':')?;
        if interface.trim() != "lo" {
            total += counters.split_whitespace().next()?.parse::<u64>().ok()?;
        }
    }
    Some(total)
}

#[cfg(not(target_os = "linux"))]
fn network_bytes() -> Option<u64> {
    None
}

fn color(rgb: [u8; 3]) -> Color {
    Color::from_rgb_u8(rgb[0], rgb[1], rgb[2])
}

fn sample(value: f64) -> MeterSample {
    let thresholds = BANDWIDTH_THRESHOLDS;
    let colors = BANDWIDTH_COLORS;
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

fn build(history: &[Option<f64>], cursor: usize) -> ModelRc<MeterSample> {
    let mut items: Vec<MeterSample> = Vec::with_capacity(SLOTS);
    for index in 0..SLOTS {
        match history[index] {
            Some(value) if index != cursor => items.push(sample(value)),
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
        let Some(info) = self.current_play.as_ref() else {
            return;
        };
        let now = Instant::now();
        if self
            .stats
            .last_update
            .is_some_and(|last| now.duration_since(last).as_secs_f64() < 1.0)
        {
            return;
        }
        self.stats.last_update = Some(now);
        let mut speed_text = "-".to_string();
        if let Some(bytes) = network_bytes() {
            if let Some((at, previous)) = self.stats.last_sample {
                let speed =
                    bytes.saturating_sub(previous) as f64 / now.duration_since(at).as_secs_f64();
                speed_text = bitrate_text(speed * 8.0);
                for offset in 0..2 {
                    self.stats.connection[(self.stats.cursor + offset) % SLOTS] = Some(speed);
                }
                self.stats.cursor = (self.stats.cursor + 2) % SLOTS;
            }
            self.stats.last_sample = Some((now, bytes));
        }
        let tech = &info.tech;
        let lines = vec![
            line("Video Info", format!("{}:{}", info.bvid, info.cid)),
            line("Resolution", format!("{}@{}", tech.quality, tech.fps)),
            line(
                "Codec",
                format!("video/{}, audio/{}", tech.vcodec, tech.acodec),
            ),
            line("Video Host", tech.vhost.clone()),
            line("Audio Host", tech.ahost.clone()),
            line(
                "Bitrate",
                bitrate_text((tech.vbandwidth + tech.abandwidth) as f64),
            ),
            line("Player", format!("CardBili/v{}", env!("CARGO_PKG_VERSION"))),
            line("Date", date_text()),
        ];
        player.set_connection_samples(build(&self.stats.connection, self.stats.cursor));
        player.set_connection_value(speed_text.into());
        player.set_stat_lines(ModelRc::new(VecModel::from(lines)));
    }
}
