use crate::{api, model::VideoAction, Msg};
use std::{sync::mpsc::Sender, thread};

pub fn fetch_liked(bvid: String, cookies: String, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = api::playback::liked(&bvid, &cookies);
        let _ = tx.send(Msg::Liked(request, result));
    });
}

pub fn react(bvid: String, cookies: String, action: VideoAction, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = api::playback::react(&bvid, &cookies, action);
        let _ = tx.send(Msg::Reaction(request, result));
    });
}

pub fn comments(aid: i64, page: u32, cookies: Option<String>, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = api::playback::comments(aid, page, cookies.as_deref());
        let _ = tx.send(Msg::Comments(request, page, result));
    });
}

pub fn chapters(bvid: String, cid: i64, cookies: Option<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let _ = tx.send(Msg::Chapters(
            cid,
            api::sections::chapters(&bvid, cid, cookies.as_deref()),
        ));
    });
}

pub fn watching(bvid: String, cid: i64, cookies: Option<String>, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let _ = tx.send(Msg::Watching(
            request,
            api::sections::watching(&bvid, cid, cookies.as_deref()),
        ));
    });
}
