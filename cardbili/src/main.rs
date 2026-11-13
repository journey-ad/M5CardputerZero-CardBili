mod app;
mod api;
mod model;
mod net;
mod player;
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
    QrReady(String),
    QrStatus(String),
    QrDone(String),
    Cover(usize, usize, u32, u32, Vec<u8>),
    Acct(model::Account, Vec<u8>),
    Detail(model::Detail, u32, u32, Vec<u8>),
}

fn main() {
    app::run();
}
