#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api;
mod app;
#[cfg(feature = "desktop")]
mod desktop_shim;
mod cache;
mod cdn;
mod danmaku;
mod diagnostics;
mod fonts;
mod emoji;
mod menu;
mod live_danmaku;
mod super_chat;
mod model;
mod net;
mod player;
mod settings;
mod tasks;
mod util;

slint::include_modules!();

pub enum Msg {
    Confirmed(bool),
    CacheUsage(Result<u64, String>),
    CacheCleared(Result<(), String>),
    CdnSpeed(u64, usize, String),
    Chapters(i64, Result<Vec<model::Chapter>, String>),
    Watching(u64, Result<String, String>),
    Liked(u64, Result<bool, String>),
    Reaction(u64, Result<model::ReactionResult, String>),
    Comments(u64, u32, Result<model::CommentPage, String>),
    Frame(u64),
    Volume(Result<i32, String>),
    PlaybackEnded(u64),
    PlaybackFailed(u64, String),
    ListLoaded(usize, Vec<model::Item>),
    ListMore(usize, Vec<model::Item>),
    Dynamic(u64, String, Result<model::DynamicPage, String>),
    History(u64, bool, Result<model::HistoryPage, String>),
    Favorites(u64, Result<Vec<model::FavoriteFolder>, String>),
    FavoriteCover(u64, i64, cache::Pixels),
    Following(u64, u32, Result<model::FollowingPage, String>),
    Err(usize, String),
    PlayReady(u64, Result<model::PlayInfo, String>),
    LiveReady(u64, Result<model::LiveInfo, api::live::RoomError>),
    LiveAreas(Result<Vec<model::LiveArea>, String>),
    LiveList(u64, u32, Result<model::VideoPage, String>),
    Danmaku(u64, danmaku::Engine),
    QrReady(u64, String),
    QrStatus(u64, String),
    QrDone(u64, net::session::Login),
    Cover(usize, String, u32, u32, Vec<u8>),
    Acct(model::Account, Option<cache::Pixels>),
    FollowingAvatar(u64, String, cache::Pixels),
    Detail(u64, model::Detail, u32, u32, Vec<u8>),
    DetailError(u64, String),
    Collection(u64, u32, Result<model::VideoPage, String>),
    AuthorProfile(
        u64,
        Result<(model::AuthorProfile, Option<cache::Pixels>), String>,
    ),
}

fn main() {
    let mut settings = settings::Settings::load();
    let force_log = std::env::var_os("CARDBILI_DEBUG").is_some();
    if let Err(error) = diagnostics::set_enabled(settings.debug_log || force_log) {
        settings.debug_log = false;
        eprintln!("开启调试日志失败: {error}");
    }
    if force_log {
        diagnostics::set_mirror(true);
    }
    diagnostics::write(format_args!("[应用] 启动 pid={}", std::process::id()));
    diagnostics::write(format_args!("[设置] 已加载 {settings:?}"));
    app::run(settings);
    diagnostics::write(format_args!("[应用] 已退出"));
}
