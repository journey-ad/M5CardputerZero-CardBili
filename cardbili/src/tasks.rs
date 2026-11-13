use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use crate::{api, net, util, Msg};

pub fn load_list(tab: usize, cookies: Option<String>, mid: String, tx: Sender<Msg>) {
    thread::spawn(move || {
        let res = match tab {
            0 => net::fetch_mixin().and_then(|mk| api::rcmd(&mk, cookies.as_deref(), 1)),
            1 => api::popular(cookies.as_deref(), 1),
            2 => match cookies.as_deref() { Some(c) => api::dynamic(c), None => Err("未登录，请先登录".into()) },
            3 => match cookies.as_deref() { Some(c) => api::history(c), None => Err("未登录，请先登录".into()) },
            4 => match cookies.as_deref() { Some(c) => api::fav(c, &mid), None => Err("未登录，请先登录".into()) },
            _ => Err("?".into()),
        };
        match res {
            Ok(items) => {
                if std::env::var("BILI_DUMP").is_ok() {
                    for it in items.iter().take(6) { eprintln!("ITEM: {} | {}", it.title, it.meta); }
                }
                let _ = tx.send(Msg::ListLoaded(tab, items));
            }
            Err(e) => { let _ = tx.send(Msg::Err(tab, e)); }
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
        match api::video_detail(&bvid, cookies.as_deref()) {
            Ok(d) => {
                let mut w = 0u32;
                let mut h = 0u32;
                let mut raw = Vec::new();
                if !d.pic.is_empty() {
                    if let Some((cw, ch, cr)) = util::fetch_cover(&d.pic) { w = cw; h = ch; raw = cr; }
                }
                let _ = tx.send(Msg::Detail(d, w, h, raw));
            }
            Err(e) => { let _ = tx.send(Msg::Err(10, e)); }
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
