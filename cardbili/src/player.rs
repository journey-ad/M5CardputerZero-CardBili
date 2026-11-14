#[cfg(target_os = "linux")]
mod audio;
mod frames;
pub mod volume;

use crate::diagnostics;
use crate::model::PlayInfo;
use crate::net::{REFERER, UA};
use crate::Msg;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc::Sender, Arc};
use std::thread;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 170;
pub const FPS: f64 = 30.0;
pub const VID_BYTES: usize = (WIDTH as usize) * (HEIGHT as usize) * 3;

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
            diagnostics::write(format_args!("[{kind} pid={pid}] 进程已退出: {status}"));
            return;
        }
        let _ = ch.kill();
        match ch.wait() {
            Ok(status) => {
                diagnostics::write(format_args!("[{kind} pid={pid}] 进程已停止: {status}"));
            }
            Err(error) => {
                diagnostics::write(format_args!("[{kind} pid={pid}] 等待进程退出失败: {error}"));
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

struct Speed {
    output: f64,
    position: f64,
    rate: f64,
}

pub struct Player {
    pub gen: u64,
    pub title: String,
    pub duration: f64,
    base: f64,
    speeds: Vec<Speed>,
    pub paused: bool,
    pub rate: f64,
    pub finished: bool,
    ready: bool,
    video_url: String,
    audio_url: String,
    cookies: String,
    child: Option<Child>,
    audio: Option<Child>,
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
            speeds: Vec::new(),
            paused: state.paused,
            rate: state.rate,
            ready: false,
            finished: false,
            video_url: info.video_url.clone(),
            audio_url: info.audio_url.clone(),
            cookies,
            child: None,
            audio: None,
            tx,
            frames: Arc::new(frames::Frames::default()),
        };
        p.base = state.position.clamp(0.0, p.duration);
        p.spawn(p.base);
        p
    }

    fn spawn(&mut self, ss: f64) {
        self.speeds = vec![Speed {
            output: 0.0,
            position: ss,
            rate: self.rate,
        }];
        stop_child(&mut self.child, "音视频");
        stop_child(&mut self.audio, "音频");
        self.frames.stop();
        self.frames = Arc::new(frames::Frames::default());
        self.frames.pause(self.paused);
        #[cfg(target_os = "linux")]
        let audio_pipe = match audio::start(self.frames.clone()) {
            Ok((child, pipe)) => {
                self.audio = Some(child);
                Some(pipe)
            }
            Err(error) => {
                diagnostics::write(format_args!("音频输出启动失败: {error}"));
                return;
            }
        };
        #[cfg(not(target_os = "linux"))]
        let audio_pipe = None;
        self.ready = false;
        self.finished = false;
        diagnostics::write(format_args!(
            "开始解码 gen={} 跳转={ss:.3}s 倍速={} 日志={}",
            self.gen,
            self.rate,
            diagnostics::path().display()
        ));
        let hd = headers(&self.cookies);
        let vf = format!(
            "setpts@speed='if(isnan(PREV_OUTPTS),0,PREV_OUTPTS+(PTS-PREV_INPTS)/{})',fps=30,scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=fast_bilinear,pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black",
            self.rate, WIDTH, HEIGHT, WIDTH, HEIGHT
        );
        let mut args: Vec<String> = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-nostats".into(),
            "-filter_threads".into(),
            "2".into(),
        ];
        for url in [&self.video_url, &self.audio_url] {
            args.extend([
                "-threads".into(),
                "4".into(),
                "-probesize".into(),
                "65536".into(),
                "-analyzeduration".into(),
                "100000".into(),
                "-protocol_whitelist".into(),
                "file,pipe,crypto,http,https,tcp,tls".into(),
                "-user_agent".into(),
                UA.into(),
                "-referer".into(),
                REFERER.into(),
                "-headers".into(),
                hd.clone(),
            ]);
            if ss > 0.0 {
                args.extend(["-ss".into(), format!("{ss:.3}")]);
            }
            args.extend(["-i".into(), url.clone()]);
        }
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
            "1:a:0".into(),
            "-ac".into(),
            "2".into(),
            "-ar".into(),
            "44100".into(),
        ]);
        args.extend(["-af".into(), format!("atempo@speed={}", self.rate)]);
        if cfg!(target_os = "linux") {
            args.extend(["-f".into(), "s16le".into(), "pipe:3".into()]);
        } else {
            args.extend(["-f".into(), "audiotoolbox".into()]);
            args.push("default".into());
        }

        if let Some(mut ch) =
            diagnostics::spawn("音视频", &args, &self.cookies, Stdio::piped(), audio_pipe)
        {
            let pid = ch.id();
            if let Some(mut so) = ch.stdout.take() {
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
                        if !pending.publish(frames, buf.clone()) {
                            break;
                        }
                    }
                    pending.finish_video();
                });
                let tx = self.tx.clone();
                let pending = self.frames.clone();
                thread::spawn(move || {
                    while let Some(notify) = pending.present() {
                        if notify && tx.send(Msg::Frame(gen)).is_err() {
                            pending.stop();
                            return;
                        }
                    }
                    let _ = tx.send(Msg::PlaybackEnded(gen));
                });
            }
            self.child = Some(ch);
        }
    }

    pub fn position(&self, frames_in_seg: u64) -> f64 {
        let output = self.frames.position(frames_in_seg);
        let speed = self
            .speeds
            .iter()
            .rfind(|speed| speed.output <= output)
            .unwrap();
        (speed.position + (output - speed.output) * speed.rate).min(self.duration)
    }

    pub fn cycle_rate(&mut self) -> Result<(), String> {
        let rate = match self.rate {
            0.5 => 1.0,
            1.0 => 1.5,
            1.5 => 2.0,
            _ => 0.5,
        };
        // 音视频在相同媒体时间切换，已进入输出缓冲区的内容按原速播放
        let last = self.speeds.last().unwrap();
        let output = (self.frames.position(0) + 2.0).max(last.output + 1.2);
        let position = last.position + (output - last.output) * last.rate;
        let input_time = position - self.base;
        let command = format!(
            "Catempo@speed {input_time:.6} tempo {rate}\nCsetpts@speed {input_time:.6} expr (PTS-{input_time:.6}/TB)/{rate}+{output:.6}/TB\n"
        );
        let input = self
            .child
            .as_mut()
            .and_then(|child| child.stdin.as_mut())
            .ok_or("播放器尚未就绪")?;
        input
            .write_all(command.as_bytes())
            .map_err(|error| format!("倍速切换失败\n{error}"))?;
        self.speeds.push(Speed {
            output,
            position,
            rate,
        });
        self.rate = rate;
        diagnostics::write(format_args!(
            "运行中切换倍速 rate={rate} position={position:.3}"
        ));
        Ok(())
    }

    pub fn pause_toggle(&mut self) {
        self.paused = !self.paused;
        diagnostics::write(format_args!("[播放] gen={} 暂停={}", self.gen, self.paused));
        self.frames.pause(self.paused);
        let signal = if self.paused { "-STOP" } else { "-CONT" };
        if self.ready {
            Self::signal_child(&self.child, signal);
            Self::signal_child(&self.audio, signal);
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
            Self::signal_child(&self.audio, "-STOP");
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
        diagnostics::write(format_args!("[播放] gen={} 跳转到={target:.3}s", self.gen));
        self.base = target.clamp(0.0, self.duration);
        self.gen += 1;
        self.spawn(self.base);
        self.base
    }

    pub fn stop(&mut self) {
        diagnostics::write(format_args!("[播放] 停止 gen={}", self.gen));
        self.frames.stop();
        stop_child(&mut self.child, "音视频");
        stop_child(&mut self.audio, "音频");
    }
}
