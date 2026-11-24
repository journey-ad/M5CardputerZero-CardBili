use std::sync::Mutex;

#[derive(Clone, Copy, Default)]
pub(crate) struct DecoderSnapshot {
    pub received_bytes: Option<u64>,
    pub source_fps: Option<f64>,
    pub errors: u64,
}

#[derive(Default)]
struct State {
    snapshot: DecoderSnapshot,
    buffered: [Vec<(f64, f64)>; 2],
    starts: [Option<f64>; 2],
    pending: [Option<f64>; 2],
}

impl State {
    fn packet(&mut self, stream: usize, start: f64, duration: f64) {
        if !start.is_finite() || !duration.is_finite() || duration < 0.0 {
            return;
        }
        // 缺少单包时长时，相邻包的时间戳确定上一包覆盖的范围
        if let Some(previous) = self.pending[stream].take() {
            if start > previous {
                self.add_range(stream, previous, start - previous);
            }
        }
        if duration > 0.0 {
            self.add_range(stream, start, duration);
        } else {
            self.pending[stream] = Some(start);
        }
    }

    fn add_range(&mut self, stream: usize, start: f64, duration: f64) {
        // 原始音视频输出从零计时，跳转前的预读包保留目标位置作为零点
        self.starts[stream] = Some(self.starts[stream].unwrap_or(start).min(start).max(0.0));
        let ranges = &mut self.buffered[stream];
        let mut end = start + duration;
        let mut start = start;
        // FLV 时间戳以毫秒取整，相邻包允许 2ms 的舍入误差
        let first = ranges.partition_point(|&(_, end)| end + 0.002 < start);
        let mut last = first;
        while last < ranges.len() && ranges[last].0 <= end + 0.002 {
            start = start.min(ranges[last].0);
            end = end.max(ranges[last].1);
            last += 1;
        }
        ranges.splice(first..last, [(start, end)]);
    }
}

#[derive(Default)]
pub(crate) struct DecoderStats(Mutex<State>);

impl DecoderStats {
    pub fn snapshot(&self) -> DecoderSnapshot {
        self.0.lock().unwrap().snapshot
    }

    // 当前播放位置之后，音视频共同具备的连续媒体时长
    pub fn buffer_seconds(&self, position: f64) -> f64 {
        let mut state = self.0.lock().unwrap();
        let starts = state.starts;
        state
            .buffered
            .iter_mut()
            .zip(starts)
            .map(|(ranges, start)| {
                let position = position + start.unwrap_or(0.0);
                ranges.retain(|&(_, end)| end > position);
                ranges
                    .first()
                    .filter(|&&(start, _)| start <= position + 0.002)
                    .map_or(0.0, |&(_, end)| (end - position).max(0.0))
            })
            .fold(f64::INFINITY, f64::min)
    }

    // FFmpeg 的进度字段与错误日志共用 stderr；返回 true 表示该行是进度字段，不写入日志
    pub fn observe(&self, line: &str) -> bool {
        if let Some((key, value)) = line.split_once('=') {
            if !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                let mut state = self.0.lock().unwrap();
                let stats = &mut state.snapshot;
                match key {
                    "cardbili_rx_bytes" => {
                        if let Ok(bytes) = value.parse() {
                            stats.received_bytes = Some(bytes);
                        }
                    }
                    "cardbili_source_fps" => {
                        if let Some(fps) = frame_rate(value) {
                            stats.source_fps = Some(fps);
                        }
                    }
                    "cardbili_buffer_video" | "cardbili_buffer_audio" => {
                        if let Some((start, duration)) = value.split_once(',') {
                            if let (Ok(start), Ok(duration)) =
                                (start.parse::<i64>(), duration.parse::<i64>())
                            {
                                state.packet(
                                    usize::from(key.ends_with("audio")),
                                    start as f64 / 1_000_000.0,
                                    duration as f64 / 1_000_000.0,
                                );
                            }
                        }
                    }
                    _ => {}
                }
                return true;
            }
        }
        // 桌面版使用系统 FFmpeg 的标准时间戳输出
        #[cfg(feature = "desktop")]
        if let Some((_, packet)) = line.split_once("demuxer+ffmpeg -> ") {
            let field = |key| {
                packet
                    .split_whitespace()
                    .find_map(|part| part.strip_prefix(key))
            };
            let stream = match field("type:") {
                Some("video") => Some(0),
                Some("audio") => Some(1),
                _ => None,
            };
            let number = |key| field(key).and_then(|value| value.parse::<f64>().ok());
            if let (Some(stream), Some(start), Some(duration)) = (
                stream,
                number("pkt_pts_time:").or_else(|| number("pkt_dts_time:")),
                number("duration_time:"),
            ) {
                self.0.lock().unwrap().packet(stream, start, duration);
            }
            return true;
        }
        if ["[error]", "[fatal]", "[panic]"]
            .iter()
            .any(|level| line.contains(level))
        {
            self.0.lock().unwrap().snapshot.errors += 1;
        }
        // 时间戳调试附带的信息级日志在内存中消费
        cfg!(feature = "desktop") && line.contains("[info]")
    }
}

fn frame_rate(value: &str) -> Option<f64> {
    let (numerator, denominator) = value.split_once('/')?;
    let fps = numerator.parse::<f64>().ok()? / denominator.parse::<f64>().ok()?;
    (fps.is_finite() && fps > 0.0).then_some(fps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_is_the_common_remaining_media_range() {
        let stats = DecoderStats::default();
        assert_eq!(stats.buffer_seconds(0.0), 0.0);
        stats.observe("cardbili_buffer_video=0,8000000");
        assert_eq!(stats.buffer_seconds(0.0), 0.0);
        stats.observe("cardbili_buffer_audio=0,6000000");
        assert_eq!(stats.buffer_seconds(0.0), 6.0);
        assert_eq!(stats.buffer_seconds(2.0), 4.0);
        assert_eq!(stats.buffer_seconds(2.0), 4.0);
        assert_eq!(stats.buffer_seconds(6.0), 0.0);
    }

    #[test]
    fn reordered_video_packets_fill_gaps_without_counting_future_islands() {
        let stats = DecoderStats::default();
        stats.observe("cardbili_buffer_audio=0,8000000");
        stats.observe("cardbili_buffer_video=0,1000000");
        stats.observe("cardbili_buffer_video=3000000,1000000");
        assert_eq!(stats.buffer_seconds(0.0), 1.0);
        stats.observe("cardbili_buffer_video=2000000,1000000");
        assert_eq!(stats.buffer_seconds(0.0), 1.0);
        stats.observe("cardbili_buffer_video=1000000,1000000");
        assert_eq!(stats.buffer_seconds(0.0), 4.0);
        stats.observe("cardbili_buffer_video=5000000,1000000");
        assert_eq!(stats.buffer_seconds(4.5), 0.0);
        assert_eq!(stats.buffer_seconds(5.0), 1.0);
    }

    #[test]
    fn seek_discards_preroll_and_old_decoder_data() {
        let old = DecoderStats::default();
        old.observe("cardbili_buffer_video=0,30000000");
        old.observe("cardbili_buffer_audio=0,30000000");
        let current = DecoderStats::default();
        assert_eq!(current.buffer_seconds(0.0), 0.0);
        current.observe("cardbili_buffer_video=-2000000,1000000");
        current.observe("cardbili_buffer_audio=-1000000,1000000");
        assert_eq!(current.buffer_seconds(0.0), 0.0);
        current.observe("cardbili_buffer_video=0,3000000");
        current.observe("cardbili_buffer_audio=0,2000000");
        old.observe("cardbili_buffer_audio=30000000,1000000");
        assert_eq!(current.buffer_seconds(0.0), 2.0);
    }

    #[test]
    fn flv_timestamp_rounding_and_invalid_packets() {
        let stats = DecoderStats::default();
        for packet in [
            "cardbili_buffer_video=0,33000",
            "cardbili_buffer_video=34000,33000",
            "cardbili_buffer_audio=0,23000",
            "cardbili_buffer_audio=23000,23000",
            "cardbili_buffer_audio=46000,23000",
            "cardbili_buffer_video=invalid,10000000",
            "cardbili_buffer_video=67000,-1000",
        ] {
            assert!(stats.observe(packet));
        }
        assert!((stats.buffer_seconds(0.0) - 0.067).abs() < 1e-9);
        assert_eq!(stats.snapshot().errors, 0);
    }

    #[test]
    fn initial_track_offsets_follow_the_raw_output_clock() {
        let stats = DecoderStats::default();
        stats.observe("cardbili_buffer_video=23000,3000000");
        stats.observe("cardbili_buffer_audio=0,3100000");
        assert_eq!(stats.buffer_seconds(0.0), 3.0);
        assert!((stats.buffer_seconds(1.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn audio_without_packet_duration_uses_consecutive_timestamps() {
        let stats = DecoderStats::default();
        stats.observe("cardbili_buffer_video=0,1000000");
        stats.observe("cardbili_buffer_audio=0,0");
        assert_eq!(stats.buffer_seconds(0.0), 0.0);
        stats.observe("cardbili_buffer_audio=21000,0");
        assert_eq!(stats.buffer_seconds(0.0), 0.021);
        stats.observe("cardbili_buffer_audio=42000,0");
        stats.observe("cardbili_buffer_audio=64000,21000");
        assert!((stats.buffer_seconds(0.03) - 0.055).abs() < 1e-9);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_uses_normalized_demux_timestamps() {
        let stats = DecoderStats::default();
        for line in [
            "[info] demuxer -> ist_index:0:0 type:video pkt_pts_time:5000 duration_time:10",
            "[info] demuxer+ffmpeg -> ist_index:0:0 type:video pkt_pts_time:0 pkt_dts_time:-0.1 duration_time:0.04 off_time:-5000",
            "[info] demuxer+ffmpeg -> ist_index:1:0 type:audio pkt_pts_time:NOPTS pkt_dts_time:0 duration_time:0.03 off_time:-5000",
        ] {
            assert!(stats.observe(line));
        }
        assert_eq!(stats.buffer_seconds(0.0), 0.03);
        assert!(!stats.observe("[error] Error decoding frame"));
        assert_eq!(stats.snapshot().errors, 1);
    }

    #[test]
    fn progress_is_separated_from_errors_and_preserves_missing_counters() {
        let stats = std::sync::Arc::new(DecoderStats::default());
        for line in [
            "frame=30",
            "fps=29.97",
            "stream_0_0_q=-0.0",
            "speed=N/A",
            "progress=continue",
        ] {
            assert!(stats.observe(line));
        }
        assert!(stats.snapshot().received_bytes.is_none());
        assert_eq!(stats.snapshot().errors, 0);
        assert!(stats.observe("cardbili_rx_bytes=16384"));
        assert!(stats.observe("cardbili_rx_bytes=invalid"));
        assert!(stats.observe("cardbili_source_fps=30000/1001"));
        assert!(stats.observe("cardbili_source_fps=30/0"));
        assert!(stats.observe("cardbili_source_fps=0/1"));
        assert!(!stats.observe("Enter command: <target>|all <time>|-1 <command>"));
        assert!(!stats.observe("[https] [error] HTTP error 403 Forbidden"));
        assert!(!stats.observe(""));
        assert_eq!(stats.snapshot().received_bytes, Some(16384));
        assert_eq!(stats.snapshot().errors, 1);
        let audio = stats.clone();
        assert!(!audio.observe("[aac] [error] Error decoding AAC frame"));
        assert_eq!(stats.snapshot().errors, 2);
        assert_eq!(stats.snapshot().received_bytes, Some(16384));
        assert!((stats.snapshot().source_fps.unwrap() - 29.97003).abs() < 0.00001);
    }
}
