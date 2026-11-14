use slint::ComponentHandle as _;
use super::*;

pub(super) struct App {
    pub(super) volume_queue: mpsc::SyncSender<i32>,
    pub(super) history: history::History,
    pub(super) controls: playback::Controls,
    pub(super) comment_page: comments::Comments,
    pub(super) sections: sections::Sections,
    pub(super) requested_cid: Option<i64>,
    pub(super) play_start: f64,
    pub(super) comments_request: u64,
    pub(super) requested_comments: Option<i64>,
    pub(super) login_return: bool,
    pub(super) ui: AppWindow,
    pub(super) tx: mpsc::Sender<Msg>,
    pub(super) cpath: String,
    pub(super) cookies: Option<String>,
    pub(super) mid: String,
    pub(super) ph: slint::Image,
    pub(super) page: Page,
    pub(super) tab: HomeTab,
    pub(super) settings: Settings,
    pub(super) cache_checked: Option<Instant>,
    pub(super) confirmation: Option<confirmation::Action>,
    pub(super) cdn_request: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub(super) view: VideoLayout,
    pub(super) help: bool,
    pub(super) sel: i32,
    pub(super) lists: Vec<Vec<model::Item>>,
    pub(super) covers: Vec<Vec<Option<slint::Image>>>,
    pub(super) covers_started: [bool; 5],
    pub(super) loaded: [bool; 5],
    pub(super) pages: [usize; 5],
    pub(super) first_page_sizes: [usize; 5],
    pub(super) has_more: [bool; 5],
    pub(super) loading_more: [bool; 5],
    pub(super) mine_detail: i32,
    pub(super) acct: Option<model::Account>,
    pub(super) avatar_img: Option<slint::Image>,
    pub(super) acct_loaded: bool,
    pub(super) account_dirty: bool,
    pub(super) home_dirty: bool,
    pub(super) home_view: Option<(usize, VideoLayout, Page, usize, MenuPosition)>,
    pub(super) menu_view: Option<[Option<menu::Action>; 5]>,
    pub(super) player: Option<player::Player>,
    pub(super) frames_in_seg: u64,
    pub(super) gen: u64,
    pub(super) danmaku: Option<danmaku::Engine>,
    pub(super) danmaku_view: DanmakuView,
    pub(super) status: String,
    pub(super) status_detail: String,
    pub(super) loading: bool,
    pub(super) scroll_tap: Option<(f64, Instant)>,
    pub(super) detail_bvid: String,
    pub(super) detail_request: u64,
    pub(super) detail_display: Option<DetailDisplay>,
    pub(super) play_request: u64,
    pub(super) current_play: Option<model::PlayInfo>,
    pub(super) requested_play: Option<String>,
    pub(super) requested_uploader: Option<model::Owner>,
    pub(super) requested_related: Option<String>,
    pub(super) requested_login: bool,
    pub(super) login_request: u64,
    pub(super) login_uploader: Option<model::Owner>,
    pub(super) history_cursor: model::HistoryCursor,
    pub(super) history_request: u64,
    pub(super) dynamic_offset: String,
    pub(super) dynamic_request: u64,
    pub(super) following: following::FollowingState,
    pub(super) following_request: u64,
    pub(super) requested_following: bool,
    pub(super) login_following: bool,
    pub(super) favorites: favorites::Favorites,
    pub(super) requested_favorites: bool,
    pub(super) requested_folder: Option<model::FavoriteFolder>,
    pub(super) requested_home_list: Option<usize>,
    pub(super) requested_about: bool,
    pub(super) login_favorites: bool,
    pub(super) collection: CollectionState,
    pub(super) collection_request: u64,
    pub(super) collection_dirty: bool,
    pub(super) back_stack: Vec<BackTarget>,
    pub(super) first_row: usize,
    pub(super) grid_scroll: f64,
    pub(super) scroll_context: Option<(i32, VideoLayout, Page, MenuPosition)>,
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
    pub(super) direction_hold: Option<input::DirectionHold>,
    pub(super) return_hold: Option<(usize, Instant)>,
    pub(super) back_taps: (u8, Option<Instant>),
    pub(super) back_hint: bool,
    pub(super) return_home: bool,
    pub(super) latest: Option<(u64, Vec<u8>)>,
    pub(super) settings_changed: bool,
    pub(super) go_back: bool,
}

impl App {
    pub(super) fn new(ui: AppWindow, tx: mpsc::Sender<Msg>, settings: Settings) -> Self {
        let cpath = util::cookie_path();
        let cookies: Option<String> = std::fs::read_to_string(&cpath)
            .inspect_err(|error| {
                crate::diagnostics::write(format_args!("[登录] 读取登录态失败: {error}"));
            })
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        crate::diagnostics::write(format_args!(
            "[登录] 已读取登录态 登录={}",
            cookies.is_some()
        ));
        let mid = cookies.as_deref().map(util::parse_mid).unwrap_or_default();

        ui.global::<crate::Meta>().set_version(format!("V{}", env!("CARGO_PKG_VERSION")).into());
        ui.global::<crate::Meta>().set_about_text("作者  journey-ad\n\n项目地址\ngithub.com/journey-ad/CardBili\n\n字体\nFusion Pixel Font\ngithub.com/TakWolf/fusion-pixel-font\n\n接口参考\nPiliPlus\ngithub.com/bggRGjQaUbCoE/PiliPlus\n\n图标\nPixelarticons\ngithub.com/halfmage/pixelarticons".into());

        let ph = util::make_placeholder();
        let mut page = Page::Home;
        let tab = std::env::var("BILI_TAB")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .map(HomeTab::from_index)
            .unwrap_or(HomeTab::Recommended);
        let mut view = std::env::var("BILI_VIEW")
            .ok()
            .and_then(|s| s.parse::<i32>().ok())
            .map(VideoLayout::from_index)
            .unwrap_or(settings.list_view);
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
        let loading = tab.is_video();

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
        let collection = CollectionState::default();
        let collection_request = 0u64;
        let collection_dirty = false;
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
        if tab.is_video() {
            tasks::load_list(tab.index(), cookies.clone(), tx.clone(), force0);
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
            page = Page::About;
        }
        if std::env::var("BILI_TEST_LOGIN").is_ok() {
            page = Page::Login;
            view = VideoLayout::List;
            ui.global::<crate::Login>().set_has_qr(false);
            ui.global::<crate::Login>().set_qr_status("正在生成二维码…".into());
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
        let latest = None;
        let settings_changed = false;
        let go_back = false;
        Self {
            volume_queue: player::volume::worker(tx.clone()),
            history: history::History::new(settings.record_history),
            controls: playback::Controls::default(),
            comment_page: comments::Comments::default(),
            sections: sections::Sections::default(),
            requested_cid: None,
            play_start: std::env::var("BILI_START")
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .unwrap_or(0.0),
            comments_request: 0,
            requested_comments: None,
            login_return: false,
            ui,
            tx,
            cpath,
            cookies,
            mid,
            ph,
            page,
            tab,
            settings,
            cache_checked: None,
            confirmation: None,
            cdn_request: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            view,
            help,
            sel,
            lists,
            covers,
            covers_started,
            loaded,
            pages,
            first_page_sizes: [0; 5],
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
            scroll_tap: None,
            detail_bvid,
            detail_request,
            detail_display,
            play_request,
            current_play,
            requested_play,
            requested_uploader,
            requested_related: None,
            requested_login,
            login_request,
            login_uploader,
            history_cursor: model::HistoryCursor::default(),
            history_request: 0,
            dynamic_offset: String::new(),
            dynamic_request: 0,
            following: following::FollowingState::default(),
            following_request: 0,
            requested_following: false,
            login_following: false,
            favorites: favorites::Favorites::default(),
            requested_favorites: false,
            requested_folder: None,
            requested_home_list: None,
            requested_about: false,
            login_favorites: false,
            collection,
            collection_request,
            collection_dirty,
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
            direction_hold: None,
            return_hold: None,
            back_taps: (0, None),
            back_hint: false,
            return_home: false,
            latest,
            settings_changed,
            go_back,
        }
    }
}
