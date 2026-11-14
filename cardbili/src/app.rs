mod account;
mod comments;
mod components;
mod detail;
mod favorites;
mod following;
mod history;
mod home;
mod input;
mod messages;
mod navigation;
mod pages;
mod platform;
mod playback;
mod scroll;
mod state;
mod view;

use crate::player::{HEIGHT, VID_BYTES, WIDTH};
use crate::settings::Settings;
use crate::{danmaku, menu, model, player, tasks, util, AppWindow, Cmt, Msg, VideoRow};
use components::*;
use detail::{show_detail, show_detail_loading, sync_thumb};
use navigation::{BackTarget, CollectionState, DetailDisplay};
use slint::{ComponentHandle, ModelRc, Rgb8Pixel, VecModel};
use state::App;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub fn run() {
    crate::cache::init();
    let (tx, rx) = mpsc::channel();
    let (ktx, krx) = mpsc::channel();
    let mut display = platform::Display::new(ktx.clone());
    let ui = AppWindow::new().expect("ui");
    ui.show().expect("show");
    ui.on_player_key(move |key| {
        let _ = ktx.send(key.to_string());
    });
    let mut app = App::new(ui, tx);
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
        if let Some(message) = pending.take() {
            app.handle_message(message);
        }
        while let Ok(message) = rx.try_recv() {
            app.handle_message(message);
        }
        display.poll_keys();
        while let Ok(key) = krx.try_recv() {
            app.handle_key(key);
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
    app.report_history(true, false);
    if let Some(mut player) = app.player.take() {
        player.stop();
    }
    app.history.flush();
}
