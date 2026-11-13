use std::fs::OpenOptions;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use std::rc::Rc;

use memmap2::MmapOptions;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, PhysicalSize, Rgb8Pixel, VecModel};

use crate::model;
use crate::player::{self, WIDTH, HEIGHT, VID_BYTES};
use crate::{tasks, util, AppWindow, Cmt, Msg};

const FB_BYTES: usize = (WIDTH as usize) * (HEIGHT as usize) * 2;

struct DemoPlatform {
    window: Rc<MinimalSoftwareWindow>,
}
impl Platform for DemoPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
}

/// 跑马灯的 yoyo 位移：0 → D → 0 无缝往返。
fn yoyo_shift(est: f64, clip: f64, t: f64) -> f32 {
    let d = est - clip;
    if d <= 1.0 { return 0.0; }
    let mt = (d / 40.0).max(0.8);
    let pause = 2.5;
    let per = (mt + pause) * 2.0;
    let ph = t % per;
    if ph < mt {
        (d * ph / mt) as f32
    } else if ph < mt + pause {
        d as f32
    } else if ph < mt * 2.0 + pause {
        (d * (1.0 - (ph - mt - pause) / mt)) as f32
    } else {
        0.0
    }
}

/// 估算详情滚动区的最大滚动量。
fn sync_thumb(ui: &AppWindow, scroll: f64, maxv: f64) {
    let vh = 84.0f64;
    if maxv <= 0.5 { ui.set_detail_thumb_h(0.0); ui.set_detail_thumb_y(0.0); return; }
    let h = (vh * (vh / (maxv + vh))).max(14.0);
    let y = (scroll / maxv) * (vh - h);
    ui.set_detail_thumb_h(h as f32);
    ui.set_detail_thumb_y(y as f32);
}

fn detail_max(body: &str) -> f64 {
    let mut lines = 0.0;
    for l in body.split('\n') {
        let n = l.chars().count() as f64;
        lines += (n / 33.0).ceil().max(1.0);
    }
    ((lines * 11.0) - 80.0).max(0.0)
}

pub fn run() {
    let fb_path = std::env::var("APPLAUNCH_LINUX_FBDEV_DEVICE")
        .or_else(|_| std::env::var("BILI_FB"))
        .unwrap_or_else(|_| "/dev/fb0".into());
    let kb_path = std::env::var("APPLAUNCH_LINUX_KEYBOARD_DEVICE")
        .or_else(|_| std::env::var("BILI_KBD"))
        .unwrap_or_else(|_| "/dev/input/by-path/platform-3f804000.i2c-event".into());

    let cpath = util::cookie_path();
    let mut cookies: Option<String> = std::fs::read_to_string(&cpath).ok()
        .map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut mid = cookies.as_deref().map(util::parse_mid).unwrap_or_default();

    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
    slint::platform::set_platform(Box::new(DemoPlatform { window: window.clone() })).expect("set_platform");

    let ui = AppWindow::new().expect("ui");
    ui.show().expect("show");

    let file = OpenOptions::new().read(true).write(true).open(&fb_path).expect("open fb");
    let mut fbmap = unsafe { MmapOptions::new().len(FB_BYTES).map_mut(&file).expect("mmap fb") };

    let (tx, rx) = mpsc::channel::<Msg>();
    let (ktx, krx) = mpsc::channel::<String>();
    util::spawn_keys(kb_path, ktx);

    ui.set_about_text(format!(
        "CardBili  v{}\nCardputerZero 哔哩哔哩客户端\n\n功能：推荐 / 热门 / 动态 / 我的\n      播放 · 扫码登录 · 视频详情\n\n内核：Rust + Slint 软件渲染\n屏幕：M5Stack 320x170\n\nESC 返回",
        env!("CARGO_PKG_VERSION")
    ).as_str().into());

    let ph = util::make_placeholder();
    let mut page: i32 = 0;
    let mut tab: usize = std::env::var("BILI_TAB").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(0).min(4);
    let mut view: i32 = std::env::var("BILI_VIEW").ok().and_then(|s| s.parse::<i32>().ok()).unwrap_or(1).clamp(0, 2);
    let mut help: bool = std::env::var("BILI_HELP").is_ok();
    let mut sel: i32 = std::env::var("BILI_SEL").ok().and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let mut lists: Vec<Vec<model::Item>> = vec![Vec::new(); 5];
    let mut covers: Vec<Vec<Option<slint::Image>>> = vec![Vec::new(); 5];
    let mut covers_started = [false; 5];
    let mut loaded = [false; 5];
    let mut pages = [1usize; 5];
    let mut has_more = [true; 5];
    let mut loading_more = [false; 5];
    let mut mine_detail: i32 = -1;
    let mut acct: Option<model::Account> = None;
    let mut avatar_img: Option<slint::Image> = None;
    let mut acct_loaded = false;
    let mut player: Option<player::Player> = None;
    let mut frames_in_seg: u64 = 0;
    let mut gen: u64 = 1;
    let mut status = String::from("加载中...");
    let mut loading = true;

    // 详情页
    let mut detail_img: Option<slint::Image> = None;
    let mut detail_bvid = String::new();
    let mut detail_scroll: f64 = 0.0;
    let mut detail_maxv: f64 = 0.0;

    // 跑马灯
    let mut marq_t: f64 = 0.0;
    let mut last_tick = Instant::now();

    tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone());

    if let Ok(b) = std::env::var("BILI_AUTOPLAY") {
        if !b.is_empty() { status = "正在加载视频...".into(); tasks::start_play(b, cookies.clone(), tx.clone()); }
    }
    if let Ok(b) = std::env::var("BILI_DETAIL") {
        if !b.is_empty() { detail_bvid = b.clone(); status = "正在加载详情...".into(); tasks::start_detail(b, cookies.clone(), tx.clone()); }
    }
    if std::env::var("BILI_ABOUT").is_ok() { page = 4; }
    if std::env::var("BILI_TEST_LOGIN").is_ok() {
        page = 2; view = 0; ui.set_has_qr(false); ui.set_qr_status("正在生成二维码...".into()); tasks::start_login(tx.clone());
    }
    let deadline = std::env::var("BILI_SECONDS").ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|s| Instant::now() + Duration::from_secs_f64(s));

    let mut buffer = vec![slint::platform::software_renderer::Rgb565Pixel(0); FB_BYTES / 2];
    let mut quit = false;
    while !quit {
        if let Some(d) = deadline { if Instant::now() >= d { quit = true; } }
        let mut latest: Option<Vec<u8>> = None;

        // --- 后台消息 ---
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Frame(g, b) => {
                    if page == 1 {
                        if let Some(p) = player.as_ref() {
                            if g == p.gen { frames_in_seg += 1; latest = Some(b); }
                        }
                    }
                }
                Msg::SegEnd(g) => {
                    if let Some(p) = player.as_mut() {
                        if g == p.gen {
                            if p.advance() { frames_in_seg = 0; } else { status = "播放结束，按 ESC 返回".into(); }
                        }
                    }
                }
                Msg::ListLoaded(t, items) => {
                    loaded[t] = true;
                    pages[t] = 1; has_more[t] = true; loading_more[t] = false;
                    let len = items.len();
                    if t < 3 {
                        covers[t] = vec![None; len];
                        covers_started[t] = true;
                        tasks::spawn_covers(t, items.clone(), 0, tx.clone());
                    }
                    lists[t] = items;
                    if t == tab || (tab == 3 && t == mine_detail as usize) {
                        sel = 0; loading = false;
                        status = if len == 0 { "暂无内容".to_string() } else { format!("{} 条", len) };
                        if let Ok(v) = std::env::var("BILI_SEL") { if let Ok(v) = v.parse::<i32>() { sel = v; } }
                    }
                }
                Msg::ListMore(t, items) => {
                    loading_more[t] = false;
                    if items.is_empty() {
                        has_more[t] = false;
                    } else {
                        let start = lists[t].len();
                        let mut add: Vec<model::Item> = Vec::new();
                        for it in items {
                            if !lists[t].iter().any(|e| e.bvid == it.bvid) && !add.iter().any(|e| e.bvid == it.bvid) {
                                add.push(it);
                            }
                        }
                        if add.is_empty() { has_more[t] = false; }
                        else {
                            pages[t] += 1;
                            lists[t].extend(add.clone());
                            covers[t].extend(vec![None; add.len()]);
                            if std::env::var("BILI_DUMP").is_ok() {
                                eprintln!("MORE tab={} page={} add={} total={}", t, pages[t], add.len(), lists[t].len());
                            }
                            tasks::spawn_covers(t, add, start, tx.clone());
                            if t == tab { status = format!("{} 条", lists[t].len()); }
                        }
                    }
                }
                Msg::Err(t, msg) => {
                    if t == 9 || t == 10 { status = msg; }
                    else if t == tab { loading = false; status = msg; }
                }
                Msg::PlayReady(info) => {
                    if let Some(mut old) = player.take() { old.stop(); }
                    gen += 1;
                    frames_in_seg = 0;
                    let p = player::Player::start(
                        info.segs.clone(), info.duration, info.title.clone(),
                        cookies.clone().unwrap_or_default(), gen, tx.clone(),
                    );
                    player = Some(p);
                    page = 1;
                    status.clear();
                }
                Msg::QrReady(url) => {
                    if let Some(img) = util::make_qr(&url) { ui.set_qr(img); ui.set_has_qr(true); }
                    ui.set_qr_status("请用 B 站 App 扫码".into());
                }
                Msg::QrStatus(s) => { ui.set_qr_status(s.as_str().into()); }
                Msg::QrDone(ck) => {
                    if !ck.is_empty() {
                        if let Some(dir) = std::path::Path::new(&cpath).parent() { let _ = std::fs::create_dir_all(dir); }
                        let _ = std::fs::write(&cpath, &ck);
                        mid = util::parse_mid(&ck);
                        cookies = Some(ck);
                        ui.set_qr_status("登录成功".into());
                        acct_loaded = false;
                        page = 0;
                        if tab == 3 {
                            loading = false;
                            status = "已登录".into();
                        } else {
                            loaded[tab] = false;
                            loading = true;
                            status = "已登录，刷新中...".into();
                            tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone());
                        }
                    }
                }
                Msg::Acct(a, raw) => {
                    if !raw.is_empty() {
                        if let Some(img) = util::decode_avatar(&raw) { avatar_img = Some(img); }
                    }
                    acct = Some(a);
                }
                Msg::Cover(t, i, w, h, raw) => {
                    if t < 5 && i < covers[t].len() {
                        covers[t][i] = Some(util::raw_to_image(w, h, &raw));
                    }
                }
                Msg::Detail(d, w, h, raw) => {
                    if !raw.is_empty() { detail_img = Some(util::raw_to_image(w, h, &raw)); }
                    else { detail_img = None; }
                    let desc = d.desc.trim();
                    let desc = if desc.is_empty() { "（无简介）".to_string() } else { desc.to_string() };
                    detail_maxv = 0.0;
                    detail_scroll = std::env::var("BILI_DSCROLL").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                    if std::env::var("BILI_DUMP").is_ok() { eprintln!("DETAIL comments={}", d.comments.len()); }
                    ui.set_detail_scroll(detail_scroll as f32);
                    sync_thumb(&ui, detail_scroll, detail_maxv);
                    ui.set_detail_owner(format!("UP主：{}", if d.owner.is_empty() { "未知" } else { d.owner.as_str() }).as_str().into());
                    ui.set_detail_desc(desc.as_str().into());
                    let cmts: Vec<Cmt> = d.comments.iter().map(|c| Cmt { user: c.user.clone().into(), text: c.text.clone().into() }).collect();
                    ui.set_comments(ModelRc::new(VecModel::from(cmts)));
                    ui.set_detail_title(d.title.as_str().into());
                    ui.set_detail_stats(format!(
                        "时长 {} · 播放 {} · 点赞 {} · 投币 {}",
                        model::mmss(d.duration), model::fmt_num(d.views), model::fmt_num(d.likes), model::fmt_num(d.coins)
                    ).as_str().into());
                    ui.set_has_detail_cover(detail_img.is_some());
                    if let Some(im) = detail_img.clone() { ui.set_detail_cover(im); }
                    page = 3;
                    status.clear();
                }
            }
        }

        // --- 键盘 ---
        while let Ok(k) = krx.try_recv() {
            if page == 0 {
                if k == "TAB" && !help {
                    tab = (tab + 1) % 4;
                    sel = 0; mine_detail = -1;
                    if tab == 3 {
                        loading = false; status.clear(); acct_loaded = false;
                    } else {
                        if loaded[tab] { loading = false; status = format!("{} 条", lists[tab].len()); }
                        else { loading = true; status = "加载中...".into(); tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone()); }
                        if loaded[tab] && !covers_started[tab] {
                            covers_started[tab] = true;
                            covers[tab] = vec![None; lists[tab].len()];
                            tasks::spawn_covers(tab, lists[tab].clone(), 0, tx.clone());
                        }
                    }
                    continue;
                }
                if help {
                    if matches!(k.as_str(), "7" | "ESC" | "BACKSPACE" | "q") { help = false; }
                } else if tab == 3 {
                    if mine_detail < 0 {
                        match k.as_str() {
                            "7" | "h" => { help = true; }
                            "UP" | "f" | "w" | "LEFT" | "z" => { sel = (sel - 1).max(0); }
                            "DOWN" | "x" | "s" | "RIGHT" | "c" => { if sel < 2 { sel += 1; } }
                            "ENTER" => {
                                if sel == 2 {
                                    page = 4;
                                } else {
                                    mine_detail = if sel == 0 { 3 } else { 4 };
                                    sel = 0;
                                    let src = mine_detail as usize;
                                    loaded[src] = false; loading = true; status = "加载中...".into();
                                    tasks::load_list(src, cookies.clone(), mid.clone(), tx.clone());
                                }
                            }
                            "r" | "4" => { acct_loaded = false; }
                            "l" | "5" => { page = 2; ui.set_has_qr(false); ui.set_qr_status("正在生成二维码...".into()); tasks::start_login(tx.clone()); }
                            "8" | "q" | "ESC" => { tab = 0; sel = 0; mine_detail = -1; }
                            _ => {}
                        }
                    } else {
                        let src = mine_detail as usize;
                        let n = lists[src].len() as i32;
                        match k.as_str() {
                            "UP" | "f" | "w" | "LEFT" | "z" => { sel = (sel - 1).max(0); }
                            "DOWN" | "x" | "s" | "RIGHT" | "c" => { if sel + 1 < n { sel += 1; } }
                            "ENTER" => {
                                if let Some(it) = lists[src].get(sel as usize) {
                                    if !it.bvid.is_empty() { status = "正在加载视频...".into(); tasks::start_play(it.bvid.clone(), cookies.clone(), tx.clone()); }
                                    else { status = "该条目无视频".into(); }
                                }
                            }
                            "SPACE" => {
                                if let Some(it) = lists[src].get(sel as usize) {
                                    if !it.bvid.is_empty() { detail_bvid = it.bvid.clone(); status = "正在加载详情...".into(); tasks::start_detail(it.bvid.clone(), cookies.clone(), tx.clone()); }
                                }
                            }
                            "r" | "4" => { loaded[src] = false; loading = true; status = "刷新中...".into(); tasks::load_list(src, cookies.clone(), mid.clone(), tx.clone()); }
                            "ESC" | "8" | "BACKSPACE" | "q" => { mine_detail = -1; sel = 0; }
                            _ => {}
                        }
                    }
                } else {
                    let n = lists[tab].len() as i32;
                    let cols: i32 = if view == 2 { 2 } else if view == 1 { 3 } else { 1 };
                    match k.as_str() {
                        "7" | "h" => { help = true; }
                        "v" => { view = (view + 1) % 3; }
                        "UP" | "f" | "w" => { let st = if view > 0 { cols } else { 1 }; sel = (sel - st).max(0); }
                        "DOWN" | "x" | "s" => { let st = if view > 0 { cols } else { 1 }; if sel + st < n { sel += st; } }
                        "LEFT" | "z" => { sel = (sel - 1).max(0); }
                        "RIGHT" | "c" => { if sel + 1 < n { sel += 1; } }
                        "ENTER" => {
                            if let Some(it) = lists[tab].get(sel as usize) {
                                if !it.bvid.is_empty() { status = "正在加载视频...".into(); tasks::start_play(it.bvid.clone(), cookies.clone(), tx.clone()); }
                                else { status = "该条目无视频".into(); }
                            }
                        }
                        "SPACE" => {
                            if let Some(it) = lists[tab].get(sel as usize) {
                                if !it.bvid.is_empty() { detail_bvid = it.bvid.clone(); status = "正在加载详情...".into(); tasks::start_detail(it.bvid.clone(), cookies.clone(), tx.clone()); }
                            }
                        }
                        "r" | "4" => { loaded[tab] = false; loading = true; status = "刷新中...".into(); tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone()); }
                        "l" | "5" => { page = 2; ui.set_has_qr(false); ui.set_qr_status("正在生成二维码...".into()); tasks::start_login(tx.clone()); }
                        "8" | "q" => { quit = true; }
                        _ => {}
                    }
                }
            } else if page == 1 {
                if let Some(p) = player.as_mut() {
                    match k.as_str() {
                        "SPACE" | "ENTER" => { p.pause_toggle(); }
                        "LEFT" | "z" => { let t = p.position(frames_in_seg) - 10.0; p.seek(t.max(0.0)); frames_in_seg = 0; }
                        "RIGHT" | "c" => { let t = p.position(frames_in_seg) + 10.0; p.seek(t); frames_in_seg = 0; }
                        "UP" | "f" | "w" => { p.vol_up(); }
                        "DOWN" | "x" | "s" => { p.vol_down(); }
                        "ESC" | "BACKSPACE" | "q" => { p.stop(); player = None; page = 0; frames_in_seg = 0; }
                        _ => {}
                    }
                } else if matches!(k.as_str(), "ESC" | "BACKSPACE" | "q") {
                    page = 0;
                }
            } else if page == 2 {
                match k.as_str() {
                    "ESC" | "BACKSPACE" | "q" | "8" => { page = 0; }
                    "r" | "4" => { ui.set_has_qr(false); ui.set_qr_status("正在刷新二维码...".into()); tasks::start_login(tx.clone()); }
                    _ => {}
                }
            } else if page == 3 {
                match k.as_str() {
                    "UP" | "f" | "w" => { detail_scroll = (detail_scroll - 22.0).max(0.0); ui.set_detail_scroll(detail_scroll as f32); sync_thumb(&ui, detail_scroll, detail_maxv); }
                    "DOWN" | "x" | "s" => { detail_scroll = (detail_scroll + 22.0).min(detail_maxv); ui.set_detail_scroll(detail_scroll as f32); sync_thumb(&ui, detail_scroll, detail_maxv); }
                    "ENTER" => {
                        if !detail_bvid.is_empty() { status = "正在加载视频...".into(); tasks::start_play(detail_bvid.clone(), cookies.clone(), tx.clone()); }
                    }
                    "ESC" | "BACKSPACE" | "q" | "8" => { page = 0; }
                    _ => {}
                }
            } else if page == 4 {
                match k.as_str() {
                    "ESC" | "BACKSPACE" | "q" | "8" | "ENTER" => { page = 0; }
                    _ => {}
                }
            }
        }

        // --- 应用最新帧 ---
        if let Some(bytes) = latest {
            if bytes.len() == VID_BYTES {
                let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(WIDTH, HEIGHT);
                {
                    let sl = pb.make_mut_slice();
                    let dst = unsafe { std::slice::from_raw_parts_mut(sl.as_mut_ptr() as *mut u8, VID_BYTES) };
                    dst.copy_from_slice(&bytes);
                }
                ui.set_frame(slint::Image::from_rgb8(pb));
                ui.set_has_frame(true);
            }
        }

        // --- 选中项夹取 ---
        if page == 0 {
            let len = if tab == 3 && mine_detail < 0 { 3 } else {
                lists[if tab == 3 { mine_detail as usize } else { tab }].len() as i32
            };
            if len > 0 && sel >= len { sel = len - 1; }
            if sel < 0 { sel = 0; }
            // 接近底部时自动加载下一页（仅 推荐/热门）
            if tab < 2 && mine_detail < 0 && view >= 0 && loaded[tab] && !loading_more[tab] && has_more[tab] {
                let n = lists[tab].len() as i32;
                let cols2: i32 = if view == 2 { 2 } else if view == 1 { 3 } else { 1 };
                if n > 0 && sel >= n - cols2 * 2 {
                    loading_more[tab] = true;
                    tasks::load_more(tab, (pages[tab] + 1) as u32, cookies.clone(), tx.clone());
                }
            }
        }

        // --- 账号信息 ---
        if tab == 3 && !acct_loaded {
            acct_loaded = true;
            if cookies.is_some() { tasks::fetch_account(cookies.clone(), tx.clone()); }
            else { acct = None; avatar_img = None; }
        }
        ui.set_acct_known(cookies.is_some());
        ui.set_acct_logged(acct.as_ref().map(|a| a.logged).unwrap_or(false));
        ui.set_acct_name(acct.as_ref().map(|a| a.name.clone()).unwrap_or_default().as_str().into());
        ui.set_acct_mid(acct.as_ref().map(|a| a.mid.clone()).unwrap_or_default().as_str().into());
        let lv = acct.as_ref().map(|a| a.level).unwrap_or(0);
        ui.set_acct_level(if lv > 0 { format!("Lv{}", lv) } else { String::new() }.as_str().into());
        ui.set_acct_vip(acct.as_ref().map(|a| a.vip.clone()).unwrap_or_default().as_str().into());
        ui.set_has_avatar(avatar_img.is_some());
        if let Some(im) = avatar_img.clone() { ui.set_avatar(im); }

        // --- 跑马灯计时 ---
        let now = Instant::now();
        marq_t += now.duration_since(last_tick).as_secs_f64();
        last_tick = now;

        // --- 详情页滚动尺寸 ---
        if page == 3 {
            let m = (ui.get_detail_content_h() as f64 - 84.0).max(0.0);
            if (m - detail_maxv).abs() > 0.5 { detail_maxv = m; }
            if std::env::var("BILI_DSEND").is_ok() { detail_scroll = detail_maxv; ui.set_detail_scroll(detail_scroll as f32); }
            if detail_scroll > detail_maxv { detail_scroll = detail_maxv; ui.set_detail_scroll(detail_scroll as f32); }
            sync_thumb(&ui, detail_scroll, detail_maxv);
        }

        // --- 同步 UI ---
        ui.set_page(page);
        ui.set_tab(tab as i32);
        ui.set_sel(sel);
        ui.set_status(status.as_str().into());
        ui.set_loading(loading);
        ui.set_view(view);
        ui.set_help(help);
        ui.set_mine_detail(mine_detail >= 0);

        // 列表标题跑马灯
        let mut list_marq = false;
        let mut list_shift = 0.0f32;
        if page == 0 && view == 0 {
            let sel_title: String = if tab == 3 {
                if mine_detail >= 0 { lists[mine_detail as usize].get(sel as usize).map(|it| it.title.clone()).unwrap_or_default() }
                else { String::new() }
            } else {
                lists[tab].get(sel as usize).map(|it| it.title.clone()).unwrap_or_default()
            };
            let est = util::est_w(&sel_title, 12.0);
            let clip = (WIDTH as f64) - 16.0;
            list_marq = est > clip;
            list_shift = yoyo_shift(est, clip, marq_t);
        }
        ui.set_title_marquee(list_marq);
        ui.set_title_shift(list_shift);

        if page == 0 {
            if tab == 3 {
                if mine_detail < 0 {
                    let titles: Vec<slint::SharedString> = vec!["历史".into(), "收藏".into(), "关于".into()];
                    let hist = if loaded[3] { format!("{} 条", lists[3].len()) } else { ">".to_string() };
                    let favm = if loaded[4] { format!("{} 条", lists[4].len()) } else { ">".to_string() };
                    let metas: Vec<slint::SharedString> = vec![hist.into(), favm.into(), "".into()];
                    ui.set_titles(slint::ModelRc::new(slint::VecModel::from(titles)));
                    ui.set_metas(slint::ModelRc::new(slint::VecModel::from(metas)));
                } else {
                    let src = mine_detail as usize;
                    let titles: Vec<slint::SharedString> = lists[src].iter().map(|it| it.title.as_str().into()).collect();
                    let metas: Vec<slint::SharedString> = lists[src].iter().map(|it| it.meta.as_str().into()).collect();
                    ui.set_titles(slint::ModelRc::new(slint::VecModel::from(titles)));
                    ui.set_metas(slint::ModelRc::new(slint::VecModel::from(metas)));
                }
            } else if view == 0 {
                let titles: Vec<slint::SharedString> = lists[tab].iter().map(|it| it.title.as_str().into()).collect();
                let metas: Vec<slint::SharedString> = lists[tab].iter().map(|it| it.meta.as_str().into()).collect();
                ui.set_titles(slint::ModelRc::new(slint::VecModel::from(titles)));
                ui.set_metas(slint::ModelRc::new(slint::VecModel::from(metas)));
            } else {
                let len = lists[tab].len();
                let cols: i32 = if view == 2 { 2 } else { 3 };
                let per = (cols * 2) as usize;
                let start = ((sel.max(0) as usize) / per) * per;
                let end = (start + per).min(len);
                let mut gt: Vec<slint::SharedString> = Vec::new();
                let mut gc: Vec<slint::Image> = Vec::new();
                let mut active: i32 = -1;
                for i in start..end {
                    gt.push(lists[tab][i].title.as_str().into());
                    let im = covers[tab].get(i).and_then(|o| o.clone()).unwrap_or_else(|| ph.clone());
                    gc.push(im);
                    if i as i32 == sel { active = (i - start) as i32; }
                }
                ui.set_cols(cols);
                ui.set_cell_w(if cols == 2 { 160.0 } else { 106.0 });
                ui.set_cell_h(58.0);
                ui.set_grid_titles(slint::ModelRc::new(slint::VecModel::from(gt)));
                ui.set_grid_covers(slint::ModelRc::new(slint::VecModel::from(gc)));
                ui.set_grid_active(active);
            }
        }
        if page == 1 {
            if let Some(p) = player.as_ref() {
                ui.set_ptitle(p.title.as_str().into());
                ui.set_paused(p.paused);
                let pos = p.position(frames_in_seg);
                let dur = p.duration;
                ui.set_progress(if dur > 0.0 { (pos / dur) as f32 } else { 0.0 });
                ui.set_time_text(format!("{} / {}", model::mmss(pos), model::mmss(dur)).as_str().into());
                let st = if p.paused { "已暂停  SPACE继续" } else { "Z/C 快退进  F/X 音量  SPACE暂停" };
                ui.set_play_state(st.into());

                let est = util::est_w(&p.title, 10.0);
                let clip = (WIDTH as f64) - 44.0;
                ui.set_ptitle_marquee(est > clip);
                ui.set_ptitle_shift(yoyo_shift(est, clip, marq_t));
            }
        }

        // --- 渲染 ---
        window.request_redraw();
        slint::platform::update_timers_and_animations();
        if window.draw_if_needed(|r| {
            let _ = r.render(&mut buffer, WIDTH as usize);
        }) {
            let out = unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u8, FB_BYTES) };
            fbmap[..FB_BYTES].copy_from_slice(out);
        }

        thread::sleep(Duration::from_millis(16));
    }

    if let Some(mut p) = player.take() { p.stop(); }
}
