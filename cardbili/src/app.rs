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

/// 本机预览：把窗口按键映射成与真机 evdev 一致的按键字符串
#[cfg(feature = "desktop")]
fn desktop_key(k: minifb::Key) -> Option<&'static str> {
    use minifb::Key;
    Some(match k {
        Key::Left => "LEFT", Key::Right => "RIGHT", Key::Up => "UP", Key::Down => "DOWN",
        Key::Enter => "ENTER", Key::Space => "SPACE", Key::Escape => "ESC",
        Key::Backspace => "BACKSPACE", Key::Tab => "TAB",
        Key::Key1 => "1", Key::Key2 => "2", Key::Key3 => "3", Key::Key4 => "4",
        Key::Key5 => "5", Key::Key7 => "7",
        Key::Z => "z", Key::C => "c", Key::F => "f", Key::W => "w",
        Key::X => "x", Key::S => "s", Key::Q => "q",
        _ => return None,
    })
}

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
    #[cfg(not(feature = "desktop"))]
    let fb_path = std::env::var("APPLAUNCH_LINUX_FBDEV_DEVICE")
        .or_else(|_| std::env::var("BILI_FB"))
        .unwrap_or_else(|_| "/dev/fb0".into());
    #[cfg(not(feature = "desktop"))]
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

    #[cfg(not(feature = "desktop"))]
    let file = OpenOptions::new().read(true).write(true).open(&fb_path).expect("open fb");
    #[cfg(not(feature = "desktop"))]
    let mut fbmap = unsafe { MmapOptions::new().len(FB_BYTES).map_mut(&file).expect("mmap fb") };

    #[cfg(feature = "desktop")]
    let mut win = minifb::Window::new(
        "CardBili 320x170",
        WIDTH as usize,
        HEIGHT as usize,
        minifb::WindowOptions { scale: minifb::Scale::X2, resize: false, ..Default::default() },
    ).expect("创建预览窗口失败");
    #[cfg(feature = "desktop")]
    let mut dbuf: Vec<u32> = vec![0; (WIDTH * HEIGHT) as usize];

    crate::cache::init();

    let (tx, rx) = mpsc::channel::<Msg>();
    let (ktx, krx) = mpsc::channel::<String>();
    #[cfg(not(feature = "desktop"))]
    util::spawn_keys(kb_path, ktx);

    // 本机预览可用的按键脚本：BILI_KEYS="TAB,RIGHT,ENTER" 按 700ms 间隔依次注入
    #[cfg(feature = "desktop")]
    let scripted: Vec<String> = std::env::var("BILI_KEYS").ok()
        .map(|s| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    #[cfg(feature = "desktop")]
    let mut script_idx = 0usize;
    #[cfg(feature = "desktop")]
    let mut script_next = Instant::now() + Duration::from_millis(900);
    #[cfg(feature = "desktop")]
    let mut prev_keys: std::collections::HashSet<minifb::Key> = std::collections::HashSet::new();
    // 长按测试入口：BILI_HOLD=RIGHT，按下后保持 BILI_HOLD_MS 毫秒再松开
    #[cfg(feature = "desktop")]
    let hold_key = std::env::var("BILI_HOLD").ok();
    #[cfg(feature = "desktop")]
    let hold_ms = std::env::var("BILI_HOLD_MS").ok().and_then(|s| s.parse::<u64>().ok()).unwrap_or(1500);
    #[cfg(feature = "desktop")]
    let mut hold_started = false;
    #[cfg(feature = "desktop")]
    let mut hold_release: Option<Instant> = None;

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
    let mut status_detail = String::new();
    let mut loading = true;
    let mut prev_loading = false;

    // 详情页
    let mut detail_img: Option<slint::Image> = None;
    let mut detail_bvid = String::new();
    let mut detail_scroll: f64 = 0.0;
    let mut detail_maxv: f64 = 0.0;

    // 跑马灯
    let mut marq_t: f64 = 0.0;
    let mut last_tick = Instant::now();

    let force0 = std::env::var("BILI_FORCE").is_ok();
    tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), force0);

    if let Ok(b) = std::env::var("BILI_AUTOPLAY") {
        if !b.is_empty() { status = "正在加载视频...".into(); status_detail.clear(); tasks::start_play(b, cookies.clone(), tx.clone()); }
    }
    if let Ok(b) = std::env::var("BILI_DETAIL") {
        if !b.is_empty() { detail_bvid = b.clone(); status = "正在加载详情...".into(); status_detail.clear(); tasks::start_detail(b, cookies.clone(), tx.clone()); }
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
    let mut seek_pending: Option<f64> = None;
    let mut seek_deadline: Option<Instant> = None;
    let mut esc_press: Option<Instant> = None;
    let mut bar_until: Option<Instant> = None;
    let mut last_input = Instant::now();
    let mut left_hold: Option<Instant> = None;
    let mut right_hold: Option<Instant> = None;
    let mut left_rep = Instant::now();
    let mut right_rep = Instant::now();
    while !quit {
        #[cfg(feature = "desktop")]
        if !win.is_open() { quit = true; }
        if let Some(d) = deadline { if Instant::now() >= d { quit = true; } }
        // 左右调整结束（停止按键 300ms）后才真正跳转，此时展示加载中
        if let Some(dl) = seek_deadline {
            if Instant::now() >= dl {
                seek_deadline = None;
                if let (Some(pp), Some(t)) = (player.as_mut(), seek_pending.take()) {
                    pp.seek(t); frames_in_seg = 0;
                    ui.set_status("加载中…".into());
                    ui.set_has_frame(false);
                }
            }
        }
        // 长按左右：按住超过 350ms 后每 120ms 连续调整 2 秒，松开后按现有防抖落地
        if page == 1 {
            if let Some(p) = player.as_mut() {
                let mut step = 0.0f64;
                if let Some(start) = left_hold {
                    if start.elapsed() >= Duration::from_millis(350) && left_rep.elapsed() >= Duration::from_millis(120) {
                        left_rep = Instant::now();
                        step = -2.0;
                    }
                }
                if let Some(start) = right_hold {
                    if start.elapsed() >= Duration::from_millis(350) && right_rep.elapsed() >= Duration::from_millis(120) {
                        right_rep = Instant::now();
                        step = 2.0;
                    }
                }
                if step != 0.0 {
                    let base = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
                    let t = (base + step).clamp(0.0, p.duration.max(0.0));
                    ui.set_progress(if p.duration > 0.0 { (t / p.duration) as f32 } else { 0.0 });
                    ui.set_time_text(format!("{} / {}", model::mmss(t), model::mmss(p.duration)).as_str().into());
                    seek_pending = Some(t);
                    seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                }
            }
        }
        if page == 0 && tab < 3 {
            if let Some(t) = esc_press {
                if t.elapsed() >= Duration::from_millis(2000) { quit = true; }
            }
        }
        if let Some(t) = bar_until {
            if Instant::now() >= t {
                ui.set_bar("".into());
                ui.set_bar_sticky(false);
                bar_until = None;
            }
        }
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
                            if p.advance() { frames_in_seg = 0; } else { status = "播放结束，按 ESC 返回".into(); status_detail.clear(); }
                        }
                    }
                }
                Msg::ListLoaded(t, items) => {
                    let first = !loaded[t];
                    // 内容与已展示的一致时保留封面与选中，避免 SWR 更新时闪烁
                    let same_ids = !first
                        && lists[t].len() == items.len()
                        && lists[t].iter().zip(items.iter()).all(|(a, b)| a.bvid == b.bvid);
                    loaded[t] = true;
                    pages[t] = 1; has_more[t] = true; loading_more[t] = false;
                    let len = items.len();
                    if t < 3 {
                        if same_ids {
                            if covers[t].len() != len {
                                let mut newc = vec![None; len];
                                for i in 0..covers[t].len().min(len) { newc[i] = covers[t][i].take(); }
                                covers[t] = newc;
                            }
                        } else {
                            covers[t] = vec![None; len];
                        }
                        covers_started[t] = true;
                        let have: Vec<bool> = covers[t].iter().map(|c| c.is_some()).collect();
                        if have.iter().any(|b| !b) {
                            tasks::spawn_covers_missing(t, items.clone(), have, tx.clone());
                        }
                    }
                    lists[t] = items;
                    if t == tab || (tab == 3 && t == mine_detail as usize) {
                        if first {
                            sel = 0;
                        } else if sel >= len as i32 {
                            sel = if len == 0 { 0 } else { len as i32 - 1 };
                        }
                        loading = false;
                        if page == 0 { status = if len == 0 { "暂无内容".to_string() } else { format!("{} 条", len) }; } status_detail.clear();
                        ui.set_bar(if len == 0 { String::new().into() } else { format!("已获取到 {} 条数据", len).into() });
                        ui.set_bar_sticky(false);
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
                            if t == tab && page == 0 { status = format!("{} 条", lists[t].len()); status_detail.clear(); ui.set_bar(format!("已获取到 {} 条数据", lists[t].len()).into()); ui.set_bar_sticky(false); }
                        }
                    }
                }
                Msg::Err(t, msg) => {
                    let (friendly, tech) = match msg.split_once('\n') {
                        Some((a, b)) => (a.to_string(), b.to_string()),
                        None => (msg, String::new()),
                    };
                    if t == 9 || t == 10 { status = friendly; status_detail = tech; }
                    else if t == tab { loading = false; if page == 0 { status = friendly; status_detail = tech; ui.set_bar("加载失败".into()); ui.set_bar_sticky(false); } }
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
                    seek_pending = None; seek_deadline = None;
                    ui.set_has_frame(false);
                    status = "加载中…".into(); status_detail.clear();
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
                            status = "已登录".into(); status_detail.clear();
                        } else {
                            loaded[tab] = false;
                            loading = true;
                            status = "已登录，刷新中...".into(); status_detail.clear();
                            tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), false);
                        }
                    }
                }
                Msg::Acct(a, raw) => {
                    if !raw.is_empty() {
                        if let Some(img) = util::decode_avatar(&raw) { avatar_img = Some(img); }
                    }
                    acct = Some(a);
                }
                Msg::Cover(t, bvid, w, h, raw) => {
                    if t < 5 {
                        if let Some(idx) = lists[t].iter().position(|it| it.bvid == bvid) {
                            if idx < covers[t].len() { covers[t][idx] = Some(util::raw_to_image(w, h, &raw)); }
                        }
                    }
                }
                Msg::Detail(d, w, h, raw) => {
                    let first = page != 3;
                    if !raw.is_empty() { detail_img = Some(util::raw_to_image(w, h, &raw)); }
                    else { detail_img = None; }
                    let desc = d.desc.trim();
                    let desc = if desc.is_empty() { "（无简介）".to_string() } else { desc.to_string() };
                    if first {
                        detail_maxv = 0.0;
                        detail_scroll = std::env::var("BILI_DSCROLL").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                    }
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
                    if first { page = 3; status.clear(); status_detail.clear(); }
                }
            }
        }

        // --- 键盘 ---
        #[cfg(feature = "desktop")]
        if script_idx < scripted.len() && Instant::now() >= script_next {
            let _ = ktx.send(scripted[script_idx].clone());
            script_idx += 1;
            script_next = Instant::now() + Duration::from_millis(700);
        }
        #[cfg(feature = "desktop")]
        if let Some(hk) = hold_key.clone() {
            if !hold_started && scripted.is_empty() && Instant::now() >= script_next {
                let _ = ktx.send(hk);
                hold_started = true;
                hold_release = Some(Instant::now() + Duration::from_millis(hold_ms));
            }
        }
        #[cfg(feature = "desktop")]
        if let Some(rt) = hold_release {
            if Instant::now() >= rt {
                if let Some(hk) = &hold_key { let _ = ktx.send(format!("{}_UP", hk)); }
                hold_release = None;
            }
        }
        #[cfg(feature = "desktop")]
        {
            let cur: std::collections::HashSet<minifb::Key> = win.get_keys().into_iter().collect();
            for key in cur.iter() {
                if !prev_keys.contains(key) {
                    if let Some(s) = desktop_key(*key) { let _ = ktx.send(s.to_string()); }
                }
            }
            for key in prev_keys.iter() {
                if !cur.contains(key) {
                    if let Some(s) = desktop_key(*key) {
                        if s == "LEFT" || s == "RIGHT" { let _ = ktx.send(format!("{}_UP", s)); }
                    }
                }
            }
            prev_keys = cur;
        }
        while let Ok(k) = krx.try_recv() {
            last_input = Instant::now();
            if k == "LEFT_UP" { left_hold = None; continue; }
            if k == "RIGHT_UP" { right_hold = None; continue; }
            if page == 0 {
                if tab < 3 && !help && (k == "ESC_DOWN" || k == "ESC_UP") {
                    if k == "ESC_DOWN" {
                        esc_press = Some(Instant::now());
                        ui.set_bar("长按 2 秒退出".into());
                        ui.set_bar_sticky(true);
                        bar_until = None;
                    } else {
                        esc_press = None;
                        bar_until = Some(Instant::now() + Duration::from_millis(3000));
                    }
                    continue;
                }
                if k == "TAB" && !help {
                    tab = (tab + 1) % 4;
                    sel = 0; mine_detail = -1;
                    if tab == 3 {
                        loading = false; status.clear(); status_detail.clear(); acct_loaded = false;
                    } else {
                        if loaded[tab] { loading = false; status = format!("{} 条", lists[tab].len()); status_detail.clear(); }
                        else { loading = true; status = "加载中...".into(); status_detail.clear(); tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), false); }
                        if loaded[tab] && !covers_started[tab] {
                            covers_started[tab] = true;
                            covers[tab] = vec![None; lists[tab].len()];
                            tasks::spawn_covers(tab, lists[tab].clone(), 0, tx.clone());
                        }
                    }
                    continue;
                }
                if help {
                    if matches!(k.as_str(), "7" | "ESC" | "BACKSPACE") { help = false; }
                } else if tab == 3 {
                    if mine_detail < 0 {
                        match k.as_str() {
                            "7" | "8" | "h" => { help = true; }
                            "UP" | "f" | "w" | "LEFT" | "z" => { sel = (sel - 1).max(0); }
                            "DOWN" | "x" | "s" | "RIGHT" | "c" => { if sel < 2 { sel += 1; } }
                            "ENTER" => {
                                if sel == 2 {
                                    page = 4;
                                } else {
                                    mine_detail = if sel == 0 { 3 } else { 4 };
                                    sel = 0;
                                    let src = mine_detail as usize;
                                    loaded[src] = false; loading = true; status = "加载中...".into(); status_detail.clear();
                                    tasks::load_list(src, cookies.clone(), mid.clone(), tx.clone(), false);
                                }
                            }
                            "r" | "4" => { acct_loaded = false; }
                            "l" | "5" => { page = 2; ui.set_has_qr(false); ui.set_qr_status("正在生成二维码...".into()); tasks::start_login(tx.clone()); }
                            "ESC" | "BACKSPACE" => { tab = 0; sel = 0; mine_detail = -1; }
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
                                    if !it.bvid.is_empty() { status = "正在加载视频...".into(); status_detail.clear(); tasks::start_play(it.bvid.clone(), cookies.clone(), tx.clone()); }
                                    else { status = "该条目无视频".into(); status_detail.clear(); }
                                }
                            }
                            "SPACE" => {
                                if let Some(it) = lists[src].get(sel as usize) {
                                    if !it.bvid.is_empty() { detail_bvid = it.bvid.clone(); status = "正在加载详情...".into(); status_detail.clear(); tasks::start_detail(it.bvid.clone(), cookies.clone(), tx.clone()); }
                                }
                            }
                            "r" | "4" => { loaded[src] = false; loading = true; status = "刷新中...".into(); status_detail.clear(); tasks::load_list(src, cookies.clone(), mid.clone(), tx.clone(), true); }
                            "ESC" | "BACKSPACE" => { mine_detail = -1; sel = 0; }
                            _ => {}
                        }
                    }
                } else {
                    let n = lists[tab].len() as i32;
                    let cols: i32 = if view == 2 { 2 } else if view == 1 { 3 } else { 1 };
                    match k.as_str() {
                        "7" | "8" | "h" => { help = true; }
                        "v" => { view = (view + 1) % 3; }
                        "UP" | "f" | "w" => { let st = if view > 0 { cols } else { 1 }; sel = (sel - st).max(0); }
                        "DOWN" | "x" | "s" => { let st = if view > 0 { cols } else { 1 }; if sel + st < n { sel += st; } }
                        "LEFT" | "z" => { sel = (sel - 1).max(0); }
                        "RIGHT" | "c" => { if sel + 1 < n { sel += 1; } }
                        "ENTER" => {
                            if let Some(it) = lists[tab].get(sel as usize) {
                                if !it.bvid.is_empty() { status = "正在加载视频...".into(); status_detail.clear(); tasks::start_play(it.bvid.clone(), cookies.clone(), tx.clone()); }
                                else { status = "该条目无视频".into(); status_detail.clear(); }
                            }
                        }
                        "SPACE" => {
                            if let Some(it) = lists[tab].get(sel as usize) {
                                if !it.bvid.is_empty() { detail_bvid = it.bvid.clone(); status = "正在加载详情...".into(); status_detail.clear(); tasks::start_detail(it.bvid.clone(), cookies.clone(), tx.clone()); }
                            }
                        }
                        "r" | "4" => { loaded[tab] = false; loading = true; status = "刷新中...".into(); status_detail.clear(); tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), true); }
                        "l" | "5" => { page = 2; ui.set_has_qr(false); ui.set_qr_status("正在生成二维码...".into()); tasks::start_login(tx.clone()); }
                        "ESC_HOLD" => { quit = true; }
                        _ => {}
                    }
                }
            } else if page == 1 {
                if let Some(p) = player.as_mut() {
                    match k.as_str() {
                        "SPACE" | "ENTER" => { p.pause_toggle(); }
                        "LEFT" | "z" => {
                            left_hold = Some(Instant::now());
                            left_rep = Instant::now();
                            let base = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
                            let t = (base - 10.0).max(0.0);
                            ui.set_progress(if p.duration > 0.0 { (t / p.duration) as f32 } else { 0.0 });
                            ui.set_time_text(format!("{} / {}", model::mmss(t), model::mmss(p.duration)).as_str().into());
                            seek_pending = Some(t);
                            seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                        }
                        "RIGHT" | "c" => {
                            right_hold = Some(Instant::now());
                            right_rep = Instant::now();
                            let base = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
                            let t = (base + 10.0).min(p.duration.max(0.0));
                            ui.set_progress(if p.duration > 0.0 { (t / p.duration) as f32 } else { 0.0 });
                            ui.set_time_text(format!("{} / {}", model::mmss(t), model::mmss(p.duration)).as_str().into());
                            seek_pending = Some(t);
                            seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                        }
                        "UP" | "f" | "w" => { p.vol_up(); }
                        "DOWN" | "x" | "s" => { p.vol_down(); }
                        "ESC" | "BACKSPACE" => { p.stop(); player = None; page = 0; frames_in_seg = 0; seek_pending = None; seek_deadline = None; ui.set_has_frame(false); }
                        _ => {}
                    }
                } else if matches!(k.as_str(), "ESC" | "BACKSPACE") {
                    page = 0;
                    ui.set_has_frame(false);
                }
            } else if page == 2 {
                match k.as_str() {
                    "ESC" | "BACKSPACE" => { page = 0; }
                    "r" | "4" => { ui.set_has_qr(false); ui.set_qr_status("正在刷新二维码...".into()); tasks::start_login(tx.clone()); }
                    _ => {}
                }
            } else if page == 3 {
                match k.as_str() {
                    "UP" | "f" | "w" => { detail_scroll = (detail_scroll - 22.0).max(0.0); ui.set_detail_scroll(detail_scroll as f32); sync_thumb(&ui, detail_scroll, detail_maxv); }
                    "DOWN" | "x" | "s" => { detail_scroll = (detail_scroll + 22.0).min(detail_maxv); ui.set_detail_scroll(detail_scroll as f32); sync_thumb(&ui, detail_scroll, detail_maxv); }
                    "ENTER" => {
                        if !detail_bvid.is_empty() { status = "正在加载视频...".into(); status_detail.clear(); tasks::start_play(detail_bvid.clone(), cookies.clone(), tx.clone()); }
                    }
                    "ESC" | "BACKSPACE" => { page = 0; }
                    _ => {}
                }
            } else if page == 4 {
                match k.as_str() {
                    "ESC" | "BACKSPACE" | "ENTER" => { page = 0; }
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
        // 播放中 3 秒无操作隐藏控件；暂停或正在长按调进度时保持显示
        let controls_show = if page == 1 {
            player.as_ref().map(|p| p.paused || left_hold.is_some() || right_hold.is_some()
                || last_input.elapsed() < Duration::from_millis(3000)).unwrap_or(true)
        } else {
            true
        };
        ui.set_controls_visible(controls_show);
        ui.set_page(page);
        ui.set_tab(tab as i32);
        ui.set_sel(sel);
        ui.set_status(status.as_str().into());
        ui.set_status_detail(status_detail.as_str().into());
        if loading != prev_loading {
            if loading { ui.set_bar("加载中…".into()); ui.set_bar_sticky(true); }
            prev_loading = loading;
        }
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
                // 有未落地的跳转时用预览位置，避免每帧被实际播放位置覆盖
                let pos = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
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
            #[cfg(not(feature = "desktop"))]
            {
                let out = unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u8, FB_BYTES) };
                fbmap[..FB_BYTES].copy_from_slice(out);
            }
            #[cfg(feature = "desktop")]
            {
                for (i, px) in buffer.iter().enumerate() {
                    let v = px.0;
                    let r = ((v >> 11) & 0x1f) as u32 * 255 / 31;
                    let g = ((v >> 5) & 0x3f) as u32 * 255 / 63;
                    let b = (v & 0x1f) as u32 * 255 / 31;
                    dbuf[i] = (r << 16) | (g << 8) | b;
                }
                if win.update_with_buffer(&dbuf, WIDTH as usize, HEIGHT as usize).is_err() { quit = true; }
            }
        }

        thread::sleep(Duration::from_millis(16));
    }

    if let Some(mut p) = player.take() { p.stop(); }
}
