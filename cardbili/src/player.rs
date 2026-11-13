use crate::model::{PlayInfo, Seg};
use crate::net::{REFERER, UA};
use crate::Msg;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
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

fn stop_child(c: &mut Option<Child>) {
    if let Some(mut ch) = c.take() {
        let _ = ch.kill();
        let _ = ch.wait();
    }
}

fn pactl_vol(audio_pid: u32, pct: i32) {
    let out = Command::new("pactl")
        .args(["--format=json", "list", "sink-inputs"])
        .output();
    if let Ok(o) = out {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
            if let Some(arr) = v.as_array() {
                let pid = audio_pid.to_string();
                for s in arr {
                    if s["properties"]["application.process.id"].as_str() == Some(pid.as_str()) {
                        if let Some(idx) = s["index"].as_i64() {
                            let _ = Command::new("pactl")
                                .args([
                                    "set-sink-input-volume",
                                    &idx.to_string(),
                                    &format!("{}%", pct),
                                ])
                                .output();
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct PlaybackState {
    pub position: f64,
    pub paused: bool,
    pub volume: i32,
    pub rate: f64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            position: 0.0,
            paused: false,
            volume: 100,
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
    pub vol: i32,
    pub rate: f64,
    ready: bool,
    segs: Vec<Seg>,
    seg_idx: usize,
    cookies: String,
    vchild: Option<Child>,
    achild: Option<Child>,
    tx: Sender<Msg>,
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
            vol: state.volume.clamp(0, 150),
            rate: state.rate,
            ready: false,
            segs: info.segs.clone(),
            seg_idx: 0,
            cookies,
            vchild: None,
            achild: None,
            tx,
        };
        let offset = p.set_position(state.position);
        p.spawn(offset);
        p
    }

    fn spawn(&mut self, ss: f64) {
        stop_child(&mut self.vchild);
        stop_child(&mut self.achild);
        self.ready = false;
        let seg = self.segs.get(self.seg_idx).cloned().unwrap_or_default();
        let hd = headers(&self.cookies);
        let vf = format!(
            "setpts=(PTS-STARTPTS)/{},fps=30,scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=fast_bilinear,pad={}:{}:(ow-iw)/2:(oh-ih)/2:color=black",
            self.rate, WIDTH, HEIGHT, WIDTH, HEIGHT
        );
        let mut vargs: Vec<String> = vec![
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
            hd.clone(),
        ];
        if ss > 0.0 {
            vargs.push("-ss".into());
            vargs.push(format!("{:.3}", ss));
        }
        vargs.push("-readrate".into());
        vargs.push(self.rate.to_string());
        vargs.push("-i".into());
        vargs.push(seg.url.clone());
        vargs.push("-map".into());
        vargs.push("0:v:0".into());
        vargs.push("-vf".into());
        vargs.push(vf);
        vargs.push("-pix_fmt".into());
        vargs.push("rgb24".into());
        vargs.push("-threads:v".into());
        vargs.push("2".into());
        vargs.push("-f".into());
        vargs.push("rawvideo".into());
        vargs.push("pipe:1".into());

        let mut vcmd = Command::new("ffmpeg");
        vcmd.args(&vargs)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null());
        if let Ok(mut ch) = vcmd.spawn() {
            if let Some(mut so) = ch.stdout.take() {
                let tx = self.tx.clone();
                let gen = self.gen;
                thread::spawn(move || {
                    let mut buf = vec![0u8; VID_BYTES];
                    while so.read_exact(&mut buf).is_ok() {
                        if tx.send(Msg::Frame(gen, buf.clone())).is_err() {
                            break;
                        }
                    }
                    let _ = tx.send(Msg::SegEnd(gen));
                });
            }
            self.vchild = Some(ch);
        }

        // 音频使用独立的解码进程
        let aargs: Vec<String> = {
            let mut a: Vec<String> = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-nostdin".into(),
                "-protocol_whitelist".into(),
                "file,pipe,crypto,http,https,tcp,tls".into(),
                "-user_agent".into(),
                UA.into(),
                "-referer".into(),
                REFERER.into(),
                "-headers".into(),
                hd.clone(),
            ];
            if ss > 0.0 {
                a.push("-ss".into());
                a.push(format!("{:.3}", ss));
            }
            a.push("-readrate".into());
            a.push(self.rate.to_string());
            a.push("-i".into());
            a.push(seg.url.clone());
            a.push("-map".into());
            a.push("0:a:0".into());
            a.push("-af".into());
            a.push(format!("atempo={}", self.rate));
            a.push("-ac".into());
            a.push("2".into());
            a.push("-ar".into());
            a.push("44100".into());
            a.push("-f".into());
            a.push("pulse".into());
            a.push("default".into());
            a
        };
        if let Ok(ch) = Command::new("ffmpeg")
            .args(&aargs)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
        {
            self.achild = Some(ch);
        }
        if let Some(a) = &self.achild {
            let pid = a.id();
            let vol = self.vol;
            thread::spawn(move || {
                thread::sleep(std::time::Duration::from_millis(400));
                pactl_vol(pid, vol);
            });
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
        if self.ready {
            let signal = if self.paused { "-STOP" } else { "-CONT" };
            self.signal_children(signal);
        }
    }

    pub fn frame_ready(&mut self) {
        if self.ready {
            return;
        }
        self.ready = true;
        // 加载期间的暂停在首帧就绪后执行
        if self.paused {
            self.signal_children("-STOP");
        }
    }

    fn signal_children(&self, signal: &str) {
        for child in [&self.vchild, &self.achild].into_iter().flatten() {
            let _ = Command::new("kill")
                .args([signal, &child.id().to_string()])
                .output();
        }
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
            false
        }
    }

    pub fn vol_up(&mut self) {
        self.set_vol(self.vol + 10);
    }
    pub fn vol_down(&mut self) {
        self.set_vol(self.vol - 10);
    }

    fn set_vol(&mut self, v: i32) {
        self.vol = v.clamp(0, 150);
        if let Some(a) = &self.achild {
            pactl_vol(a.id(), self.vol);
        }
    }

    pub fn stop(&mut self) {
        stop_child(&mut self.vchild);
        stop_child(&mut self.achild);
    }
}
