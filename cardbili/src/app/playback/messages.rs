use super::super::*;

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
            player.frame_ready();
            self.frames_in_seg = index;
            self.latest = Some((g, pixels));
        }
    }

    pub(in crate::app) fn playback_ended(&mut self, g: u64) {
        if self.player.as_ref().is_some_and(|p| p.gen == g)
            && self.current_play.as_ref().is_some_and(|info| {
                self.player.as_ref().unwrap().position(self.frames_in_seg) >= info.duration - 1.0
            })
        {
            self.report_history(true, true);
        }
        if let Some(p) = self.player.as_mut() {
            if g == p.gen {
                p.finished = true;
                crate::diagnostics::write(format_args!(
                    "[播放] 已结束 gen={g} 位置={:.3}s 帧数={}",
                    p.position(self.frames_in_seg),
                    self.frames_in_seg
                ));
                self.status = if self.frames_in_seg == 0 {
                    "播放失败，按播放键重试"
                } else {
                    "播放结束，按 ESC 返回"
                }
                .into();
                self.status_detail.clear();
            }
        }
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
                self.gen += 1;
                self.frames_in_seg = 0;
                self.player = Some(player::Player::start(
                    &info,
                    self.cookies.clone().unwrap_or_default(),
                    self.gen,
                    self.tx.clone(),
                    player::PlaybackState {
                        position: std::mem::take(&mut self.play_start),
                        ..Default::default()
                    },
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
                self.current_play = Some(info);
                self.refresh_liked();
                self.status = "加载中…".into();
                self.status_detail.clear();
            }
            Err(error) => {
                self.play_start = 0.0;
                self.status = error;
                self.status_detail.clear();
            }
        }
    }

    pub(in crate::app) fn danmaku_loaded(&mut self, g: u64, eng: danmaku::Engine) {
        if g != self.gen {
            return;
        }
        self.danmaku_view.clear();
        let position = self
            .player
            .as_ref()
            .map(|player| player.position(self.frames_in_seg))
            .unwrap_or(0.0);
        if self.settings.danmaku_enabled {
            self.danmaku_view.update(&eng, position);
        }
        self.danmaku = Some(eng);
    }
}
