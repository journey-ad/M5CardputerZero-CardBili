use super::super::*;
use crate::api;
use slint::{ComponentHandle as _, ModelRc, VecModel};

#[derive(Default)]
pub(in crate::app) struct Media {
    subtitle_tracks: Vec<model::SubtitleTrack>,
    subtitle_cues: Vec<model::SubtitleCue>,
    last_subtitle: i32,
    audio_tracks: Vec<model::AudioTrack>,
}

fn target_language() -> &'static str {
    match crate::i18n::current() {
        crate::i18n::Language::Zh => "zh",
        crate::i18n::Language::En => "en",
    }
}

impl App {
    pub(in crate::app) fn load_media(&mut self, info: &model::PlayInfo) {
        self.media = Media::default();
        let player = self.ui.global::<crate::Player>();
        player.set_subtitle_selected(0);
        player.set_subtitle_text("".into());
        player.set_audio_selected(0);
        self.sync_media_labels();
        let request = self.play_request;
        let bvid = info.bvid.clone();
        let cid = info.cid;
        let cookies = self.cookies.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            std::thread::scope(|scope| {
                scope.spawn(|| {
                    let result = api::subtitle::tracks(&bvid, cid, cookies.as_deref());
                    let _ = tx.send(Msg::SubtitleTracks(request, result));
                });
                let result = api::audio::tracks(&bvid, cid, cookies.as_deref());
                let _ = tx.send(Msg::AudioTracks(request, result));
            });
        });
    }

    pub(in crate::app) fn restore_media(&mut self, subtitle: i32, audio: i32) {
        self.sync_media_labels();
        let player = self.ui.global::<crate::Player>();
        player.set_subtitle_selected(subtitle);
        player.set_audio_selected(audio);
        if subtitle > 0 && self.media.subtitle_cues.is_empty() {
            self.apply_subtitle(subtitle);
        }
    }

    fn sync_media_labels(&self) {
        let mut labels: Vec<slint::SharedString> = vec![crate::i18n::tr("关闭").into()];
        for track in &self.media.subtitle_tracks {
            let name = if track.lan_doc.is_empty() {
                &track.lan
            } else {
                &track.lan_doc
            };
            labels.push(if track.is_ai {
                format!("{name}（AI）").into()
            } else {
                name.into()
            });
        }
        let player = self.ui.global::<crate::Player>();
        player.set_subtitle_labels(ModelRc::new(VecModel::from(labels)));
        let mut labels = vec![crate::i18n::tr("自动").into()];
        labels.extend(
            self.media
                .audio_tracks
                .iter()
                .map(|track| track.title.as_str().into()),
        );
        player.set_audio_labels(ModelRc::new(VecModel::from(labels)));
    }

    // 字幕轨加载完成，默认选择目标语言字幕
    pub(in crate::app) fn subtitles_loaded(
        &mut self,
        request: u64,
        result: Result<Vec<model::SubtitleTrack>, String>,
    ) {
        if request != self.play_request || self.page != Page::Player {
            return;
        }
        let tracks = match result {
            Ok(tracks) => tracks,
            Err(error) => {
                crate::diagnostics::write(format_args!("[字幕] 加载失败: {error}"));
                return;
            }
        };
        crate::diagnostics::write(format_args!("[字幕] 轨道={}", tracks.len()));
        let selected = tracks
            .iter()
            .position(|track| track.lan.to_lowercase().starts_with(target_language()))
            .or_else(|| tracks.iter().position(|track| track.is_ai))
            .map(|index| index as i32 + 1)
            .unwrap_or(0);
        self.media.subtitle_tracks = tracks;
        self.sync_media_labels();
        self.apply_subtitle(selected);
    }

    // 0 为关闭，其余选项对应字幕轨
    pub(super) fn apply_subtitle(&mut self, index: i32) {
        let url = if index == 0 {
            None
        } else {
            let Some(track) = self.media.subtitle_tracks.get((index - 1) as usize) else {
                return;
            };
            Some(track.url.clone())
        };
        let player = self.ui.global::<crate::Player>();
        player.set_subtitle_selected(index);
        if index > 0 {
            self.media.last_subtitle = index;
        }
        player.set_subtitle_text("".into());
        self.media.subtitle_cues.clear();
        let Some(url) = url else { return };
        let request = self.play_request;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = api::subtitle::cues(&url);
            let _ = tx.send(Msg::SubtitleCues(request, index, result));
        });
    }

    pub(super) fn toggle_subtitle(&mut self) {
        if self.media.subtitle_tracks.is_empty() {
            return;
        }
        let selected = self.ui.global::<crate::Player>().get_subtitle_selected();
        self.apply_subtitle(if selected > 0 { 0 } else { self.media.last_subtitle.max(1) });
    }

    pub(in crate::app) fn subtitle_cues_loaded(
        &mut self,
        request: u64,
        index: i32,
        result: Result<Vec<model::SubtitleCue>, String>,
    ) {
        if request != self.play_request
            || self.page != Page::Player
            || index != self.ui.global::<crate::Player>().get_subtitle_selected()
        {
            return;
        }
        match result {
            Ok(cues) => self.media.subtitle_cues = cues,
            Err(error) => crate::diagnostics::write(format_args!("[字幕] 加载失败: {error}")),
        }
    }

    pub(super) fn sync_subtitle(&self, position: f64) {
        let player = self.ui.global::<crate::Player>();
        let text = self
            .media
            .subtitle_cues
            .iter()
            .find(|cue| self.current_play.is_some() && cue.from <= position && position < cue.to)
            .map(|cue| cue.content.as_str())
            .unwrap_or_default();
        if player.get_subtitle_text() != text {
            player.set_subtitle_text(text.into());
        }
    }

    // 自动选择目标语言的 AI 翻译音轨
    pub(in crate::app) fn audio_loaded(
        &mut self,
        request: u64,
        result: Result<Vec<model::AudioTrack>, String>,
    ) {
        if request != self.play_request || self.page != Page::Player {
            return;
        }
        let tracks = match result {
            Ok(tracks) => tracks,
            Err(error) => {
                crate::diagnostics::write(format_args!("[音轨] 加载失败: {error}"));
                return;
            }
        };
        crate::diagnostics::write(format_args!(
            "[音轨] 共 {} 条: {:?}",
            tracks.len(),
            tracks
                .iter()
                .map(|track| (&track.lang, &track.title, track.production_type))
                .collect::<Vec<_>>()
        ));
        self.media.audio_tracks = tracks;
        self.sync_media_labels();
        self.select_audio(0);
    }

    // 0 为自动，其余选项对应音轨
    pub(super) fn select_audio(&mut self, label: i32) {
        let index = if label == 0 {
            let Some(index) = self.media.audio_tracks.iter().position(|track| {
                track.production_type == 2
                    && track.lang.to_lowercase().starts_with(target_language())
            }) else {
                crate::diagnostics::write(format_args!("[音轨] 无 AI 翻译音轨，保持原声"));
                return;
            };
            index
        } else {
            (label - 1) as usize
        };
        let Some(track) = self.media.audio_tracks.get(index).cloned() else {
            return;
        };
        let Some(info) = self.current_play.as_ref() else {
            return;
        };
        let selected = index as i32 + 1;
        self.ui
            .global::<crate::Player>()
            .set_audio_selected(selected);
        crate::diagnostics::write(format_args!(
            "[音轨] 切换到 {} ({})",
            track.title, track.lang
        ));
        let bvid = info.bvid.clone();
        let cid = info.cid;
        let cdn = self.settings.cdn.clone();
        let cookies = self.cookies.clone();
        let request = self.play_request;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = api::audio::stream_urls(&bvid, cid, &cdn, cookies.as_deref(), &track);
            let _ = tx.send(Msg::AudioSwitched(request, selected, result));
        });
    }

    // 替换播放地址，保留进度、暂停状态与倍速
    pub(in crate::app) fn audio_switched(
        &mut self,
        request: u64,
        index: i32,
        result: Result<api::Streams, String>,
    ) {
        if request != self.play_request
            || self.page != Page::Player
            || index != self.ui.global::<crate::Player>().get_audio_selected()
        {
            return;
        }
        let streams = match result {
            Ok(streams) => streams,
            Err(error) => {
                crate::diagnostics::write(format_args!("[音轨] 切换失败: {error}"));
                self.status = error;
                return;
            }
        };
        let Some(info) = self.current_play.as_mut() else {
            return;
        };
        crate::diagnostics::write(format_args!(
            "[音轨] 切换成功 画质={}",
            streams.tech.quality
        ));
        info.video_url = streams.video_url;
        info.audio_url = streams.audio_url;
        if streams.duration > 0.0 {
            info.duration = streams.duration;
        }
        info.tech = streams.tech;
        let state = if let Some(mut player) = self.player.take() {
            let state = player::PlaybackState {
                position: player.position(self.frames_in_seg),
                paused: player.paused,
                rate: player.rate,
            };
            self.gen = self.gen.max(player.gen);
            player.stop();
            state
        } else {
            player::PlaybackState::default()
        };
        self.gen += 1;
        self.frames_in_seg = 0;
        self.player = Some(player::Player::start(
            info,
            self.cookies.clone().unwrap_or_default(),
            self.gen,
            self.tx.clone(),
            &self.settings,
            state,
        ));
        self.ui.global::<crate::Player>().set_has_frame(false);
        self.status = crate::i18n::tr("加载中…").into();
    }
}
