pub mod playback;

use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use crate::{api, util, Msg};

// 先发布缓存，再发布最新数据；刷新失败时保留已展示的缓存
fn cached_then_refresh<T>(
    force: bool,
    fetch: impl Fn() -> Result<T, String>,
    publish: impl Fn(Result<T, String>),
) {
    let mut showed = false;
    if !force {
        crate::cache::set_cached_only(true);
        let cached = fetch();
        crate::cache::set_cached_only(false);
        if let Ok(value) = cached {
            publish(Ok(value));
            showed = true;
        }
    }
    crate::cache::set_force(true);
    let fresh = fetch();
    if fresh.is_ok() || !showed {
        publish(fresh);
    }
}

pub fn load_list(tab: usize, cookies: Option<String>, tx: Sender<Msg>, force: bool) {
    if tab == 2 {
        load_dynamic(0, String::new(), cookies, tx, force);
        return;
    }
    thread::spawn(move || {
        cached_then_refresh(
            force,
            || match tab {
                0 => api::rcmd(cookies.as_deref(), 1),
                1 => api::popular(cookies.as_deref(), 1),
                _ => Err("未知视频列表".into()),
            },
            |result| {
                let message = match result {
                    Ok(items) => Msg::ListLoaded(tab, items),
                    Err(error) => Msg::Err(tab, error),
                };
                let _ = tx.send(message);
            },
        );
    });
}

pub fn load_live_list(
    request: u64,
    area: crate::model::LiveArea,
    page: u32,
    tx: Sender<Msg>,
    force: bool,
) {
    thread::spawn(move || {
        cached_then_refresh(
            force || page > 1,
            || api::live::rooms(page, &area),
            |result| {
                let _ = tx.send(Msg::LiveList(request, page, result));
            },
        );
    });
}

pub fn fetch_account(cookies: Option<String>, tx: Sender<Msg>, force: bool) {
    thread::spawn(move || {
        cached_then_refresh(
            force,
            || api::account(cookies.as_deref()),
            |result| {
                let message = match result {
                    Ok(account) => {
                        let avatar = crate::cache::avatar(&account.face);
                        Msg::Acct(account, avatar)
                    }
                    Err(error) => Msg::Err(9, error),
                };
                let _ = tx.send(message);
            },
        );
    });
}

pub fn spawn_covers(tab: usize, items: Vec<crate::model::Item>, tx: Sender<Msg>) {
    thread::spawn(move || {
        for it in &items {
            if it.pic.is_empty() {
                continue;
            }
            if let Some((w, h, raw)) = util::fetch_cover(&it.pic) {
                if tx
                    .send(Msg::Cover(tab, it.bvid.clone(), w, h, raw))
                    .is_err()
                {
                    break;
                }
            }
        }
    });
}

pub fn load_more(tab: usize, page: u32, cookies: Option<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let res = match tab {
            0 => api::rcmd(cookies.as_deref(), page),
            1 => api::popular(cookies.as_deref(), page),
            _ => Ok(Vec::new()),
        };
        match res {
            Ok(items) => {
                let _ = tx.send(Msg::ListMore(tab, items));
            }
            Err(e) => {
                let _ = tx.send(Msg::Err(tab, e));
            }
        }
    });
}

pub fn start_play(
    bvid: String,
    cid: Option<i64>,
    cookies: Option<String>,
    request: u64,
    cdn: String,
    tx: Sender<Msg>,
) {
    crate::diagnostics::write(format_args!(
        "[播放] 获取播放信息 request={request} bvid={bvid} cid={cid:?}"
    ));
    thread::spawn(move || {
        let result = api::play_info(&bvid, cid, cookies.as_deref(), &cdn);
        let _ = tx.send(Msg::PlayReady(request, result));
    });
}

pub fn start_live(room_id: u64, cookies: Option<String>, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let result = api::live_room(room_id, cookies.as_deref());
        let _ = tx.send(Msg::LiveReady(request, result));
    });
}

pub fn load_collection(
    source: crate::model::VideoSource,
    page: u32,
    cookies: Option<String>,
    request: u64,
    tx: Sender<Msg>,
) {
    thread::spawn(move || {
        if matches!(source, crate::model::VideoSource::FollowedLive) {
            let result = cookies
                .as_deref()
                .ok_or_else(|| "未登录，请先登录".to_string())
                .and_then(|cookies| api::live::followed_rooms(page, cookies));
            let _ = tx.send(Msg::Collection(request, page, result));
            return;
        }
        if let crate::model::VideoSource::Related(bvid) = &source {
            crate::cache::set_force(true);
            let result = api::related_videos(bvid, cookies.as_deref());
            let _ = tx.send(Msg::Collection(request, page, result));
            return;
        }
        cached_then_refresh(
            page > 1,
            || match &source {
                crate::model::VideoSource::Author(owner) => {
                    api::uploader_videos(&owner.mid, page, cookies.as_deref())
                }
                crate::model::VideoSource::Favorites(folder) => cookies
                    .as_deref()
                    .ok_or_else(|| "未登录，请先登录".to_string())
                    .and_then(|cookies| api::favorites::videos(folder.id, page, cookies)),
                crate::model::VideoSource::Related(bvid) => {
                    api::related_videos(bvid, cookies.as_deref())
                }
                crate::model::VideoSource::FollowedLive => unreachable!(),
            },
            |result| {
                let _ = tx.send(Msg::Collection(request, page, result));
            },
        );
    });
}

pub fn load_author_profile(mid: String, cookies: Option<String>, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        cached_then_refresh(
            false,
            || api::author_profile(&mid, cookies.as_deref()),
            |result| {
                let result = result.map(|profile| {
                    let avatar = crate::cache::avatar(&profile.face);
                    (profile, avatar)
                });
                let _ = tx.send(Msg::AuthorProfile(request, result));
            },
        );
    });
}

// 播放就绪后单独拉取弹幕，失败时回空列表，不影响播放
pub fn start_danmaku(cid: i64, cookies: Option<String>, gen: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let list = api::danmaku_list(cid, cookies.as_deref()).unwrap_or_default();
        let engine = crate::danmaku::Engine::new(list);
        let _ = tx.send(Msg::Danmaku(gen, engine));
    });
}

pub fn start_detail(bvid: String, cookies: Option<String>, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let build = |d: crate::model::Detail| -> (crate::model::Detail, u32, u32, Vec<u8>) {
            let mut w = 0u32;
            let mut h = 0u32;
            let mut raw = Vec::new();
            if !d.pic.is_empty() {
                if let Some((cw, ch, cr)) = util::fetch_cover(&d.pic) {
                    w = cw;
                    h = ch;
                    raw = cr;
                }
            }
            (d, w, h, raw)
        };
        cached_then_refresh(
            false,
            || api::video_detail(&bvid, cookies.as_deref()),
            |result| {
                let message = match result {
                    Ok(detail) => {
                        let (detail, w, h, raw) = build(detail);
                        Msg::Detail(request, detail, w, h, raw)
                    }
                    Err(error) => Msg::DetailError(request, error),
                };
                let _ = tx.send(message);
            },
        );
    });
}

pub fn start_login(request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        let client = crate::net::new_agent();
        let (url, key) = match api::auth::qr_generate(&client) {
            Ok(v) => v,
            Err(e) => {
                let _ = tx.send(Msg::QrStatus(request, e));
                return;
            }
        };
        let _ = tx.send(Msg::QrReady(request, url));
        let expires = std::time::Instant::now() + Duration::from_secs(180);
        loop {
            if std::time::Instant::now() >= expires {
                let _ = tx.send(Msg::QrStatus(request, "二维码已过期，请刷新".into()));
                break;
            }
            match api::auth::qr_poll(&key, &client) {
                api::auth::Poll::Success(ck) => {
                    let _ = tx.send(Msg::QrDone(request, ck));
                    break;
                }
                api::auth::Poll::Scanned => {
                    let _ = tx.send(Msg::QrStatus(request, "已扫码，请在手机上确认".into()));
                }
                api::auth::Poll::Pending => {}
                api::auth::Poll::Expired => {
                    let _ = tx.send(Msg::QrStatus(request, "二维码已过期，请刷新".into()));
                    break;
                }
                api::auth::Poll::Err(e) => {
                    let _ = tx.send(Msg::QrStatus(request, e));
                    break;
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

pub fn load_dynamic(
    request: u64,
    offset: String,
    cookies: Option<String>,
    tx: Sender<Msg>,
    force: bool,
) {
    thread::spawn(move || {
        cached_then_refresh(
            force || !offset.is_empty(),
            || {
                cookies
                    .as_deref()
                    .ok_or_else(|| "未登录，请先登录".to_string())
                    .and_then(|cookies| api::dynamic(cookies, &offset))
            },
            |result| {
                let _ = tx.send(Msg::Dynamic(request, offset.clone(), result));
            },
        );
    });
}

pub fn load_following(mid: String, page: u32, cookies: String, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        cached_then_refresh(
            page > 1,
            || api::followings(&mid, page, &cookies),
            |result| {
                let _ = tx.send(Msg::Following(request, page, result));
            },
        );
    });
}

pub fn load_following_avatars(users: Vec<crate::model::Following>, request: u64, tx: Sender<Msg>) {
    thread::spawn(move || {
        for user in users {
            if let Some(avatar) = crate::cache::avatar(&user.face) {
                if tx
                    .send(Msg::FollowingAvatar(request, user.owner.mid, avatar))
                    .is_err()
                {
                    break;
                }
            }
        }
    });
}

pub fn load_favorites(mid: String, cookies: String, request: u64, force: bool, tx: Sender<Msg>) {
    thread::spawn(move || {
        cached_then_refresh(
            force,
            || api::favorites::folders(&mid, &cookies),
            |result| {
                let _ = tx.send(Msg::Favorites(request, result));
            },
        );
    });
}

pub fn load_history(
    cursor: Option<crate::model::HistoryCursor>,
    cookies: Option<String>,
    request: u64,
    force: bool,
    tx: Sender<Msg>,
) {
    thread::spawn(move || {
        cached_then_refresh(
            force || cursor.is_some(),
            || {
                cookies
                    .as_deref()
                    .ok_or_else(|| "未登录，请先登录".to_string())
                    .and_then(|cookies| api::history(cookies, cursor.as_ref()))
            },
            |result| {
                let _ = tx.send(Msg::History(request, cursor.is_none(), result));
            },
        );
    });
}

pub fn load_favorite_covers(
    folders: Vec<crate::model::FavoriteFolder>,
    request: u64,
    tx: Sender<Msg>,
) {
    thread::spawn(move || {
        for folder in folders {
            if let Some(image) = crate::cache::avatar(&folder.cover) {
                if tx
                    .send(Msg::FavoriteCover(request, folder.id, image))
                    .is_err()
                {
                    break;
                }
            }
        }
    });
}
