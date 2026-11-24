use super::super::*;
use slint::ComponentHandle as _;

impl App {
    pub(in crate::app) fn frame_ready(&mut self, g: u64) {
        if self.page != Page::Player {
            return;
        }
        let Some(player) = self.player.as_mut().filter(|player| player.gen == g) else {
            return;
        };
        let Some((index, pixels)) = player.take_frame() else {
            return;
        };
        if !player.paused || self.frames_in_seg == 0 {
            if self.live.is_some() && self.live_recovery.attempts > 0 {
                crate::diagnostics::write(format_args!("[直播] 重连恢复 gen={g}"));
                self.live_recovery = super::LiveRecovery::default();
            }
            player.frame_ready();
            self.frames_in_seg = index;
            if self.latest.as_ref().is_some_and(|(generation, _)| *generation == g) {
                player.discard_frame();
            }
            self.latest = Some((g, pixels));
        } else {
            player.discard_frame();
        }
    }

    pub(in crate::app) fn playback_ended(&mut self, g: u64) {
        if self.live.is_some()
            && self
                .player
                .as_ref()
                .is_some_and(|p| p.gen == g && !p.finished)
        {
            self.recover_live(crate::i18n::tr("直播流已断开"));
            return;
        }
        if self.player.as_ref().is_some_and(|p| p.gen == g)
            && self.current_play.as_ref().is_some_and(|info| {
                self.player.as_ref().unwrap().position(self.frames_in_seg) >= info.duration - 1.0
            })
        {
            self.report_history(true, true);
        }
        if let Some(p) = self.player.as_mut() {
            if g == p.gen && !p.finished {
                p.finished = true;
                p.stop();
                self.live_danmaku = None;
                crate::diagnostics::write(format_args!(
                    "[播放] 已结束 gen={g} 位置={:.3}s 帧数={}",
                    p.position(self.frames_in_seg),
                    self.frames_in_seg
                ));
                self.status = if self.frames_in_seg == 0 {
                    crate::i18n::tr("播放失败，按播放键重试")
                } else if self.live.is_some() {
                    crate::i18n::tr("直播已结束，按 ESC 返回")
                } else {
                    crate::i18n::tr("播放结束，按 ESC 返回")
                }
                .into();
                self.status_detail.clear();
            }
        }
    }

    pub(in crate::app) fn playback_failed(&mut self, g: u64, error: String) {
        let Some(player) = self
            .player
            .as_mut()
            .filter(|player| player.gen == g && !player.finished)
        else {
            return;
        };
        player.stop();
        player.finished = true;
        self.live_danmaku = None;
        self.status = error;
        self.status_detail.clear();
        self.toast(&self.status);
    }

    pub(in crate::app) fn play_ready(
        &mut self,
        request: u64,
        result: Result<model::PlayInfo, String>,
    ) {
        if self.page != Page::Player || request != self.play_request {
            return;
        }
        match result {
            Ok(info) => {
                let state = if let Some(mut previous) = self.player.take() {
                    self.gen = self.gen.max(previous.gen);
                    let position = previous.position(self.frames_in_seg);
                    let state = player::PlaybackState {
                        position: if position >= previous.duration - 1.0 {
                            0.0
                        } else {
                            position
                        },
                        paused: false,
                        rate: previous.rate,
                        finished: false,
                    };
                    previous.stop();
                    state
                } else {
                    player::PlaybackState {
                        position: std::mem::take(&mut self.play_start),
                        ..Default::default()
                    }
                };
                self.gen += 1;
                self.frames_in_seg = 0;
                if let Some(engine) = self.danmaku.as_mut() {
                    engine.clear_echoes();
                }
                self.player = Some(player::Player::start(
                    &info,
                    self.cookies.clone().unwrap_or_default(),
                    self.gen,
                    self.tx.clone(),
                    &self.settings,
                    state,
                ));
                tasks::start_danmaku(info.cid, self.cookies.clone(), self.gen, self.tx.clone());
                self.setup_sections(info.bvid.clone(), info.cid, info.parts.clone());
                tasks::playback::watching(
                    info.bvid.clone(),
                    info.cid,
                    self.cookies.clone(),
                    self.play_request,
                    self.tx.clone(),
                );
                self.load_media(&info);
                self.current_play = Some(info);
                self.refresh_liked();
                self.status = crate::i18n::tr("加载中…").into();
                self.status_detail.clear();
            }
            Err(error) => {
                self.play_start = 0.0;
                self.status = error;
                self.status_detail.clear();
            }
        }
    }

    pub(in crate::app) fn live_ready(
        &mut self,
        request: u64,
        result: Result<model::LiveInfo, crate::api::live::RoomError>,
    ) {
        if self.page != Page::Player || request != self.play_request {
            return;
        }
        match result {
            Ok(info) => {
                self.gen += 1;
                self.frames_in_seg = 0;
                let g_player = self.ui.global::<crate::Player>();
                g_player.set_ptitle(info.title.as_str().into());
                g_player.set_watching(info.online.as_str().into());
                let play = model::PlayInfo {
                    bvid: info.room_id.to_string(),
                    owner: model::Owner {
                        mid: String::new(),
                        name: info.uname.clone(),
                    },
                    title: info.title.clone(),
                    video_url: info.play_url.clone(),
                    live: true,
                    ..Default::default()
                };
                self.player = Some(player::Player::start(
                    &play,
                    self.cookies.clone().unwrap_or_default(),
                    self.gen,
                    self.tx.clone(),
                    &self.settings,
                    player::PlaybackState::default(),
                ));
                if self.live_danmaku.is_none() {
                    self.live_danmaku = Some(crate::live_danmaku::Connection::start(
                        info.room_id,
                        self.cookies.clone(),
                    ));
                }
                self.status = crate::i18n::tr("加载中…").into();
                self.status_detail.clear();
                self.live = Some(info);
            }
            Err(crate::api::live::RoomError::Request(error)) => {
                crate::diagnostics::write(format_args!("[直播] 获取播放地址失败: {error}"));
                self.recover_live(crate::i18n::tr("直播连接失败"));
            }
            Err(error) => {
                self.live_recovery = super::LiveRecovery::default();
                self.live_danmaku = None;
                self.ui.global::<crate::Player>().set_has_frame(false);
                self.status = error.to_string();
                self.status_detail.clear();
            }
        }
    }

    pub(in crate::app) fn danmaku_loaded(&mut self, g: u64, mut eng: danmaku::Engine) {
        if g != self.gen {
            return;
        }
        if let Some(previous) = self.danmaku.take() {
            eng.retain_own(previous);
        }
        self.danmaku_view.clear();
        let position = self
            .player
            .as_ref()
            .map(|player| player.position(self.frames_in_seg))
            .unwrap_or(0.0);
        if self.controls.danmaku_enabled {
            self.danmaku_view.update(&eng, position);
        }
        self.danmaku = Some(eng);
    }
}
