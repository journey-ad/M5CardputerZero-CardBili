use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use crate::{api, net, util, Msg};

pub fn load_list(tab: usize, cookies: Option<String>, mid: String, tx: Sender<Msg>, force: bool) {
    thread::spawn(move || {
        let fetch = |cookies: &Option<String>, mid: &String| -> Result<Vec<crate::model::Item>, String> {
            match tab {
                0 => net::fetch_mixin().and_then(|mk| api::rcmd(&mk, cookies.as_deref(), 1)),
                1 => api::popular(cookies.as_deref(), 1),
                2 => match cookies.as_deref() { Some(c) => api::dynamic(c), None => Err("未登录，请先登录".into()) },
                3 => match cookies.as_deref() { Some(c) => api::history(c), None => Err("未登录，请先登录".into()) },
                4 => match cookies.as_deref() { Some(c) => api::fav(c, mid), None => Err("未登录，请先登录".into()) },
                _ => Err("?".into()),
            }
        };
        // SWR 阶段1：有本地缓存先展示（手动刷新时跳过）
        let mut showed = false;
        if !force {
            crate::cache::set_cached_only(true);
            let cached = fetch(&cookies, &mid);
            crate::cache::set_cached_only(false);
            if let Ok(items) = cached {
                if !items.is_empty() {
                    if std::env::var("BILI_DUMP").is_ok() {
                        for it in items.iter().take(6) { eprintln!("CACHE ITEM: {} | {}", it.title, it.meta); }
                    }
                    let _ = tx.send(Msg::ListLoaded(tab, items));
                    showed = true;
                }
            }
        }
        // SWR 阶段2：后台请求最新，成功则覆盖界面
        crate::cache::set_force(true);
        match fetch(&cookies, &mid) {
            Ok(items) => {
                if std::env::var("BILI_DUMP").is_ok() {
                    for it in items.iter().take(6) { eprintln!("ITEM: {} | {}", it.title, it.meta); }
                }
                let _ = tx.send(Msg::ListLoaded(tab, items));
            }
            Err(e) => {
                // 已展示缓存时，后台失败静默，保留旧数据
                if !showed { let _ = tx.send(Msg::Err(tab, e)); }
            }
        }
    });
}

pub fn fetch_account(cookies: Option<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        match api::account(cookies.as_deref()) {
            Ok(a) => {
                let mut face = a.face.clone();
                if face.starts_with("//") { face = format!("https:{}", face); }
                let raw = if face.is_empty() { Vec::new() } else { net::curl_bytes(&face).unwrap_or_default() };
                let _ = tx.send(Msg::Acct(a, raw));
            }
            Err(e) => { let _ = tx.send(Msg::Err(9, e)); }
        }
    });
}

pub fn spawn_covers(tab: usize, items: Vec<crate::model::Item>, start: usize, tx: Sender<Msg>) {
    thread::spawn(move || {
        for (i, it) in items.iter().enumerate() {
            if it.pic.is_empty() { continue; }
            if let Some((w, h, raw)) = util::fetch_cover(&it.pic) {
                if tx.send(Msg::Cover(tab, start + i, w, h, raw)).is_err() { break; }
            }
        }
    });
}

// 仅补齐缺失的封面（SWR 更新时保留已加载项，避免整列表闪烁）
pub fn spawn_covers_missing(tab: usize, items: Vec<crate::model::Item>, have: Vec<bool>, tx: Sender<Msg>) {
    thread::spawn(move || {
        for (i, it) in items.iter().enumerate() {
            if have.get(i).copied().unwrap_or(false) { continue; }
            if it.pic.is_empty() { continue; }
            if let Some((w, h, raw)) = util::fetch_cover(&it.pic) {
                if tx.send(Msg::Cover(tab, i, w, h, raw)).is_err() { break; }
            }
        }
    });
}

pub fn load_more(tab: usize, page: u32, cookies: Option<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let res = match tab {
            0 => net::fetch_mixin().and_then(|mk| api::rcmd(&mk, cookies.as_deref(), page)),
            1 => api::popular(cookies.as_deref(), page),
            _ => Ok(Vec::new()),
        };
        match res {
            Ok(items) => { let _ = tx.send(Msg::ListMore(tab, items)); }
            Err(e) => { let _ = tx.send(Msg::Err(tab, e)); }
        }
    });
}

pub fn start_play(bvid: String, cookies: Option<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        match api::play_info(&bvid, cookies.as_deref()) {
            Ok(p) => { let _ = tx.send(Msg::PlayReady(p)); }
            Err(e) => { let _ = tx.send(Msg::Err(9, e)); }
        }
    });
}

pub fn start_detail(bvid: String, cookies: Option<String>, tx: Sender<Msg>) {
    thread::spawn(move || {
        let build = |d: crate::model::Detail| -> (crate::model::Detail, u32, u32, Vec<u8>) {
            let mut w = 0u32;
            let mut h = 0u32;
            let mut raw = Vec::new();
            if !d.pic.is_empty() {
                if let Some((cw, ch, cr)) = util::fetch_cover(&d.pic) { w = cw; h = ch; raw = cr; }
            }
            (d, w, h, raw)
        };
        // SWR 阶段1：缓存详情先展示
        let mut showed = false;
        crate::cache::set_cached_only(true);
        let cached = api::video_detail(&bvid, cookies.as_deref());
        crate::cache::set_cached_only(false);
        if let Ok(d) = cached {
            let (d, w, h, raw) = build(d);
            let _ = tx.send(Msg::Detail(d, w, h, raw));
            showed = true;
        }
        // SWR 阶段2：拉最新（播放量等）
        crate::cache::set_force(true);
        match api::video_detail(&bvid, cookies.as_deref()) {
            Ok(d) => { let (d, w, h, raw) = build(d); let _ = tx.send(Msg::Detail(d, w, h, raw)); }
            Err(e) => { if !showed { let _ = tx.send(Msg::Err(10, e)); } }
        }
    });
}

pub fn start_login(tx: Sender<Msg>) {
    thread::spawn(move || {
        let jar = "/tmp/cardbili_qr.jar";
        let _ = std::fs::remove_file(jar);
        let (url, key) = match net::qr_generate() {
            Ok(v) => v,
            Err(e) => { let _ = tx.send(Msg::QrStatus(format!("生成二维码失败: {}", e))); return; }
        };
        let _ = tx.send(Msg::QrReady(url));
        loop {
            match net::qr_poll(&key, jar) {
                net::Poll::Success(ck) => { let _ = tx.send(Msg::QrDone(ck)); break; }
                net::Poll::Scanned => { let _ = tx.send(Msg::QrStatus("已扫码，请在手机上确认".into())); }
                net::Poll::Pending => {}
                net::Poll::Expired => { let _ = tx.send(Msg::QrStatus("二维码已过期，请刷新".into())); break; }
                net::Poll::Err(e) => { let _ = tx.send(Msg::QrStatus(format!("登录出错: {}", e))); break; }
            }
            thread::sleep(Duration::from_secs(2));
        }
    });
}
