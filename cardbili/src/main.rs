mod api;
mod app;
mod cache;
mod danmaku;
mod model;
mod net;
mod player;
mod settings;
mod tasks;
mod util;

slint::include_modules!();

pub enum Msg {
    Frame(u64, Vec<u8>),
    SegEnd(u64),
    ListLoaded(usize, Vec<model::Item>),
    ListMore(usize, Vec<model::Item>),
    Err(usize, String),
    PlayReady(model::PlayInfo),
    Danmaku(u64, Vec<danmaku::Dm>),
    QrReady(String),
    QrStatus(String),
    QrDone(String),
    Cover(usize, String, u32, u32, Vec<u8>),
    Acct(model::Account, Vec<u8>),
    Detail(u64, model::Detail, u32, u32, Vec<u8>),
    DetailError(u64, String),
}

fn main() {
    app::run();
}
