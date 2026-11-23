use std::sync::Mutex;

#[derive(Clone, Copy, Default)]
pub(crate) struct DecoderSnapshot {
    pub received_bytes: Option<u64>,
    pub source_fps: Option<f64>,
    pub errors: u64,
}

#[derive(Default)]
pub(crate) struct DecoderStats(Mutex<DecoderSnapshot>);

impl DecoderStats {
    pub fn snapshot(&self) -> DecoderSnapshot {
        *self.0.lock().unwrap()
    }

    // FFmpeg 的进度字段与错误日志共用 stderr；返回 true 表示该行是进度字段，不写入日志
    pub fn observe(&self, line: &str) -> bool {
        if let Some((key, value)) = line.split_once('=') {
            if !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                let mut stats = self.0.lock().unwrap();
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
                    _ => {}
                }
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
