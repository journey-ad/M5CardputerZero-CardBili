use super::super::*;

#[derive(Default)]
pub(in crate::app) struct LiveRecovery {
    pub attempts: u32,
    pub retry_at: Option<Instant>,
    pub disconnected_at: Option<Instant>,
}

impl LiveRecovery {
    pub fn notice(&self, now: Instant) -> String {
        // 连续断流 30 秒后显示提示，期间保留最后一帧
        if !self
            .disconnected_at
            .is_some_and(|since| now.duration_since(since) >= Duration::from_secs(30))
        {
            return String::new();
        }
        if let Some(at) = self.retry_at.filter(|at| *at > now) {
            let seconds = at.duration_since(now).as_secs_f64().ceil() as u64;
            format!("直播已断开，{seconds} 秒后重试")
        } else {
            "直播已断开，正在重连…".into()
        }
    }
}

impl App {
    pub(in crate::app) fn recover_live(&mut self, reason: &str) {
        if self.page != Page::Player
            || self.live.is_none()
            || self.player.as_ref().is_some_and(|player| player.paused)
        {
            return;
        }
        if let Some(mut player) = self.player.take() {
            self.gen = self.gen.max(player.gen);
            player.stop();
        }
        self.play_request += 1;
        self.latest = None;
        self.frames_in_seg = 0;
        self.live_engine = danmaku::LiveEngine::default();
        self.danmaku_view.clear();
        self.live_recovery
            .disconnected_at
            .get_or_insert_with(Instant::now);
        self.live_recovery.attempts = self.live_recovery.attempts.saturating_add(1);
        // 前三次每秒重试，随后按 3、6、12、24、30 秒退避
        let delay = match self.live_recovery.attempts {
            1..=3 => 1,
            attempts => (3u64 << (attempts - 4).min(4)).min(30),
        };
        self.live_recovery.retry_at = Some(Instant::now() + Duration::from_secs(delay));
        self.status_detail.clear();
        let player = self.ui.global::<crate::Player>();
        player.set_finished(false);
        player.set_paused(false);
        crate::diagnostics::write(format_args!(
            "[直播] 重连等待 次数={} 延迟={delay}s 原因={reason}",
            self.live_recovery.attempts
        ));
    }

    pub(in crate::app) fn tick_live_recovery(&mut self) {
        if !self
            .live_recovery
            .retry_at
            .is_some_and(|at| Instant::now() >= at)
        {
            return;
        }
        self.live_recovery.retry_at = None;
        if self.page != Page::Player {
            return;
        }
        let Some(info) = &self.live else { return };
        self.status = "正在重新连接直播…".into();
        // 重连只替换媒体流，醒目留言与弹幕连接沿用当前直播间
        tasks::start_live(
            info.room_id,
            self.cookies.clone(),
            self.play_request,
            self.tx.clone(),
        );
    }
}
