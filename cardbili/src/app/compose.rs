use super::*;
use crate::api::posting::Target;
use crate::voice::{Event, Session};

#[derive(Default, PartialEq, Eq)]
pub(super) enum Phase {
    Recording,
    Finishing,
    #[default]
    Ready,
    Sending,
}

#[derive(Default)]
pub(super) struct Composer {
    pub target: Option<Target>,
    pub request: u64,
    pub phase: Phase,
    pub send_when_ready: bool,
    pub text: String,
    pub resume_playback: bool,
}

impl App {
    pub(super) fn open_danmaku_input(&mut self) {
        let target = if let Some(live) = &self.live {
            Target::Live {
                room_id: live.room_id,
            }
        } else if let Some(info) = &self.current_play {
            Target::Video {
                cid: info.cid,
                bvid: info.bvid.clone(),
                progress_ms: self.player.as_ref().map_or(0, |p| {
                    (p.position(self.frames_in_seg).max(0.0) * 1000.0) as u64
                }),
            }
        } else {
            return;
        };
        self.open_composer(target);
    }

    pub(super) fn open_comment_input(&mut self) {
        if self.comment_page.aid > 0 {
            self.open_composer(Target::Comment {
                aid: self.comment_page.aid,
            });
        }
    }

    fn open_composer(&mut self, target: Target) {
        if self.composer.target.is_some() {
            return;
        }
        if self.cookies.as_deref().is_none_or(|cookies| {
            crate::net::session::cookie(cookies, "bili_jct").is_none_or(|csrf| csrf.is_empty())
        }) {
            self.toast(crate::i18n::tr("请先登录后再发送"));
            return;
        }
        self.direction_hold = None;
        self.return_hold = None;
        self.danmaku_hold = None;
        self.controls.like_hold = None;
        self.seek_pending = None;
        self.seek_deadline = None;
        let resume_playback = self
            .player
            .as_ref()
            .is_some_and(|p| !p.paused && !p.finished);
        if resume_playback {
            self.player.as_mut().unwrap().pause_toggle();
        }
        let request = self.composer.request + 1;
        self.composer = Composer {
            target: Some(target.clone()),
            request,
            resume_playback,
            ..Default::default()
        };
        let panel = self.ui.global::<crate::VoiceInput>();
        panel.set_title(target.title().into());
        panel.set_text("".into());
        panel.set_hint("".into());
        panel.set_open(true);
        panel.set_scroll(0.0);
    }

    pub(super) fn close_composer(&mut self) {
        if let Some(voice) = &self.voice {
            voice.cancel();
        }
        let resume = self.composer.resume_playback;
        self.composer = Composer {
            request: self.composer.request + 1,
            ..Default::default()
        };
        self.ui.global::<crate::VoiceInput>().set_open(false);
        if resume {
            if let Some(player) = self.player.as_mut().filter(|p| p.paused && !p.finished) {
                player.pause_toggle();
            }
        }
        self.last_input = Instant::now();
    }

    pub(super) fn composer_key(&mut self, key: &str) {
        if self.composer.phase == Phase::Sending {
            return;
        }
        match key {
            "8" | "ESC" | "ESC_HOLD" | "BACKSPACE" => self.close_composer(),
            "6" | "SPACE" if self.composer.phase != Phase::Recording => {
                self.start_recognition();
            }
            "6_UP" | "SPACE_UP" => self.finish_recognition(),
            "TAB" => {
                self.settings.voice_backend = self.settings.voice_backend.cycle(1);
                self.settings_changed = true;
                if self.composer.phase == Phase::Recording {
                    self.start_recognition();
                }
            }
            "7" | "ENTER" => {
                if self.composer.phase == Phase::Ready {
                    self.send_composition();
                } else {
                    self.composer.send_when_ready = true;
                    self.finish_recognition();
                }
            }
            "UP" | "f" | "w" | "DOWN" | "x" | "s" => {
                let panel = self.ui.global::<crate::VoiceInput>();
                let step = if matches!(key, "UP" | "f" | "w") {
                    -24.0
                } else {
                    24.0
                };
                panel.set_scroll((panel.get_scroll() + step).clamp(0.0, panel.get_max_scroll()));
            }
            _ => {}
        }
    }

    fn start_recognition(&mut self) {
        self.composer.request += 1;
        self.composer.phase = Phase::Recording;
        self.composer.send_when_ready = false;
        self.composer.text.clear();
        let panel = self.ui.global::<crate::VoiceInput>();
        panel.set_text("".into());
        panel.set_scroll(0.0);
        panel.set_hint(crate::i18n::tr("正在开启麦克风…").into());
        if let Some(voice) = &self.voice {
            voice.restart(self.settings.voice_backend, self.composer.request);
        } else {
            self.voice = Some(Session::start(
                self.settings.voice_backend,
                self.composer.request,
                self.tx.clone(),
            ));
        }
    }

    fn finish_recognition(&mut self) {
        if self.composer.phase == Phase::Recording {
            if let Some(voice) = &self.voice {
                voice.stop();
            }
            self.composer.phase = Phase::Finishing;
            self.ui
                .global::<crate::VoiceInput>()
                .set_hint(crate::i18n::tr("正在完成识别…").into());
        }
    }

    pub(super) fn voice_event(&mut self, request: u64, event: Event) {
        if request != self.composer.request
            || self.composer.target.is_none()
            || matches!(self.composer.phase, Phase::Ready | Phase::Sending)
        {
            return;
        }
        let panel = self.ui.global::<crate::VoiceInput>();
        match event {
            Event::Started if self.composer.phase == Phase::Recording => {
                panel.set_hint(crate::i18n::tr("正在聆听…").into());
            }
            Event::Partial { text } | Event::Final { text } => {
                self.composer.text = crate::voice::format_text(
                    &text,
                    self.settings.voice_punctuation,
                    self.settings.voice_add_spaces,
                );
                panel.set_text(self.composer.text.as_str().into());
                panel.set_scroll(panel.get_max_scroll());
            }
            Event::Error { message } => {
                self.composer.phase = Phase::Ready;
                self.composer.send_when_ready = false;
                panel.set_hint(crate::i18n::trf("语音输入失败：{}", &[&message]).into());
            }
            Event::Ended { reason } => {
                let send = std::mem::take(&mut self.composer.send_when_ready);
                self.composer.phase = Phase::Ready;
                if reason == "client_cancel" || reason == "error" {
                    panel.set_hint(crate::i18n::tr("语音输入已中断，请重新识别").into());
                } else if self.composer.text.trim().is_empty() {
                    panel.set_hint(crate::i18n::tr("未识别到文字，请重新识别").into());
                } else if send {
                    self.send_composition();
                } else {
                    panel.set_hint(crate::i18n::tr("识别完成").into());
                }
            }
            _ => {}
        }
    }

    fn send_composition(&mut self) {
        let panel = self.ui.global::<crate::VoiceInput>();
        let text = self.composer.text.clone();
        if text.trim().is_empty() {
            panel.set_hint(crate::i18n::tr("未识别到文字，请重新识别").into());
            return;
        }
        let Some(cookies) = self.cookies.clone() else {
            panel.set_hint(crate::i18n::tr("请先登录后再发送").into());
            return;
        };
        let Some(target) = self.composer.target.clone() else {
            return;
        };
        self.composer.phase = Phase::Sending;
        panel.set_hint(crate::i18n::tr("正在发送…").into());
        tasks::send_composition(
            target,
            text,
            cookies,
            self.composer.request,
            self.tx.clone(),
        );
    }

    pub(super) fn composition_sent(
        &mut self,
        request: u64,
        result: Result<Option<model::Comment>, String>,
    ) {
        if request != self.composer.request || self.composer.target.is_none() {
            return;
        }
        match result {
            Ok(comment) => {
                if let Some(comment) = comment {
                    self.comments_request += 1;
                    self.comment_page.items.insert(0, comment);
                    self.comment_page.loading = false;
                    self.comment_page.rebuild();
                }
                self.close_composer();
                self.toast(crate::i18n::tr("发送成功"));
            }
            Err(error) => {
                self.composer.phase = Phase::Ready;
                let panel = self.ui.global::<crate::VoiceInput>();
                panel.set_hint(crate::i18n::trf("发送失败：{}", &[&error]).into());
            }
        }
    }
}
