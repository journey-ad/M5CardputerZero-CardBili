use crate::{
    api,
    model::{CommentEmote, VideoAction},
    Msg,
};
use std::{sync::mpsc::Sender, thread};

pub fn fetch_liked(bvid: String, cookies: String, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = api::playback::liked(&bvid, &cookies);
        let _ = tx.send(Msg::Liked(request, result));
    });
}

pub fn react(
    aid: i64,
    bvid: String,
    cookies: String,
    action: VideoAction,
    request: u64,
    tx: Sender<Msg>,
) {
    thread::spawn(move || {
        let result = api::playback::react(aid, &bvid, &cookies, action);
        let _ = tx.send(Msg::Reaction(request, result));
    });
}

pub fn comments(aid: i64, page: u32, cookies: Option<String>, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = api::playback::comments(aid, page, cookies.as_deref());
        let _ = tx.send(Msg::Comments(request, page, result));
    });
}

// 每批最多四个下载线程，完成后统一更新表情
pub fn comment_emotes(emotes: Vec<CommentEmote>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let images = thread::scope(|scope| {
            let handles: Vec<_> = emotes
                .chunks(emotes.len().div_ceil(4).max(1))
                .map(|batch| {
                    scope.spawn(move || {
                        batch
                            .iter()
                            .filter_map(|emote| {
                                crate::cache::emote(&emote.url, emote.size)
                                    .map(|image| (emote.name.clone(), emote.size, image))
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        if !images.is_empty() {
            let _ = tx.send(Msg::CommentEmotes(images));
        }
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
