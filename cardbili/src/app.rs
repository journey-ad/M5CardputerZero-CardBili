use std::collections::{HashMap, HashSet};
#[cfg(not(feature = "desktop"))]
use std::fs::OpenOptions;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(not(feature = "desktop"))]
use memmap2::MmapOptions;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, ModelRc, PhysicalSize, Rgb8Pixel, VecModel};

use crate::danmaku;
use crate::model;
use crate::player::{self, HEIGHT, VID_BYTES, WIDTH};
use crate::settings::Settings;
use crate::{tasks, util, AppWindow, Cmt, Msg};

const FB_BYTES: usize = (WIDTH as usize) * (HEIGHT as usize) * 2;

// 弹幕描边位图按文本和颜色缓存
struct DmCache {
    map: HashMap<u64, slint::Image>,
}

impl DmCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn image(&mut self, text: &str, color: u32) -> Option<slint::Image> {
        let mut key = 0xcbf2_9ce4_8422_2325u64;
        for b in text.as_bytes() {
            key ^= *b as u64;
            key = key.wrapping_mul(0x100_0000_01b3);
        }
        key ^= (color as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        if let Some(img) = self.map.get(&key) {
            return Some(img.clone());
        }
        let bmp = danmaku::bake(text, color)?;
        let mut pb = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(bmp.w, bmp.h);
        pb.make_mut_bytes().copy_from_slice(&bmp.px);
        let img = slint::Image::from_rgba8(pb);
        if self.map.len() >= 256 {
            self.map.clear();
        }
        self.map.insert(key, img.clone());
        Some(img)
    }
}

// 把引擎算出的弹幕行写进 Slint 模型
fn render_danmaku(ui: &AppWindow, engine: &danmaku::Engine, t: f64, cache: &mut DmCache) {
    let baked = danmaku::font_available();
    ui.set_dm_baked(baked);
    let mut rows: Vec<crate::DmRow> = Vec::new();
    for r in engine.rows_at(t) {
        // 字体可用时生成描边位图，其余情况使用文字和半透明背景
        let img = if baked {
            cache.image(r.text, r.color)
        } else {
            None
        };
        let use_img = img.is_some();
        rows.push(crate::DmRow {
            text: if use_img { "".into() } else { r.text.into() },
            x: r.x,
            y: r.y,
            w: r.w,
            color: slint::Color::from_rgb_u8(
                ((r.color >> 16) & 0xff) as u8,
                ((r.color >> 8) & 0xff) as u8,
                (r.color & 0xff) as u8,
            ),
            img: img.unwrap_or_default(),
        });
    }
    ui.set_dm_rows(ModelRc::new(VecModel::from(rows)));
}

/// 本机预览：把窗口按键映射成与真机 evdev 一致的按键字符串
#[cfg(feature = "desktop")]
fn desktop_key(k: minifb::Key) -> Option<&'static str> {
    use minifb::Key;
    Some(match k {
        Key::Left => "LEFT",
        Key::Right => "RIGHT",
        Key::Up => "UP",
        Key::Down => "DOWN",
        Key::Enter => "ENTER",
        Key::Space => "SPACE",
        Key::Escape => "ESC",
        Key::Backspace => "BACKSPACE",
        Key::Tab => "TAB",
        Key::Key1 => "1",
        Key::Key2 => "2",
        Key::Key3 => "3",
        Key::Key4 => "4",
        Key::Key5 => "5",
        Key::Key7 => "7",
        Key::Z => "z",
        Key::C => "c",
        Key::F => "f",
        Key::W => "w",
        Key::X => "x",
        Key::S => "s",
        Key::Q => "q",
        Key::V => "v",
        Key::R => "r",
        Key::L => "l",
        Key::H => "h",
        Key::Key8 => "8",
        _ => return None,
    })
}

struct AppPlatform {
    window: Rc<MinimalSoftwareWindow>,
}
impl Platform for AppPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
}

/// 计算跑马灯在起点与终点之间的往返位移
fn yoyo_shift(est: f64, clip: f64, t: f64) -> f32 {
    let d = est - clip;
    if d <= 1.0 {
        return 0.0;
    }
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

/// 根据可视高度与滚动范围更新详情页滚动条
fn sync_thumb(ui: &AppWindow, scroll: f64, maxv: f64) {
    let vh = 84.0f64;
    if maxv <= 0.5 {
        ui.set_detail_thumb_h(0.0);
        ui.set_detail_thumb_y(0.0);
        return;
    }
    let h = (vh * (vh / (maxv + vh))).max(14.0);
    let y = (scroll / maxv) * (vh - h);
    ui.set_detail_thumb_h(h as f32);
    ui.set_detail_thumb_y(y as f32);
}

fn sync_list(ui: &AppWindow, items: &[model::Item]) {
    let titles: Vec<slint::SharedString> = items
        .iter()
        .map(|item| item.title.as_str().into())
        .collect();
    let metas: Vec<slint::SharedString> =
        items.iter().map(|item| item.meta.as_str().into()).collect();
    ui.set_titles(ModelRc::new(VecModel::from(titles)));
    ui.set_metas(ModelRc::new(VecModel::from(metas)));
}

fn list_source(tab: usize, mine_detail: i32) -> Option<usize> {
    match tab {
        0..=2 => Some(tab),
        3 if mine_detail >= 0 => Some(mine_detail as usize),
        _ => None,
    }
}

fn show_detail_loading(ui: &AppWindow, title: &str) {
    ui.set_detail_loading(true);
    ui.set_detail_error("".into());
    ui.set_detail_title(title.into());
    ui.set_detail_stats("".into());
    ui.set_detail_desc("".into());
    ui.set_detail_owner("".into());
    ui.set_has_detail_cover(false);
    ui.set_detail_scroll(0.0);
    ui.set_detail_thumb_h(0.0);
    ui.set_comments(ModelRc::new(VecModel::from(Vec::<Cmt>::new())));
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
    let mut cookies: Option<String> = std::fs::read_to_string(&cpath)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let mut mid = cookies.as_deref().map(util::parse_mid).unwrap_or_default();

    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(PhysicalSize::new(WIDTH, HEIGHT));
    slint::platform::set_platform(Box::new(AppPlatform {
        window: window.clone(),
    }))
    .expect("set_platform");

    let ui = AppWindow::new().expect("ui");
    ui.show().expect("show");

    #[cfg(not(feature = "desktop"))]
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fb_path)
        .expect("open fb");
    #[cfg(not(feature = "desktop"))]
    let mut fbmap = unsafe {
        MmapOptions::new()
            .len(FB_BYTES)
            .map_mut(&file)
            .expect("mmap fb")
    };

    #[cfg(feature = "desktop")]
    let mut win = minifb::Window::new(
        "CardBili 320x170",
        WIDTH as usize,
        HEIGHT as usize,
        minifb::WindowOptions {
            scale: minifb::Scale::X2,
            resize: false,
            ..Default::default()
        },
    )
    .expect("创建预览窗口失败");
    #[cfg(feature = "desktop")]
    let mut dbuf: Vec<u32> = vec![0; (WIDTH * HEIGHT) as usize];

    crate::cache::init();

    let (tx, rx) = mpsc::channel::<Msg>();
    let (ktx, krx) = mpsc::channel::<String>();
    #[cfg(not(feature = "desktop"))]
    util::spawn_keys(kb_path, ktx);

    // 本机预览可用的按键脚本：BILI_KEYS="TAB,RIGHT,ENTER" 按 700ms 间隔依次注入
    #[cfg(feature = "desktop")]
    let scripted: Vec<String> = std::env::var("BILI_KEYS")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
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
    let hold_ms = std::env::var("BILI_HOLD_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1500);
    #[cfg(feature = "desktop")]
    let mut hold_started = false;
    #[cfg(feature = "desktop")]
    let mut hold_release: Option<Instant> = None;

    ui.set_version(concat!("CardBili  v", env!("CARGO_PKG_VERSION")).into());
    ui.set_about_text(format!(
        "CardBili  v{}\nCardputerZero 哔哩哔哩客户端\n\n功能：推荐 / 热门 / 动态 / 我的\n      播放 · 扫码登录 · 视频详情\n\n内核：Rust + Slint 软件渲染\n屏幕：M5Stack 320x170\n\nESC 返回",
        env!("CARGO_PKG_VERSION")
    ).as_str().into());

    let ph = util::make_placeholder();
    let mut page: i32 = 0;
    let mut tab: usize = std::env::var("BILI_TAB")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0)
        .min(4);
    let mut settings = Settings::load();
    let mut view: i32 = std::env::var("BILI_VIEW")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(settings.list_view)
        .clamp(0, 2);
    let mut help: bool = std::env::var("BILI_HELP").is_ok();
    let mut sel: i32 = std::env::var("BILI_SEL")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
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
    let mut account_dirty = true;
    let mut home_dirty = true;
    let mut home_view = None;
    let mut player: Option<player::Player> = None;
    let mut frames_in_seg: u64 = 0;
    let mut gen: u64 = 1;
    let mut danmaku: Option<danmaku::Engine> = None;
    let mut dm_cache = DmCache::new();
    let mut status = String::from("加载中…");
    let mut status_detail = String::new();
    let mut loading = tab < 3;

    // 详情页
    let mut detail_bvid = String::new();
    let mut detail_request = 0u64;
    let mut requested_detail: Option<(String, String)> = None;
    let mut detail_scroll: f64 = 0.0;
    let mut detail_maxv: f64 = 0.0;

    // 跑马灯
    let mut marq_t: f64 = 0.0;
    let mut last_tick = Instant::now();

    let force0 = std::env::var("BILI_FORCE").is_ok();
    if tab < 3 {
        tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), force0);
    }

    if let Ok(b) = std::env::var("BILI_AUTOPLAY") {
        if !b.is_empty() {
            status = "正在加载视频…".into();
            status_detail.clear();
            tasks::start_play(b, cookies.clone(), tx.clone());
        }
    }
    if let Ok(b) = std::env::var("BILI_DETAIL") {
        if !b.is_empty() {
            requested_detail = Some((b, String::new()));
        }
    }
    if std::env::var("BILI_ABOUT").is_ok() {
        page = 4;
    }
    if std::env::var("BILI_TEST_LOGIN").is_ok() {
        page = 2;
        view = 0;
        ui.set_has_qr(false);
        ui.set_qr_status("正在生成二维码…".into());
        tasks::start_login(tx.clone());
    }
    let deadline = std::env::var("BILI_SECONDS")
        .ok()
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
    let mut detail_hold: Option<(f64, Instant)> = None;
    let mut detail_repeat = Instant::now();
    while !quit {
        #[cfg(feature = "desktop")]
        if !win.is_open() {
            quit = true;
        }
        if let Some(d) = deadline {
            if Instant::now() >= d {
                quit = true;
            }
        }
        // 方向键停止输入 300ms 后提交跳转并显示加载状态
        if let Some(dl) = seek_deadline {
            if Instant::now() >= dl {
                seek_deadline = None;
                if let (Some(pp), Some(t)) = (player.as_mut(), seek_pending.take()) {
                    pp.seek(t);
                    frames_in_seg = 0;
                    if let Some(engine) = &danmaku {
                        render_danmaku(&ui, engine, pp.position(frames_in_seg), &mut dm_cache);
                    }
                    ui.set_status("加载中…".into());
                    ui.set_has_frame(false);
                }
            }
        }
        // 长按方向键超过 350ms 后，每 120ms 调整 2 秒
        if page == 1 {
            if let Some(p) = player.as_mut() {
                let mut step = 0.0f64;
                if let Some(start) = left_hold {
                    if start.elapsed() >= Duration::from_millis(350)
                        && left_rep.elapsed() >= Duration::from_millis(120)
                    {
                        left_rep = Instant::now();
                        step = -2.0;
                    }
                }
                if let Some(start) = right_hold {
                    if start.elapsed() >= Duration::from_millis(350)
                        && right_rep.elapsed() >= Duration::from_millis(120)
                    {
                        right_rep = Instant::now();
                        step = 2.0;
                    }
                }
                if step != 0.0 {
                    let base = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
                    let t = (base + step).clamp(0.0, p.duration.max(0.0));
                    ui.set_progress(if p.duration > 0.0 {
                        (t / p.duration) as f32
                    } else {
                        0.0
                    });
                    ui.set_time_text(
                        format!("{} / {}", model::mmss(t), model::mmss(p.duration))
                            .as_str()
                            .into(),
                    );
                    seek_pending = Some(t);
                    seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                }
            }
        }
        if page == 0 && tab < 3 {
            if let Some(t) = esc_press {
                if t.elapsed() >= Duration::from_millis(2000) {
                    quit = true;
                }
            }
        }
        if let Some(t) = bar_until {
            if Instant::now() >= t {
                ui.set_bar("".into());
                ui.set_bar_sticky(false);
                bar_until = None;
            }
        }
        let mut latest: Option<(u64, Vec<u8>)> = None;

        // --- 后台消息 ---
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Frame(g, b) => {
                    if page == 1 {
                        if let Some(p) = player.as_mut() {
                            if g == p.gen {
                                p.frame_ready();
                                frames_in_seg += 1;
                                latest = Some((g, b));
                            }
                        }
                    }
                }
                Msg::SegEnd(g) => {
                    if let Some(p) = player.as_mut() {
                        if g == p.gen {
                            if p.advance() {
                                frames_in_seg = 0;
                            } else {
                                status = "播放结束，按 ESC 返回".into();
                                status_detail.clear();
                            }
                        }
                    }
                }
                Msg::ListLoaded(t, items) => {
                    home_dirty = true;
                    let first = !loaded[t];
                    // 内容与已展示的一致时保留封面与选中，避免 SWR 更新时闪烁
                    let same_ids = !first
                        && lists[t].len() == items.len()
                        && lists[t]
                            .iter()
                            .zip(items.iter())
                            .all(|(a, b)| a.bvid == b.bvid);
                    loaded[t] = true;
                    pages[t] = 1;
                    has_more[t] = true;
                    loading_more[t] = false;
                    let len = items.len();
                    if t < 3 {
                        if same_ids {
                            covers[t].resize_with(len, || None);
                        } else {
                            covers[t] = vec![None; len];
                        }
                        covers_started[t] = true;
                        let missing = items
                            .iter()
                            .zip(&covers[t])
                            .filter(|(_, cover)| cover.is_none())
                            .map(|(item, _)| item.clone())
                            .collect::<Vec<_>>();
                        if !missing.is_empty() {
                            tasks::spawn_covers(t, missing, tx.clone());
                        }
                    }
                    lists[t] = items;
                    if list_source(tab, mine_detail) == Some(t) {
                        if first {
                            sel = 0;
                        } else if sel >= len as i32 {
                            sel = if len == 0 { 0 } else { len as i32 - 1 };
                        }
                        loading = false;
                        if page == 0 {
                            status = if len == 0 {
                                "暂无内容".to_string()
                            } else {
                                format!("{} 条", len)
                            };
                        }
                        status_detail.clear();
                        ui.set_bar(if len == 0 {
                            String::new().into()
                        } else {
                            format!("已获取到 {} 条数据", len).into()
                        });
                        ui.set_bar_sticky(false);
                        if let Ok(v) = std::env::var("BILI_SEL") {
                            if let Ok(v) = v.parse::<i32>() {
                                sel = v;
                            }
                        }
                    }
                }
                Msg::ListMore(t, items) => {
                    home_dirty = true;
                    loading_more[t] = false;
                    if items.is_empty() {
                        has_more[t] = false;
                    } else {
                        let mut known: HashSet<String> =
                            lists[t].iter().map(|item| item.bvid.clone()).collect();
                        let add: Vec<model::Item> = items
                            .into_iter()
                            .filter(|item| known.insert(item.bvid.clone()))
                            .collect();
                        if add.is_empty() {
                            has_more[t] = false;
                        } else {
                            pages[t] += 1;
                            lists[t].extend(add.clone());
                            covers[t].extend(vec![None; add.len()]);
                            if std::env::var("BILI_DUMP").is_ok() {
                                eprintln!(
                                    "MORE tab={} page={} add={} total={}",
                                    t,
                                    pages[t],
                                    add.len(),
                                    lists[t].len()
                                );
                            }
                            tasks::spawn_covers(t, add, tx.clone());
                            if t == tab && page == 0 {
                                status = format!("{} 条", lists[t].len());
                                status_detail.clear();
                                ui.set_bar(format!("已获取到 {} 条数据", lists[t].len()).into());
                                ui.set_bar_sticky(false);
                            }
                        }
                    }
                }
                Msg::Err(t, msg) => {
                    let (friendly, tech) = match msg.split_once('\n') {
                        Some((a, b)) => (a.to_string(), b.to_string()),
                        None => (msg, String::new()),
                    };
                    if t == 9 {
                        status = friendly;
                        status_detail = tech;
                    } else if list_source(tab, mine_detail) == Some(t) {
                        loading = false;
                        if page == 0 {
                            status = friendly;
                            status_detail = tech;
                            ui.set_bar("加载失败".into());
                            ui.set_bar_sticky(false);
                        }
                    }
                }
                Msg::PlayReady(info) => {
                    if let Some(mut old) = player.take() {
                        gen = gen.max(old.gen);
                        old.stop();
                    }
                    gen += 1;
                    frames_in_seg = 0;
                    let p = player::Player::start(
                        info.segs,
                        info.duration,
                        info.title,
                        cookies.clone().unwrap_or_default(),
                        gen,
                        tx.clone(),
                    );
                    player = Some(p);
                    page = 1;
                    left_hold = None;
                    right_hold = None;
                    seek_pending = None;
                    seek_deadline = None;
                    ui.set_has_frame(false);
                    danmaku = None;
                    ui.set_dm_rows(slint::ModelRc::new(VecModel::from(
                        Vec::<crate::DmRow>::new(),
                    )));
                    tasks::start_danmaku(info.cid, cookies.clone(), gen, tx.clone());
                    status = "加载中…".into();
                    status_detail.clear();
                }
                Msg::Danmaku(g, list) => {
                    if g == gen {
                        let eng = danmaku::Engine::new(list);
                        let t = player
                            .as_ref()
                            .map(|p| p.position(frames_in_seg))
                            .unwrap_or(0.0);
                        render_danmaku(&ui, &eng, t, &mut dm_cache);
                        danmaku = Some(eng);
                    }
                }
                Msg::QrReady(url) => {
                    if let Some(img) = util::make_qr(&url) {
                        ui.set_qr(img);
                        ui.set_has_qr(true);
                    }
                    ui.set_qr_status("请用 B 站 App 扫码".into());
                }
                Msg::QrStatus(s) => {
                    ui.set_qr_status(s.as_str().into());
                }
                Msg::QrDone(ck) => {
                    if !ck.is_empty() {
                        if let Some(dir) = std::path::Path::new(&cpath).parent() {
                            let _ = std::fs::create_dir_all(dir);
                        }
                        let _ = std::fs::write(&cpath, &ck);
                        mid = util::parse_mid(&ck);
                        cookies = Some(ck);
                        account_dirty = true;
                        ui.set_qr_status("登录成功".into());
                        acct_loaded = false;
                        page = 0;
                        if tab >= 3 {
                            loading = false;
                            status = "已登录".into();
                            status_detail.clear();
                        } else {
                            loaded[tab] = false;
                            loading = true;
                            status = "已登录，刷新中…".into();
                            status_detail.clear();
                            tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), false);
                        }
                    }
                }
                Msg::Acct(a, raw) => {
                    account_dirty = true;
                    if !raw.is_empty() {
                        if let Some(img) = util::decode_avatar(&raw) {
                            avatar_img = Some(img);
                        }
                    }
                    acct = Some(a);
                }
                Msg::Cover(t, bvid, w, h, raw) => {
                    if t < 5 {
                        if let Some(idx) = lists[t].iter().position(|it| it.bvid == bvid) {
                            if idx < covers[t].len() {
                                covers[t][idx] = Some(util::raw_to_image(w, h, &raw));
                                home_dirty = true;
                            }
                        }
                    }
                }
                Msg::Detail(request, d, w, h, raw) => {
                    if page != 3 || request != detail_request {
                        continue;
                    }
                    ui.set_detail_loading(false);
                    ui.set_detail_error("".into());
                    let detail_img = if raw.is_empty() {
                        None
                    } else {
                        Some(util::raw_to_image(w, h, &raw))
                    };
                    let desc = d.desc.trim();
                    let desc = if desc.is_empty() {
                        "（无简介）".to_string()
                    } else {
                        desc.to_string()
                    };
                    if std::env::var("BILI_DUMP").is_ok() {
                        eprintln!("DETAIL comments={}", d.comments.len());
                    }
                    ui.set_detail_scroll(detail_scroll as f32);
                    sync_thumb(&ui, detail_scroll, detail_maxv);
                    ui.set_detail_owner(
                        format!(
                            "UP主：{}",
                            if d.owner.is_empty() {
                                "未知"
                            } else {
                                d.owner.as_str()
                            }
                        )
                        .as_str()
                        .into(),
                    );
                    ui.set_detail_desc(desc.as_str().into());
                    let cmts: Vec<Cmt> = d
                        .comments
                        .iter()
                        .map(|c| Cmt {
                            user: c.user.clone().into(),
                            text: c.text.clone().into(),
                        })
                        .collect();
                    ui.set_comments(ModelRc::new(VecModel::from(cmts)));
                    ui.set_detail_title(d.title.as_str().into());
                    ui.set_detail_stats(
                        format!(
                            "时长 {} · 播放 {} · 点赞 {} · 投币 {}",
                            model::mmss(d.duration),
                            model::fmt_num(d.views),
                            model::fmt_num(d.likes),
                            model::fmt_num(d.coins)
                        )
                        .as_str()
                        .into(),
                    );
                    ui.set_has_detail_cover(detail_img.is_some());
                    if let Some(im) = detail_img {
                        ui.set_detail_cover(im);
                    }
                }
                Msg::DetailError(request, error) => {
                    if page == 3 && request == detail_request {
                        ui.set_detail_loading(false);
                        ui.set_detail_error(error.into());
                    }
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
        if let Some(hk) = &hold_key {
            if !hold_started && scripted.is_empty() && Instant::now() >= script_next {
                let _ = ktx.send(hk.clone());
                hold_started = true;
                hold_release = Some(Instant::now() + Duration::from_millis(hold_ms));
            }
        }
        #[cfg(feature = "desktop")]
        if let Some(rt) = hold_release {
            if Instant::now() >= rt {
                if let Some(hk) = &hold_key {
                    let _ = ktx.send(format!("{}_UP", hk));
                }
                hold_release = None;
            }
        }
        #[cfg(feature = "desktop")]
        {
            let cur: std::collections::HashSet<minifb::Key> = win.get_keys().into_iter().collect();
            for key in cur.iter() {
                if !prev_keys.contains(key) {
                    if let Some(s) = desktop_key(*key) {
                        let _ = ktx.send(s.to_string());
                    }
                }
            }
            for key in prev_keys.iter() {
                if !cur.contains(key) {
                    if let Some(release) = desktop_key(*key).and_then(util::key_release) {
                        let _ = ktx.send(release.to_string());
                    }
                }
            }
            prev_keys = cur;
        }
        let mut settings_changed = false;
        while let Ok(k) = krx.try_recv() {
            last_input = Instant::now();
            if k == "LEFT_UP" {
                left_hold = None;
                continue;
            }
            if k == "RIGHT_UP" {
                right_hold = None;
                continue;
            }
            if k == "UP_UP" || k == "DOWN_UP" {
                let direction = if k == "UP_UP" { -22.0 } else { 22.0 };
                if detail_hold.is_some_and(|(step, _)| step == direction) {
                    detail_hold = None;
                }
                continue;
            }
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
                    tab = (tab + 1) % 5;
                    ui.set_bar("".into());
                    ui.set_bar_sticky(false);
                    sel = 0;
                    mine_detail = -1;
                    if tab >= 3 {
                        loading = false;
                        status.clear();
                        status_detail.clear();
                        acct_loaded = false;
                    } else {
                        if loaded[tab] {
                            loading = false;
                            status = format!("{} 条", lists[tab].len());
                            status_detail.clear();
                        } else {
                            loading = true;
                            status = "加载中…".into();
                            status_detail.clear();
                            tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), false);
                        }
                        if loaded[tab] && !covers_started[tab] {
                            covers_started[tab] = true;
                            covers[tab] = vec![None; lists[tab].len()];
                            tasks::spawn_covers(tab, lists[tab].clone(), tx.clone());
                        }
                    }
                    continue;
                }
                if help {
                    if matches!(k.as_str(), "7" | "ESC" | "BACKSPACE") {
                        help = false;
                    }
                } else if tab == 4 {
                    match k.as_str() {
                        "UP" | "f" | "w" => sel = (sel - 1).max(0),
                        "DOWN" | "x" | "s" => sel = (sel + 1).min(2),
                        "LEFT" | "z" | "RIGHT" | "c" => {
                            let step = if matches!(k.as_str(), "LEFT" | "z") {
                                -1
                            } else {
                                1
                            };
                            if sel == 0 {
                                view = (view + step).rem_euclid(3);
                                settings_changed = true;
                            } else if sel == 1 {
                                let transparency =
                                    (settings.danmaku_transparency + step * 10).clamp(0, 100);
                                settings_changed |= transparency != settings.danmaku_transparency;
                                settings.danmaku_transparency = transparency;
                            }
                        }
                        "ENTER" if sel == 2 => page = 4,
                        "7" | "8" | "h" => help = true,
                        "ESC" | "BACKSPACE" => {
                            tab = 0;
                            sel = 0;
                        }
                        _ => {}
                    }
                } else if tab == 3 {
                    if mine_detail < 0 {
                        match k.as_str() {
                            "7" | "8" | "h" => {
                                help = true;
                            }
                            "UP" | "f" | "w" | "LEFT" | "z" => {
                                sel = (sel - 1).max(0);
                            }
                            "DOWN" | "x" | "s" | "RIGHT" | "c" => {
                                if sel < 1 {
                                    sel += 1;
                                }
                            }
                            "ENTER" => {
                                mine_detail = if sel == 0 { 3 } else { 4 };
                                sel = 0;
                                let src = mine_detail as usize;
                                loaded[src] = false;
                                loading = true;
                                status = "加载中…".into();
                                status_detail.clear();
                                tasks::load_list(
                                    src,
                                    cookies.clone(),
                                    mid.clone(),
                                    tx.clone(),
                                    false,
                                );
                            }
                            "r" | "4" => {
                                acct_loaded = false;
                            }
                            "l" | "5" => {
                                page = 2;
                                ui.set_has_qr(false);
                                ui.set_qr_status("正在生成二维码…".into());
                                tasks::start_login(tx.clone());
                            }
                            "ESC" | "BACKSPACE" => {
                                tab = 0;
                                sel = 0;
                                mine_detail = -1;
                            }
                            _ => {}
                        }
                    } else {
                        let src = mine_detail as usize;
                        let n = lists[src].len() as i32;
                        match k.as_str() {
                            "UP" | "f" | "w" | "LEFT" | "z" => {
                                sel = (sel - 1).max(0);
                            }
                            "DOWN" | "x" | "s" | "RIGHT" | "c" => {
                                if sel + 1 < n {
                                    sel += 1;
                                }
                            }
                            "ENTER" => {
                                if let Some(it) = lists[src].get(sel as usize) {
                                    if !it.bvid.is_empty() {
                                        status = "正在加载视频…".into();
                                        status_detail.clear();
                                        tasks::start_play(
                                            it.bvid.clone(),
                                            cookies.clone(),
                                            tx.clone(),
                                        );
                                    } else {
                                        status = "该条目无视频".into();
                                        status_detail.clear();
                                    }
                                }
                            }
                            "SPACE" => {
                                if let Some(it) = lists[src].get(sel as usize) {
                                    if !it.bvid.is_empty() {
                                        requested_detail =
                                            Some((it.bvid.clone(), it.title.clone()));
                                    }
                                }
                            }
                            "r" | "4" => {
                                loaded[src] = false;
                                loading = true;
                                status = "刷新中…".into();
                                status_detail.clear();
                                tasks::load_list(
                                    src,
                                    cookies.clone(),
                                    mid.clone(),
                                    tx.clone(),
                                    true,
                                );
                            }
                            "ESC" | "BACKSPACE" => {
                                mine_detail = -1;
                                sel = 0;
                            }
                            _ => {}
                        }
                    }
                } else {
                    let n = lists[tab].len() as i32;
                    let cols: i32 = if view == 2 {
                        2
                    } else if view == 1 {
                        3
                    } else {
                        1
                    };
                    match k.as_str() {
                        "7" | "8" | "h" => {
                            help = true;
                        }
                        "v" => {
                            view = (view + 1) % 3;
                            settings_changed = true;
                        }
                        "UP" | "f" | "w" => {
                            let st = if view > 0 { cols } else { 1 };
                            sel = (sel - st).max(0);
                        }
                        "DOWN" | "x" | "s" => {
                            let st = if view > 0 { cols } else { 1 };
                            if sel + st < n {
                                sel += st;
                            }
                        }
                        "LEFT" | "z" => {
                            sel = (sel - 1).max(0);
                        }
                        "RIGHT" | "c" => {
                            if sel + 1 < n {
                                sel += 1;
                            }
                        }
                        "ENTER" => {
                            if let Some(it) = lists[tab].get(sel as usize) {
                                if !it.bvid.is_empty() {
                                    status = "正在加载视频…".into();
                                    status_detail.clear();
                                    tasks::start_play(it.bvid.clone(), cookies.clone(), tx.clone());
                                } else {
                                    status = "该条目无视频".into();
                                    status_detail.clear();
                                }
                            }
                        }
                        "SPACE" => {
                            if let Some(it) = lists[tab].get(sel as usize) {
                                if !it.bvid.is_empty() {
                                    requested_detail = Some((it.bvid.clone(), it.title.clone()));
                                }
                            }
                        }
                        "r" | "4" => {
                            loaded[tab] = false;
                            loading = true;
                            status = "刷新中…".into();
                            status_detail.clear();
                            tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), true);
                        }
                        "l" | "5" => {
                            page = 2;
                            ui.set_has_qr(false);
                            ui.set_qr_status("正在生成二维码…".into());
                            tasks::start_login(tx.clone());
                        }
                        "ESC_HOLD" => {
                            quit = true;
                        }
                        _ => {}
                    }
                }
            } else if page == 1 {
                if let Some(p) = player.as_mut() {
                    match k.as_str() {
                        "SPACE" | "ENTER" => {
                            p.pause_toggle();
                        }
                        "LEFT" | "z" => {
                            left_hold = Some(Instant::now());
                            left_rep = Instant::now();
                            let base = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
                            let t = (base - 10.0).max(0.0);
                            ui.set_progress(if p.duration > 0.0 {
                                (t / p.duration) as f32
                            } else {
                                0.0
                            });
                            ui.set_time_text(
                                format!("{} / {}", model::mmss(t), model::mmss(p.duration))
                                    .as_str()
                                    .into(),
                            );
                            seek_pending = Some(t);
                            seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                        }
                        "RIGHT" | "c" => {
                            right_hold = Some(Instant::now());
                            right_rep = Instant::now();
                            let base = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
                            let t = (base + 10.0).min(p.duration.max(0.0));
                            ui.set_progress(if p.duration > 0.0 {
                                (t / p.duration) as f32
                            } else {
                                0.0
                            });
                            ui.set_time_text(
                                format!("{} / {}", model::mmss(t), model::mmss(p.duration))
                                    .as_str()
                                    .into(),
                            );
                            seek_pending = Some(t);
                            seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                        }
                        "UP" | "f" | "w" => {
                            p.vol_up();
                        }
                        "DOWN" | "x" | "s" => {
                            p.vol_down();
                        }
                        "ESC" | "BACKSPACE" => {
                            gen = gen.max(p.gen);
                            p.stop();
                            player = None;
                            danmaku = None;
                            page = 0;
                            frames_in_seg = 0;
                            seek_pending = None;
                            seek_deadline = None;
                            ui.set_has_frame(false);
                        }
                        _ => {}
                    }
                } else if matches!(k.as_str(), "ESC" | "BACKSPACE") {
                    page = 0;
                    ui.set_has_frame(false);
                }
            } else if page == 2 {
                match k.as_str() {
                    "ESC" | "BACKSPACE" => {
                        page = 0;
                    }
                    "r" | "4" => {
                        ui.set_has_qr(false);
                        ui.set_qr_status("正在刷新二维码…".into());
                        tasks::start_login(tx.clone());
                    }
                    _ => {}
                }
            } else if page == 3 {
                match k.as_str() {
                    "UP" | "f" | "w" => {
                        detail_hold = Some((-22.0, Instant::now()));
                        detail_repeat = Instant::now();
                        detail_scroll = (detail_scroll - 22.0).max(0.0);
                        ui.set_detail_scroll(detail_scroll as f32);
                        sync_thumb(&ui, detail_scroll, detail_maxv);
                    }
                    "DOWN" | "x" | "s" => {
                        detail_hold = Some((22.0, Instant::now()));
                        detail_repeat = Instant::now();
                        detail_scroll = (detail_scroll + 22.0).min(detail_maxv);
                        ui.set_detail_scroll(detail_scroll as f32);
                        sync_thumb(&ui, detail_scroll, detail_maxv);
                    }
                    "ENTER" => {
                        if !detail_bvid.is_empty() {
                            status = "正在加载视频…".into();
                            status_detail.clear();
                            tasks::start_play(detail_bvid.clone(), cookies.clone(), tx.clone());
                        }
                    }
                    "ESC" | "BACKSPACE" => {
                        page = 0;
                    }
                    _ => {}
                }
            } else if page == 4 {
                match k.as_str() {
                    "ESC" | "BACKSPACE" | "ENTER" => {
                        page = 0;
                    }
                    _ => {}
                }
            }
        }

        if settings_changed {
            settings.list_view = view;
            if let Err(error) = settings.save() {
                eprintln!("设置保存失败: {error}");
                ui.set_bar("设置保存失败".into());
                ui.set_bar_sticky(false);
            }
        }
        if let Some((bvid, title)) = requested_detail.take() {
            page = 3;
            detail_bvid = bvid.clone();
            detail_request += 1;
            detail_scroll = std::env::var("BILI_DSCROLL")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0.0);
            detail_maxv = 0.0;
            detail_hold = None;
            show_detail_loading(&ui, &title);
            tasks::start_detail(bvid, cookies.clone(), detail_request, tx.clone());
        }
        if page == 3 && !ui.get_detail_loading() {
            if let Some((step, since)) = detail_hold {
                if since.elapsed() >= Duration::from_millis(350)
                    && detail_repeat.elapsed() >= Duration::from_millis(120)
                {
                    detail_repeat = Instant::now();
                    detail_scroll = (detail_scroll + step).clamp(0.0, detail_maxv);
                    ui.set_detail_scroll(detail_scroll as f32);
                    sync_thumb(&ui, detail_scroll, detail_maxv);
                }
            }
        } else {
            detail_hold = None;
        }

        // --- 应用最新帧 ---
        if let Some((frame_gen, bytes)) = latest {
            if page == 1
                && player
                    .as_ref()
                    .is_some_and(|player| player.gen == frame_gen)
                && bytes.len() == VID_BYTES
            {
                let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(WIDTH, HEIGHT);
                pb.make_mut_bytes().copy_from_slice(&bytes);
                ui.set_frame(slint::Image::from_rgb8(pb));
                ui.set_has_frame(true);
                if let (Some(p), Some(eng)) = (player.as_ref(), danmaku.as_ref()) {
                    render_danmaku(&ui, eng, p.position(frames_in_seg), &mut dm_cache);
                }
            }
        }

        // --- 选中项夹取 ---
        if page == 0 {
            let len = if tab == 4 {
                3
            } else if tab == 3 && mine_detail < 0 {
                2
            } else {
                lists[if tab == 3 { mine_detail as usize } else { tab }].len() as i32
            };
            if len > 0 && sel >= len {
                sel = len - 1;
            }
            if sel < 0 {
                sel = 0;
            }
            // 推荐和热门列表接近底部时自动加载下一页
            if tab < 2
                && mine_detail < 0
                && view >= 0
                && loaded[tab]
                && !loading_more[tab]
                && has_more[tab]
            {
                let n = lists[tab].len() as i32;
                let cols2: i32 = if view == 2 {
                    2
                } else if view == 1 {
                    3
                } else {
                    1
                };
                if n > 0 && sel >= n - cols2 * 2 {
                    loading_more[tab] = true;
                    tasks::load_more(tab, (pages[tab] + 1) as u32, cookies.clone(), tx.clone());
                }
            }
        }

        // --- 账号信息 ---
        if tab == 3 && !acct_loaded {
            acct_loaded = true;
            if cookies.is_some() {
                tasks::fetch_account(cookies.clone(), tx.clone());
            } else {
                acct = None;
                avatar_img = None;
                account_dirty = true;
            }
        }
        if account_dirty {
            ui.set_acct_known(cookies.is_some());
            ui.set_acct_logged(acct.as_ref().map(|a| a.logged).unwrap_or(false));
            ui.set_acct_name(
                acct.as_ref()
                    .map(|a| a.name.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            ui.set_acct_mid(
                acct.as_ref()
                    .map(|a| a.mid.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            let lv = acct.as_ref().map(|a| a.level).unwrap_or(0);
            ui.set_acct_level(
                if lv > 0 {
                    format!("Lv{}", lv)
                } else {
                    String::new()
                }
                .as_str()
                .into(),
            );
            ui.set_acct_vip(
                acct.as_ref()
                    .map(|a| a.vip.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            ui.set_has_avatar(avatar_img.is_some());
            if let Some(im) = avatar_img.clone() {
                ui.set_avatar(im);
            }

            account_dirty = false;
        }

        // --- 跑马灯计时 ---
        let now = Instant::now();
        marq_t += now.duration_since(last_tick).as_secs_f64();
        last_tick = now;

        // --- 详情页滚动尺寸 ---
        if page == 3 && !ui.get_detail_loading() {
            let m = (ui.get_detail_content_h() as f64 - 84.0).max(0.0);
            if (m - detail_maxv).abs() > 0.5 {
                detail_maxv = m;
            }
            if std::env::var("BILI_DSEND").is_ok() {
                detail_scroll = detail_maxv;
                ui.set_detail_scroll(detail_scroll as f32);
            }
            if detail_scroll > detail_maxv {
                detail_scroll = detail_maxv;
                ui.set_detail_scroll(detail_scroll as f32);
            }
            sync_thumb(&ui, detail_scroll, detail_maxv);
        }

        // --- 同步 UI ---
        // 播放中 3 秒无操作隐藏控件；暂停或正在长按调进度时保持显示
        let controls_show = if page == 1 {
            player
                .as_ref()
                .map(|p| {
                    p.paused
                        || left_hold.is_some()
                        || right_hold.is_some()
                        || last_input.elapsed() < Duration::from_millis(3000)
                })
                .unwrap_or(true)
        } else {
            true
        };
        ui.set_controls_visible(controls_show);
        ui.set_page(page);
        ui.set_tab(tab as i32);
        ui.set_sel(sel);
        ui.set_status(status.as_str().into());
        ui.set_status_detail(status_detail.as_str().into());
        ui.set_loading(loading);
        ui.set_view(view);
        ui.set_danmaku_transparency(settings.danmaku_transparency);
        ui.set_help(help);
        ui.set_mine_detail(mine_detail >= 0);

        // 列表标题跑马灯
        let mut list_marq = false;
        let mut list_shift = 0.0f32;
        if page == 0 && tab < 4 && view == 0 {
            let sel_title: String = if tab == 3 {
                if mine_detail >= 0 {
                    lists[mine_detail as usize]
                        .get(sel as usize)
                        .map(|it| it.title.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                lists[tab]
                    .get(sel as usize)
                    .map(|it| it.title.clone())
                    .unwrap_or_default()
            };
            let est = util::est_w(&sel_title, 12.0);
            let clip = (WIDTH as f64) - 16.0;
            list_marq = est > clip;
            list_shift = yoyo_shift(est, clip, marq_t);
        }
        ui.set_title_marquee(list_marq);
        ui.set_title_shift(list_shift);

        let grid_columns = if view == 2 { 2 } else { 3 };
        let grid_size = grid_columns * 2;
        let grid_start = sel.max(0) as usize / grid_size * grid_size;
        let current_home_view = (
            tab,
            view,
            mine_detail,
            if view > 0 && tab < 3 { grid_start } else { 0 },
        );
        if page == 0 && tab < 4 && (home_dirty || home_view != Some(current_home_view)) {
            home_dirty = false;
            home_view = Some(current_home_view);
            if tab == 3 {
                if mine_detail < 0 {
                    let titles: Vec<slint::SharedString> = vec!["历史".into(), "收藏".into()];
                    let hist = if loaded[3] {
                        format!("{} 条", lists[3].len())
                    } else {
                        String::new()
                    };
                    let favm = if loaded[4] {
                        format!("{} 条", lists[4].len())
                    } else {
                        String::new()
                    };
                    let metas: Vec<slint::SharedString> = vec![hist.into(), favm.into()];
                    ui.set_titles(slint::ModelRc::new(slint::VecModel::from(titles)));
                    ui.set_metas(slint::ModelRc::new(slint::VecModel::from(metas)));
                } else {
                    let src = mine_detail as usize;
                    sync_list(&ui, &lists[src]);
                }
            } else if view == 0 {
                sync_list(&ui, &lists[tab]);
            } else {
                let len = lists[tab].len();
                let cols = grid_columns as i32;
                let start = grid_start;
                let end = (start + grid_size).min(len);
                let mut gt: Vec<slint::SharedString> = Vec::new();
                let mut gc: Vec<slint::Image> = Vec::new();
                for (i, item) in lists[tab].iter().enumerate().take(end).skip(start) {
                    gt.push(item.title.as_str().into());
                    let im = covers[tab]
                        .get(i)
                        .and_then(|o| o.clone())
                        .unwrap_or_else(|| ph.clone());
                    gc.push(im);
                }
                ui.set_cols(cols);
                ui.set_cell_w(if cols == 2 { 160.0 } else { 106.0 });
                ui.set_cell_h(58.0);
                ui.set_grid_titles(slint::ModelRc::new(slint::VecModel::from(gt)));
                ui.set_grid_covers(slint::ModelRc::new(slint::VecModel::from(gc)));
            }
        }
        if page == 0 && tab < 3 && view > 0 {
            let active = if (sel as usize) < lists[tab].len() {
                sel - grid_start as i32
            } else {
                -1
            };
            ui.set_grid_active(active);
        }
        if page == 1 {
            if let Some(p) = player.as_ref() {
                ui.set_ptitle(p.title.as_str().into());
                ui.set_paused(p.paused);
                // 待提交的跳转使用预览位置
                let pos = seek_pending.unwrap_or_else(|| p.position(frames_in_seg));
                let dur = p.duration;
                ui.set_progress(if dur > 0.0 { (pos / dur) as f32 } else { 0.0 });
                ui.set_time_text(
                    format!("{} / {}", model::mmss(pos), model::mmss(dur))
                        .as_str()
                        .into(),
                );
                let st = if p.paused {
                    "已暂停  SPACE继续"
                } else {
                    "Z/C 快退进  F/X 音量  SPACE暂停"
                };
                ui.set_play_state(st.into());

                let est = util::est_w(&p.title, 10.0);
                let clip = (WIDTH as f64) - 12.0;
                ui.set_ptitle_marquee(est > clip);
                ui.set_ptitle_shift(yoyo_shift(est, clip, marq_t));
            }
        }

        // --- 渲染 ---
        slint::platform::update_timers_and_animations();
        let redrawn = window.draw_if_needed(|r| {
            let _ = r.render(&mut buffer, WIDTH as usize);
        });
        if redrawn {
            #[cfg(not(feature = "desktop"))]
            {
                let out =
                    unsafe { std::slice::from_raw_parts(buffer.as_ptr() as *const u8, FB_BYTES) };
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
                if win
                    .update_with_buffer(&dbuf, WIDTH as usize, HEIGHT as usize)
                    .is_err()
                {
                    quit = true;
                }
            }
        }

        #[cfg(feature = "desktop")]
        if !redrawn {
            win.update();
        }
        thread::sleep(Duration::from_millis(16));
    }

    if let Some(mut p) = player.take() {
        p.stop();
    }
}
