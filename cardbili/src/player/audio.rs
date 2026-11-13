use super::{diagnostics, frames::Frames};
use std::io::{BufRead, BufReader};
use std::os::fd::OwnedFd;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;

pub(super) fn start(frames: Arc<Frames>) -> std::io::Result<(Child, OwnedFd)> {
    let mut child = Command::new("pacat")
        .args([
            "--playback",
            "--raw",
            "--format=s16le",
            "--rate=44100",
            "--channels=2",
            "--latency-msec=60",
            "--process-time-msec=10",
            "--verbose",
            "--client-name=CardBili",
            "--stream-name=CardBili",
        ])
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let input = child.stdin.take().unwrap().into();
    let stderr = child.stderr.take().unwrap();
    let pid = child.id();
    diagnostics::write(format_args!(
        "[音频 pid={pid}] 使用实际播放时间同步，每 50ms 更新"
    ));
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = Vec::new();
        while reader.read_until(b'\r', &mut line).is_ok_and(|len| len > 0) {
            let text = String::from_utf8_lossy(&line);
            if let Some(time) = text
                .rsplit_once("Time: ")
                .and_then(|(_, value)| value.split_whitespace().next())
                .and_then(|value| value.parse::<f64>().ok())
            {
                frames.update_audio_clock(time);
            } else if !text.trim().is_empty() {
                diagnostics::write(format_args!("[音频 pid={pid}] {}", text.trim()));
            }
            line.clear();
        }
        frames.stop();
    });
    Ok((child, input))
}
