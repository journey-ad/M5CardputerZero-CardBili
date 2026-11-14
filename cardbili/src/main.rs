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
    Favorites(u64, Result<Vec<model::FavoriteFolder>, String>),
    Following(u64, u32, Result<model::FollowingPage, String>),
    Err(usize, String),
    PlayReady(u64, Result<model::PlayInfo, String>),
    Danmaku(u64, Vec<danmaku::Dm>),
    QrReady(u64, String),
    QrStatus(u64, String),
    QrDone(u64, String),
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
    Stats(String, Result<model::VideoStats, String>),
}

fn main() {
    app::run();
}
