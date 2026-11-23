use std::sync::Mutex;

#[derive(Clone, Copy, Default)]
pub(crate) struct DecoderSnapshot {
    pub received_bytes: Option<u64>,
    pub source_fps: Option<f64>,
    pub source_bitrate: Option<f64>,
    pub errors: u64,
}

#[derive(Default)]
pub(crate) struct DecoderStats(Mutex<DecoderSnapshot>);

impl DecoderStats {
    pub fn snapshot(&self) -> DecoderSnapshot {
        *self.0.lock().unwrap()
    }

    // FFmpeg 的进度字段与错误日志共用 stderr
    pub fn observe(&self, line: &str) -> bool {
        if let Some((key, value)) = line.split_once('=') {
            if key == "cardbili_source_bitrate" {
                self.0.lock().unwrap().source_bitrate = value
                    .parse::<f64>()
                    .ok()
                    .filter(|bitrate| bitrate.is_finite() && *bitrate > 0.0);
                return true;
            }
            if key == "cardbili_source_fps" {
                if let Some(fps) = frame_rate(value) {
                    self.0.lock().unwrap().source_fps = Some(fps);
                }
                return true;
            }
            if key == "cardbili_rx_bytes" {
                if let Ok(bytes) = value.parse() {
                    self.0.lock().unwrap().received_bytes = Some(bytes);
                }
                return true;
            }
            if matches!(
                key,
                "frame"
                    | "fps"
                    | "bitrate"
                    | "total_size"
                    | "out_time_us"
                    | "out_time_ms"
                    | "out_time"
                    | "dup_frames"
                    | "drop_frames"
                    | "speed"
                    | "progress"
            ) || (key.starts_with("stream_") && key.ends_with("_q"))
            {
                return true;
            }
        }
        if ["[error]", "[fatal]", "[panic]"]
            .iter()
            .any(|level| line.contains(level))
        {
            self.0.lock().unwrap().errors += 1;
        }
        false
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
    fn progress_is_separated_from_errors_and_preserves_missing_counters() {
        let stats = DecoderStats::default();
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
        assert!(stats.snapshot().source_bitrate.is_none());
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
        assert!((stats.snapshot().source_fps.unwrap() - 29.97003).abs() < 0.00001);
        assert!(stats.observe("bitrate=39168.0kbits/s"));
        assert!(stats.snapshot().source_bitrate.is_none());
        assert!(stats.observe("cardbili_source_bitrate=1500000"));
        assert_eq!(stats.snapshot().source_bitrate, Some(1_500_000.0));
        for invalid in ["0", "-1", "NaN", "inf", "N/A"] {
            assert!(stats.observe(&format!("cardbili_source_bitrate={invalid}")));
            assert!(stats.snapshot().source_bitrate.is_none());
        }
    }
}
