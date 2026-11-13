use super::*;

pub(super) struct App {
    pub(super) volume_queue: mpsc::SyncSender<i32>,
    pub(super) history: history::History,
    pub(super) controls: playback::Controls,
    pub(super) comment_page: comments::Comments,
    pub(super) comments_request: u64,
    pub(super) requested_comments: Option<i64>,
    pub(super) login_return: bool,
    pub(super) ui: AppWindow,
    pub(super) tx: mpsc::Sender<Msg>,
    pub(super) cpath: String,
    pub(super) cookies: Option<String>,
    pub(super) mid: String,
    pub(super) stats_queue: mpsc::SyncSender<(String, Option<String>)>,
    pub(super) stats_requested: HashSet<String>,
    pub(super) stats_cache: HashMap<String, model::VideoStats>,
    pub(super) ph: slint::Image,
    pub(super) page: i32,
    pub(super) tab: usize,
    pub(super) settings: Settings,
    pub(super) view: i32,
    pub(super) help: bool,
    pub(super) sel: i32,
    pub(super) lists: Vec<Vec<model::Item>>,
    pub(super) covers: Vec<Vec<Option<slint::Image>>>,
    pub(super) covers_started: [bool; 5],
    pub(super) loaded: [bool; 5],
    pub(super) pages: [usize; 5],
    pub(super) has_more: [bool; 5],
    pub(super) loading_more: [bool; 5],
    pub(super) mine_detail: i32,
    pub(super) acct: Option<model::Account>,
    pub(super) avatar_img: Option<slint::Image>,
    pub(super) acct_loaded: bool,
    pub(super) account_dirty: bool,
    pub(super) home_dirty: bool,
    pub(super) home_view: Option<(usize, i32, i32, usize, i32)>,
    pub(super) menu_view: Option<[Option<menu::Action>; 5]>,
    pub(super) player: Option<player::Player>,
    pub(super) frames_in_seg: u64,
    pub(super) gen: u64,
    pub(super) danmaku: Option<danmaku::Engine>,
    pub(super) danmaku_view: DanmakuView,
    pub(super) status: String,
    pub(super) status_detail: String,
    pub(super) loading: bool,
    pub(super) detail_from_uploader: bool,
    pub(super) scroll_tap: Option<(f64, Instant)>,
    pub(super) detail_bvid: String,
    pub(super) detail_request: u64,
    pub(super) detail_display: Option<DetailDisplay>,
    pub(super) play_request: u64,
    pub(super) current_play: Option<model::PlayInfo>,
    pub(super) requested_play: Option<String>,
    pub(super) requested_uploader: Option<model::Owner>,
    pub(super) requested_login: bool,
    pub(super) login_request: u64,
    pub(super) login_uploader: Option<model::Owner>,
    pub(super) dynamic_offset: String,
    pub(super) dynamic_request: u64,
    pub(super) following: following::FollowingState,
    pub(super) following_request: u64,
    pub(super) requested_following: bool,
    pub(super) login_following: bool,
    pub(super) uploader: UploaderState,
    pub(super) uploader_request: u64,
    pub(super) uploader_dirty: bool,
    pub(super) back_stack: Vec<BackTarget>,
    pub(super) first_row: usize,
    pub(super) grid_scroll: f64,
    pub(super) scroll_context: Option<(i32, i32, i32, i32)>,
    pub(super) about_scroll: f64,
    pub(super) requested_detail: Option<(String, String)>,
    pub(super) detail_scroll: f64,
    pub(super) detail_maxv: f64,
    pub(super) marq_t: f64,
    pub(super) last_tick: Instant,
    pub(super) deadline: Option<Instant>,
    pub(super) quit: bool,
    pub(super) seek_pending: Option<f64>,
    pub(super) seek_deadline: Option<Instant>,
    pub(super) esc_press: Option<Instant>,
    pub(super) bar_until: Option<Instant>,
    pub(super) last_input: Instant,
    pub(super) left_hold: Option<Instant>,
    pub(super) right_hold: Option<Instant>,
    pub(super) left_rep: Instant,
    pub(super) right_rep: Instant,
    pub(super) detail_hold: Option<(f64, Instant)>,
    pub(super) detail_repeat: Instant,
    pub(super) latest: Option<(u64, Vec<u8>)>,
    pub(super) settings_changed: bool,
    pub(super) go_back: bool,
}

impl App {
    pub(super) fn new(ui: AppWindow, tx: mpsc::Sender<Msg>) -> Self {
        let cpath = util::cookie_path();
        let cookies: Option<String> = std::fs::read_to_string(&cpath)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let mid = cookies.as_deref().map(util::parse_mid).unwrap_or_default();

        let stats_queue = tasks::spawn_stats_worker(tx.clone());
        let stats_requested = HashSet::new();
        let stats_cache = HashMap::new();
        ui.set_version(format!("V{}", env!("CARGO_PKG_VERSION")).into());
        ui.set_about_text("作者  journey-ad\n\n项目地址\ngithub.com/journey-ad/CardBili\n\n字体\nFusion Pixel Font\ngithub.com/TakWolf/fusion-pixel-font\n\n接口参考\nPiliPlus\ngithub.com/bggRGjQaUbCoE/PiliPlus\n\n图标\nPixelarticons\ngithub.com/halfmage/pixelarticons".into());

        let ph = util::make_placeholder();
        let mut page: i32 = 0;
        let tab: usize = std::env::var("BILI_TAB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            .min(4);
        let settings = Settings::load();
        let mut view: i32 = std::env::var("BILI_VIEW")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(settings.list_view)
            .clamp(0, 2);
        let help: bool = std::env::var("BILI_HELP").is_ok();
        let sel: i32 = std::env::var("BILI_SEL")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0);
        let lists: Vec<Vec<model::Item>> = vec![Vec::new(); 5];
        let covers: Vec<Vec<Option<slint::Image>>> = vec![Vec::new(); 5];
        let covers_started = [false; 5];
        let loaded = [false; 5];
        let pages = [1usize; 5];
        let has_more = [true; 5];
        let loading_more = [false; 5];
        let mine_detail: i32 = -1;
        let acct: Option<model::Account> = None;
        let avatar_img: Option<slint::Image> = None;
        let acct_loaded = false;
        let account_dirty = true;
        let home_dirty = true;
        let home_view = None;
        let menu_view = None;
        let player: Option<player::Player> = None;
        let frames_in_seg: u64 = 0;
        let gen: u64 = 1;
        let danmaku: Option<danmaku::Engine> = None;
        let danmaku_view = DanmakuView::new(&ui);
        let mut status = String::from("加载中…");
        let mut status_detail = String::new();
        let loading = tab < 3;

        // 详情页
        let detail_bvid = String::new();
        let detail_request = 0u64;
        let detail_display: Option<DetailDisplay> = None;
        let play_request = 0u64;
        let current_play: Option<model::PlayInfo> = None;
        let mut requested_play: Option<String> = None;
        let requested_uploader: Option<model::Owner> = None;
        let requested_login = false;
        let mut login_request = 0u64;
        let login_uploader: Option<model::Owner> = None;
        let uploader = UploaderState::default();
        let uploader_request = 0u64;
        let uploader_dirty = false;
        let back_stack: Vec<BackTarget> = Vec::new();
        let first_row = 0usize;
        let grid_scroll = 0.0f64;
        let scroll_context = None;
        let about_scroll = 0.0f64;
        let mut requested_detail: Option<(String, String)> = None;
        let detail_scroll: f64 = 0.0;
        let detail_maxv: f64 = 0.0;

        // 跑马灯
        let marq_t: f64 = 0.0;
        let last_tick = Instant::now();

        let force0 = std::env::var("BILI_FORCE").is_ok();
        if tab < 3 {
            tasks::load_list(tab, cookies.clone(), mid.clone(), tx.clone(), force0);
        }

        if let Ok(b) = std::env::var("BILI_AUTOPLAY") {
            if !b.is_empty() {
                status = "正在加载视频…".into();
                status_detail.clear();
                requested_play = Some(b);
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
            login_request += 1;
            tasks::start_login(login_request, tx.clone());
        }
        let deadline = std::env::var("BILI_SECONDS")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|s| Instant::now() + Duration::from_secs_f64(s));

        let quit = false;
        let seek_pending: Option<f64> = None;
        let seek_deadline: Option<Instant> = None;
        let esc_press: Option<Instant> = None;
        let bar_until: Option<Instant> = None;
        let last_input = Instant::now();
        let left_hold: Option<Instant> = None;
        let right_hold: Option<Instant> = None;
        let left_rep = Instant::now();
        let right_rep = Instant::now();
        let detail_hold: Option<(f64, Instant)> = None;
        let detail_repeat = Instant::now();
        let latest = None;
        let settings_changed = false;
        let go_back = false;
        Self {
            volume_queue: player::volume::worker(tx.clone()),
            history: history::History::new(settings.record_history),
            controls: playback::Controls::default(),
            comment_page: comments::Comments::default(),
            comments_request: 0,
            requested_comments: None,
            login_return: false,
            ui,
            tx,
            cpath,
            cookies,
            mid,
            stats_queue,
            stats_requested,
            stats_cache,
            ph,
            page,
            tab,
            settings,
            view,
            help,
            sel,
            lists,
            covers,
            covers_started,
            loaded,
            pages,
            has_more,
            loading_more,
            mine_detail,
            acct,
            avatar_img,
            acct_loaded,
            account_dirty,
            home_dirty,
            home_view,
            menu_view,
            player,
            frames_in_seg,
            gen,
            danmaku,
            danmaku_view,
            status,
            status_detail,
            loading,
            detail_from_uploader: false,
            scroll_tap: None,
            detail_bvid,
            detail_request,
            detail_display,
            play_request,
            current_play,
            requested_play,
            requested_uploader,
            requested_login,
            login_request,
            login_uploader,
            dynamic_offset: String::new(),
            dynamic_request: 0,
            following: following::FollowingState::default(),
            following_request: 0,
            requested_following: false,
            login_following: false,
            uploader,
            uploader_request,
            uploader_dirty,
            back_stack,
            first_row,
            grid_scroll,
            scroll_context,
            about_scroll,
            requested_detail,
            detail_scroll,
            detail_maxv,
            marq_t,
            last_tick,
            deadline,
            quit,
            seek_pending,
            seek_deadline,
            esc_press,
            bar_until,
            last_input,
            left_hold,
            right_hold,
            left_rep,
            right_rep,
            detail_hold,
            detail_repeat,
            latest,
            settings_changed,
            go_back,
        }
    }
}
