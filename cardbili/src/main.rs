mod api;
mod app;
mod cache;
mod danmaku;
mod fonts;
mod menu;
mod model;
mod net;
mod player;
mod settings;
mod tasks;
mod util;

slint::include_modules!();

pub enum Msg {
    Liked(u64, Result<bool, String>),
    Reaction(u64, Result<model::ReactionResult, String>),
    Comments(u64, u32, Result<model::CommentPage, String>),
    Frame(u64),
    Volume(Result<i32, String>),
    SegEnd(u64),
    ListLoaded(usize, Vec<model::Item>),
    ListMore(usize, Vec<model::Item>),
    Dynamic(u64, String, Result<model::DynamicPage, String>),
    Following(u64, u32, Result<model::FollowingPage, String>),
    Err(usize, String),
    PlayReady(u64, Result<model::PlayInfo, String>),
    Danmaku(u64, Vec<danmaku::Dm>),
    QrReady(u64, String),
    QrStatus(u64, String),
    QrDone(u64, String),
    Cover(usize, String, u32, u32, Vec<u8>),
    Acct(model::Account, Vec<u8>),
    Detail(u64, model::Detail, u32, u32, Vec<u8>),
    DetailError(u64, String),
    Uploader(u64, u32, Result<model::VideoPage, String>),
    Stats(String, Result<model::VideoStats, String>),
}

fn main() {
    app::run();
}
