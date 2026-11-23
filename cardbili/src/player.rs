#[cfg(target_os = "linux")]
mod audio;
#[cfg(all(feature = "desktop", windows))]
mod audio_desktop;
mod frames;
pub(crate) mod telemetry;
pub mod volume;

use crate::diagnostics;
use crate::model::PlayInfo;
use crate::net::{REFERER, UA};
use crate::settings::Settings;
use crate::Msg;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc::Sender, Arc};
use std::thread;
use std::time::{Duration, Instant};

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 170;
pub const FPS: f64 = 30.0;
pub type VideoFrame = slint::SharedPixelBuffer<slint::Rgb8Pixel>;

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

pub(crate) struct Statistics {
    pub frames: frames::Snapshot,
    pub decoder: telemetry::DecoderSnapshot,
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
    last_frame: Instant,
    video_url: String,
    audio_url: String,
    cookies: String,
    child: Option<Child>,
    audio: Option<Child>,
    live: bool,
    scale_filter: String,
    tx: Sender<Msg>,
    frames: Arc<frames::Frames>,
    decoder_stats: Arc<telemetry::DecoderStats>,
    #[cfg(all(feature = "desktop", windows))]
    audio_stats: Arc<telemetry::DecoderStats>,
}

impl Player {
    pub fn start(
        info: &PlayInfo,
        cookies: String,
        gen: u64,
        tx: Sender<Msg>,
        settings: &Settings,
        state: PlaybackState,
    ) -> Player {
        let scale_filter = if settings.pixel_optimize {
            format!(
                "scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=area,scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=bicubic",
                WIDTH * 2,
                HEIGHT * 2,
                WIDTH,
                HEIGHT
            )
        } else {
            format!(
                "scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2:flags={}",
                WIDTH,
                HEIGHT,
                settings.scale_algorithm.ffmpeg_flag()
            )
        };
        let mut p = Player {
            gen,
            title: info.title.clone(),
            duration: info.duration,
            base: 0.0,
            speeds: Vec::new(),
            paused: state.paused,
            rate: state.rate,
            ready: false,
            last_frame: Instant::now(),
            finished: false,
            video_url: info.video_url.clone(),
            audio_url: info.audio_url.clone(),
            cookies,
            child: None,
            audio: None,
            live: info.live,
            scale_filter,
            tx,
            frames: Arc::new(frames::Frames::new(info.live)),
            decoder_stats: Arc::new(telemetry::DecoderStats::default()),
            #[cfg(all(feature = "desktop", windows))]
            audio_stats: Arc::new(telemetry::DecoderStats::default()),
        };
        p.base = state.position.clamp(0.0, p.duration);
        p.spawn(p.base);
        p
    }

    #[cfg(all(feature = "desktop", windows))]
    fn start_desktop_audio(&mut self, ss: f64) {
        let headers = headers(&self.cookies);
        let mut args: Vec<String> = [
            "-hide_banner",
            "-loglevel",
            "level+error",
            "-nostats",
            "-progress",
            "pipe:2",
            "-stats_period",
            "1",
            "-threads",
            "4",
            "-probesize",
            "65536",
            "-analyzeduration",
            "100000",
            "-protocol_whitelist",
            "file,pipe,crypto,http,https,tcp,tls,httpproxy",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect();
        args.extend([
            "-user_agent".to_string(),
            UA.to_string(),
            "-referer".to_string(),
            REFERER.to_string(),
            "-headers".to_string(),
            headers,
        ]);
        if ss > 0.0 {
            args.extend(["-ss".to_string(), format!("{ss:.3}")]);
        }
        args.extend(["-rw_timeout".into(), "15000000".into()]);
        let url = if self.live {
            &self.video_url
        } else {
            &self.audio_url
        };
        args.extend(["-i".to_string(), url.clone()]);
        args.extend([
            "-vn".to_string(),
            "-ac".to_string(),
            "2".to_string(),
            "-ar".to_string(),
            "44100".to_string(),
            "-af".to_string(),
            format!("atempo@speed={}", self.rate),
            "-f".to_string(),
            "s16le".to_string(),
            "pipe:1".to_string(),
        ]);
        if let Some(mut child) = diagnostics::spawn(
            crate::i18n::tr("音频"),
            &args,
            &self.cookies,
            Stdio::piped(),
            None,
            self.audio_stats.clone(),
        ) {
            if let Some(stdout) = child.stdout.take() {
                audio_desktop::start(stdout, self.frames.clone());
            }
            self.audio = Some(child);
        }
    }

    fn spawn(&mut self, ss: f64) {
        self.ready = false;
        self.last_frame = Instant::now();
        self.finished = false;
        self.speeds = vec![Speed {
            output: 0.0,
            position: ss,
            rate: self.rate,
        }];
        stop_child(&mut self.child, crate::i18n::tr("音视频"));
        stop_child(&mut self.audio, crate::i18n::tr("音频"));
        self.frames.stop();
        self.frames = Arc::new(frames::Frames::new(self.live));
        self.decoder_stats = Arc::new(telemetry::DecoderStats::default());
        #[cfg(all(feature = "desktop", windows))]
        {
            self.audio_stats = Arc::new(telemetry::DecoderStats::default());
        }
        self.frames.pause(self.paused);
        #[cfg(target_os = "linux")]
        let audio_pipe = match audio::start(self.frames.clone()) {
            Ok((child, pipe)) => {
                self.audio = Some(child);
                Some(pipe)
            }
            Err(error) => {
                diagnostics::write(format_args!("音频输出启动失败: {error}"));
                let _ = self.tx.send(Msg::PlaybackFailed(
                    self.gen,
                    crate::i18n::tr("音频输出启动失败，请检查音频服务").into(),
                ));
                return;
            }
        };
        #[cfg(not(target_os = "linux"))]
        let audio_pipe = None;
        #[cfg(all(feature = "desktop", windows))]
        self.start_desktop_audio(ss);
        diagnostics::write(format_args!(
            "开始解码 gen={} 跳转={ss:.3}s 倍速={} 日志={}",
            self.gen,
            self.rate,
            diagnostics::path().display()
        ));
        let hd = headers(&self.cookies);
        let vf = format!(
            "setpts@speed='if(isnan(PREV_OUTPTS),0,PREV_OUTPTS+(PTS-PREV_INPTS)/{})',fps=30,{},pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black",
            self.rate, self.scale_filter, WIDTH, HEIGHT
        );
        let mut args: Vec<String> = vec![
            "-hide_banner".into(),
            "-loglevel".into(),
            "level+error".into(),
            "-nostats".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-stats_period".into(),
            "1".into(),
            "-filter_threads".into(),
            "2".into(),
        ];
        let inputs: Vec<&String> = if self.live || cfg!(all(feature = "desktop", windows)) {
            vec![&self.video_url]
        } else {
            vec![&self.video_url, &self.audio_url]
        };
        for url in inputs {
            if self.live {
                // 输入线程缓存压缩包，解码帧队列维持 500ms
                args.extend(["-thread_queue_size".into(), "128".into()]);
                // 直播源为 720p，缩到 320×170 后去块滤波的差异低于 RGB565 量化噪声，跳过可省约 15% 解码 CPU
                args.extend(["-skip_loop_filter".into(), "all".into()]);
            }
            args.extend([
                "-threads".into(),
                "4".into(),
                "-rw_timeout".into(),
                "15000000".into(),
                "-probesize".into(),
                "65536".into(),
                "-analyzeduration".into(),
                "100000".into(),
                "-protocol_whitelist".into(),
                "file,pipe,crypto,http,https,tcp,tls,httpproxy".into(),
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

        if cfg!(all(feature = "desktop", windows)) {
            // Windows 桌面版的音频由独立进程解码后交给 cpal 播放
        } else {
            args.extend([
                "-map".into(),
                if self.live {
                    "0:a:0".into()
                } else {
                    "1:a:0".into()
                },
                "-ac".into(),
                "2".into(),
                "-ar".into(),
                "44100".into(),
            ]);
            args.extend(["-af".into(), format!("atempo@speed={}", self.rate)]);
            if cfg!(target_os = "linux") {
                args.extend(["-f".into(), "s16le".into(), "pipe:3".into()]);
            } else if cfg!(target_os = "macos") {
                args.extend(["-f".into(), "audiotoolbox".into()]);
                args.push("default".into());
            } else {
                args.extend(["-f".into(), "sdl2".into(), "cardbili".into()]);
            }
        }

        if let Some(mut ch) = diagnostics::spawn(
            crate::i18n::tr("音视频"),
            &args,
            &self.cookies,
            Stdio::piped(),
            audio_pipe,
            self.decoder_stats.clone(),
        ) {
            let pid = ch.id();
            if let Some(mut so) = ch.stdout.take() {
                let gen = self.gen;
                let pending = self.frames.clone();
                thread::spawn(move || {
                    let mut frames = 0u64;
                    loop {
                        // 解码输出直接写入像素缓冲，队列与界面依次接收所有权
                        let mut pixels = VideoFrame::new(WIDTH, HEIGHT);
                        if let Err(error) = so.read_exact(pixels.make_mut_bytes()) {
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
                        if !pending.publish(frames, pixels) {
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
        } else {
            stop_child(&mut self.audio, crate::i18n::tr("音频"));
            let _ = self.tx.send(Msg::PlaybackFailed(
                self.gen,
                crate::i18n::tr("解码器启动失败，请检查 FFmpeg").into(),
            ));
        }
    }

    pub fn position(&self, frames_in_seg: u64) -> f64 {
        let output = self.frames.position(frames_in_seg);
        let speed = self
            .speeds
            .iter()
            .rfind(|speed| speed.output <= output)
            .unwrap();
        let position = speed.position + (output - speed.output) * speed.rate;
        if self.live {
            position
        } else {
            position.min(self.duration)
        }
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
            .ok_or(crate::i18n::tr("播放器尚未就绪"))?;
        input
            .write_all(command.as_bytes())
            .map_err(|error| crate::i18n::trf("倍速切换失败\n{}", &[&error]))?;
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
        self.last_frame = Instant::now();
        let signal = if self.paused { "-STOP" } else { "-CONT" };
        if self.ready {
            Self::signal_child(&self.child, signal);
            Self::signal_child(&self.audio, signal);
        }
    }

    pub fn frame_ready(&mut self) {
        self.last_frame = Instant::now();
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

    pub fn stalled(&self) -> bool {
        let timeout = if self.live && self.ready { 8 } else { 30 };
        !self.finished
            && !(self.paused && self.ready)
            && self.last_frame.elapsed() > Duration::from_secs(timeout)
    }

    pub fn take_frame(&self) -> Option<(u64, VideoFrame)> {
        self.frames.take()
    }

    pub fn discard_frame(&self) {
        self.frames.discard();
    }

    pub(crate) fn statistics(&self) -> Statistics {
        let decoder = self.decoder_stats.snapshot();
        #[cfg(all(feature = "desktop", windows))]
        let decoder = {
            let audio = self.audio_stats.snapshot();
            telemetry::DecoderSnapshot {
                received_bytes: decoder
                    .received_bytes
                    .zip(audio.received_bytes)
                    .map(|(video, audio)| video + audio),
                errors: decoder.errors + audio.errors,
                source_bitrate: decoder
                    .source_bitrate
                    .zip(audio.source_bitrate)
                    .map(|(video, audio)| video + audio),
                ..decoder
            }
        };
        Statistics {
            frames: self.frames.snapshot(),
            decoder,
        }
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
        stop_child(&mut self.child, crate::i18n::tr("音视频"));
        stop_child(&mut self.audio, crate::i18n::tr("音频"));
    }
}
