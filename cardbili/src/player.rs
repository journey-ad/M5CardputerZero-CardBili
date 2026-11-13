mod diagnostics;
mod frames;
pub mod volume;

use crate::model::{PlayInfo, Seg};
use crate::net::{REFERER, UA};
use crate::Msg;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc::Sender, Arc};
use std::thread;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 170;
pub const FPS: f64 = 30.0;
pub const VID_BYTES: usize = (WIDTH as usize) * (HEIGHT as usize) * 3;

fn cum_len(segs: &[Seg], upto: usize) -> f64 {
    segs.iter().take(upto).map(|s| s.len).sum()
}

fn headers(cookies: &str) -> String {
    let mut h = format!("Referer: {}\r\nOrigin: {}\r\n", REFERER, REFERER);
    if !cookies.is_empty() {
        h.push_str(&format!("Cookie: {}\r\n", cookies));
    }
    h
}

fn stop_child(c: &mut Option<Child>, kind: &str) {
    if let Some(mut ch) = c.take() {
        let pid = ch.id();
        if let Ok(Some(status)) = ch.try_wait() {
            diagnostics::write(format_args!("[{kind} pid={pid}] ffmpeg 已退出: {status}"));
            return;
        }
        let _ = ch.kill();
        match ch.wait() {
            Ok(status) => {
                diagnostics::write(format_args!("[{kind} pid={pid}] ffmpeg 已停止: {status}"));
            }
            Err(error) => {
                diagnostics::write(format_args!(
                    "[{kind} pid={pid}] 等待 ffmpeg 退出失败: {error}"
                ));
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct PlaybackState {
    pub position: f64,
    pub paused: bool,
    pub rate: f64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            position: 0.0,
            paused: false,
            rate: 1.0,
        }
    }
}

pub struct Player {
    pub gen: u64,
    pub title: String,
    pub duration: f64,
    pub base: f64,
    pub paused: bool,
    pub rate: f64,
    pub finished: bool,
    ready: bool,
    segs: Vec<Seg>,
    seg_idx: usize,
    cookies: String,
    child: Option<Child>,
    tx: Sender<Msg>,
    frames: Arc<frames::Frames>,
}

impl Player {
    pub fn start(
        info: &PlayInfo,
        cookies: String,
        gen: u64,
        tx: Sender<Msg>,
        state: PlaybackState,
    ) -> Player {
        let mut p = Player {
            gen,
            title: info.title.clone(),
            duration: info.duration,
            base: 0.0,
            paused: state.paused,
            rate: state.rate,
            ready: false,
            finished: false,
            segs: info.segs.clone(),
            seg_idx: 0,
            cookies,
            child: None,
            tx,
            frames: Arc::new(frames::Frames::default()),
        };
        let offset = p.set_position(state.position);
        p.spawn(offset);
        p
    }

    fn spawn(&mut self, ss: f64) {
        stop_child(&mut self.child, "音视频");
        self.frames.stop();
        self.frames = Arc::new(frames::Frames::default());
        self.frames.pause(self.paused);
        self.ready = false;
        self.finished = false;
        diagnostics::write(format_args!(
            "开始解码 gen={} 分段={}/{} 跳转={ss:.3}s 倍速={} 日志=/tmp/cardbili-player.log",
            self.gen,
            self.seg_idx + 1,
            self.segs.len(),
            self.rate
        ));
        let seg = self.segs.get(self.seg_idx).cloned().unwrap_or_default();
        let hd = headers(&self.cookies);
        let vf = format!(
            "setpts=(PTS-STARTPTS)/{},fps=30,scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=fast_bilinear,pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black",
            self.rate, WIDTH, HEIGHT, WIDTH, HEIGHT
        );
        let mut args: Vec<String> = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-nostdin".into(),
            "-threads".into(),
            "4".into(),
            "-filter_threads".into(),
            "2".into(),
            "-protocol_whitelist".into(),
            "file,pipe,crypto,http,https,tcp,tls".into(),
            "-user_agent".into(),
            UA.into(),
            "-referer".into(),
            REFERER.into(),
            "-headers".into(),
            hd,
        ];
        if ss > 0.0 {
            args.push("-ss".into());
            args.push(format!("{:.3}", ss));
        }
        args.push("-i".into());
        args.push(seg.url.clone());
        args.push("-map".into());
        args.push("0:v:0".into());
        args.push("-vf".into());
        args.push(vf);
        args.push("-pix_fmt".into());
        args.push("rgb24".into());
        args.push("-threads:v".into());
        args.push("2".into());
        args.push("-f".into());
        args.push("rawvideo".into());
        args.push("pipe:1".into());

        args.extend([
            "-map".into(),
            "0:a:0".into(),
            "-af".into(),
            format!("atempo={}", self.rate),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "44100".into(),
        ]);
        if cfg!(target_os = "macos") {
            args.extend(["-f".into(), "audiotoolbox".into()]);
        } else {
            args.extend([
                "-f".into(),
                "pulse".into(),
                "-buffer_duration".into(),
                "80".into(),
            ]);
        }
        args.push("default".into());

        if let Some(mut ch) = diagnostics::spawn("音视频", &args, &self.cookies, Stdio::piped())
        {
            let pid = ch.id();
            if let Some(mut so) = ch.stdout.take() {
                let tx = self.tx.clone();
                let gen = self.gen;
                let pending = self.frames.clone();
                thread::spawn(move || {
                    let mut buf = vec![0u8; VID_BYTES];
                    let mut frames = 0u64;
                    loop {
                        if let Err(error) = so.read_exact(&mut buf) {
                            diagnostics::write(format_args!(
                                "[视频 pid={pid} gen={gen}] 读取结束 已接收帧数={frames}: {error}"
                            ));
                            break;
                        }
                        frames += 1;
                        if frames == 1 {
                            diagnostics::write(format_args!(
                                "[视频 pid={pid} gen={gen}] 首帧已就绪"
                            ));
                        }
                        match pending.publish(frames, buf.clone()) {
                            Some(true) => {
                                if tx.send(Msg::Frame(gen)).is_err() {
                                    break;
                                }
                            }
                            Some(false) => {}
                            None => return,
                        }
                    }
                    let _ = tx.send(Msg::SegEnd(gen));
                });
            }
            self.child = Some(ch);
        }
    }

    pub fn position(&self, frames_in_seg: u64) -> f64 {
        (self.base + frames_in_seg as f64 / FPS * self.rate).min(self.duration)
    }

    pub fn cycle_rate(&mut self, position: f64) {
        self.rate = match self.rate {
            0.5 => 1.0,
            1.0 => 1.5,
            1.5 => 2.0,
            _ => 0.5,
        };
        self.seek(position);
    }

    pub fn pause_toggle(&mut self) {
        self.paused = !self.paused;
        self.frames.pause(self.paused);
        let signal = if self.paused { "-STOP" } else { "-CONT" };
        if self.ready {
            Self::signal_child(&self.child, signal);
        }
    }

    pub fn frame_ready(&mut self) {
        if self.ready {
            return;
        }
        self.ready = true;
        // 加载期间的暂停在首帧就绪后执行
        if self.paused {
            Self::signal_child(&self.child, "-STOP");
        }
    }

    fn signal_child(child: &Option<Child>, signal: &str) {
        if let Some(child) = child {
            let _ = Command::new("kill")
                .args([signal, &child.id().to_string()])
                .output();
        }
    }

    pub fn take_frame(&self) -> Option<(u64, Vec<u8>)> {
        self.frames.take()
    }

    pub fn seek(&mut self, target: f64) -> f64 {
        let offset = self.set_position(target);
        self.gen += 1;
        self.spawn(offset);
        self.base
    }

    fn set_position(&mut self, target: f64) -> f64 {
        let target = target.max(0.0).min(self.duration.max(0.0));
        let mut idx = 0usize;
        let mut acc = 0.0;
        for (i, s) in self.segs.iter().enumerate() {
            if target < acc + s.len || i + 1 == self.segs.len() {
                idx = i;
                break;
            }
            acc += s.len;
        }
        let ss = (target - acc).max(0.0);
        self.seg_idx = idx;
        self.base = acc + ss;
        ss
    }

    pub fn advance(&mut self) -> bool {
        if self.seg_idx + 1 < self.segs.len() {
            self.seg_idx += 1;
            self.base = cum_len(&self.segs, self.seg_idx);
            self.gen += 1;
            self.spawn(0.0);
            true
        } else {
            self.finished = true;
            false
        }
    }

    pub fn stop(&mut self) {
        self.frames.stop();
        stop_child(&mut self.child, "音视频");
    }
}
