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

// 表情图片依次下载，成功后立即显示，失败时保留原文
pub fn comment_emotes(emotes: Vec<CommentEmote>, tx: Sender<Msg>) {
    thread::spawn(move || {
        for emote in emotes {
            if let Some(image) = crate::cache::emote(&emote.url, crate::emoji::SIZE as u32) {
                if tx.send(Msg::CommentEmote(emote.name, image)).is_err() {
                    break;
                }
            }
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
