mod about;
mod account;
mod cache;
mod cdn;
mod collection;
mod comments;
mod compose;
mod components;
mod confirmation;
mod detail;
mod favorites;
mod following;
mod history;
mod home;
mod input;
mod login;
mod messages;
mod navigation;
mod platform;
mod playback;
mod scroll;
mod sections;
mod state;
mod view;

use crate::player::{HEIGHT, WIDTH};
use crate::settings::Settings;
use crate::{
    danmaku, menu, model, player, tasks, util, AppWindow, Cmt, HomeTab, MenuPosition, Msg, Page,
    VideoLayout, VideoRow,
};
use collection::CollectionState;
use components::*;
use detail::DetailDisplay;
use detail::{show_detail, show_detail_loading, sync_thumb};
use navigation::BackTarget;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use state::App;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

// 取当前语言最长标签宽度，写入页签统一宽度
fn refresh_tab_label_max_width(ui: &crate::AppWindow) {
    let meta = ui.global::<crate::Meta>();
    let max = meta
        .get_tab_names()
        .iter()
        .map(|name| {
            crate::fonts::LayoutFont::new(crate::fonts::UI_FONT_FAMILY, crate::fonts::UI_FONT_SIZE)
                .advance(name.as_str())
        })
        .max()
        .unwrap_or(0);
    meta.set_tab_label_max_width(max as f32);
}

// 切换界面语言并刷新页签标签宽度
pub(crate) fn set_language(ui: &crate::AppWindow, language: crate::i18n::Language) {
    crate::i18n::set(language);
    ui.global::<crate::I18n>()
        .set_lang(if language == crate::i18n::Language::Zh { 0 } else { 1 });
    refresh_tab_label_max_width(ui);
}

pub fn run(settings: Settings) {
    crate::cache::init();
    let (tx, rx) = mpsc::channel();
    let (ktx, krx) = mpsc::channel();
    let mut display = platform::Display::new(ktx.clone());
    let ui = AppWindow::new().expect("ui");
    {
        let i18n = ui.global::<crate::I18n>();
        i18n.on_lookup(|text: slint::SharedString| crate::i18n::tr(text.as_str()).into());
    }
    set_language(&ui, crate::i18n::current());
    crate::title_i18n::set_provider(settings.translate_provider);
    ui.show().expect("show");
    connect_ui(&ui, tx.clone(), ktx);
    let mut app = App::new(ui, tx, settings);
    #[cfg(feature = "desktop")]
    if !crate::diagnostics::ffmpeg_available() {
        #[cfg(windows)]
        app.confirm(
            confirmation::Action::FfmpegInstall,
            crate::i18n::tr("未找到 FFmpeg"),
            crate::i18n::tr("播放需要 FFmpeg，是否通过 winget 自动安装？"),
            crate::i18n::tr("安装"),
        );
        #[cfg(target_os = "macos")]
        app.confirm(
            confirmation::Action::FfmpegNotice,
            crate::i18n::tr("未找到 FFmpeg"),
            crate::i18n::tr("播放需要 FFmpeg，请先执行 brew install ffmpeg"),
            crate::i18n::tr("知道了"),
        );
        #[cfg(all(not(windows), not(target_os = "macos")))]
        app.confirm(
            confirmation::Action::FfmpegNotice,
            crate::i18n::tr("未找到 FFmpeg"),
            crate::i18n::tr("播放需要 FFmpeg，请先通过系统包管理器安装"),
            crate::i18n::tr("知道了"),
        );
    }
    let mut pending = None;
    while !app.quit {
        let tick_end = Instant::now() + Duration::from_millis(16);
        if app
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            break;
        }
        app.settings_changed = false;
        app.go_back = false;
        app.tick();
        if app.quit {
            return;
        }
        if let Some(message) = pending.take() {
            app.handle_message(message);
        }
        while let Ok(message) = rx.try_recv() {
            app.handle_message(message);
        }
        display.poll_keys(&app.ui);
        while let Ok(key) = krx.try_recv() {
            app.handle_key(key);
            if app.quit {
                return;
            }
        }
        app.navigate();
        app.sync_view();
        if !display.render() {
            break;
        }
        pending = rx
            .recv_timeout(tick_end.saturating_duration_since(Instant::now()))
            .ok();
    }
    app.close_cdn();
    app.report_history(true, false);
    if let Some(mut player) = app.player.take() {
        player.stop();
    }
    app.history.flush();
}

fn connect_ui(ui: &AppWindow, tx: mpsc::Sender<Msg>, ktx: mpsc::Sender<String>) {
    let area_keys = ktx.clone();
    ui.global::<crate::LiveAreas>().on_key(move |key| {
        let _ = area_keys.send(key.to_string());
    });
    let voice_keys = ktx.clone();
    ui.global::<crate::VoiceInput>().on_key(move |key| {
        let _ = voice_keys.send(key.to_string());
    });
    ui.global::<crate::Player>().on_key(move |key| {
        let _ = ktx.send(key.to_string());
    });
    let confirm_tx = tx.clone();
    ui.global::<crate::Confirm>().on_result(move |accepted| {
        let _ = confirm_tx.send(Msg::Confirmed(accepted));
    });
}

#[cfg(test)]
mod tests;
