#[derive(Clone, Default)]
pub struct Item {
    pub title: String,
    pub meta: String,
    pub bvid: String,
    pub pic: String,
    pub duration: String,
    pub views: Option<String>,
    pub likes: Option<String>,
    pub danmaku: Option<String>,
}

#[derive(Clone, Default)]
pub struct Owner {
    pub mid: String,
    pub name: String,
}

#[derive(Clone)]
pub enum VideoSource {
    Author(Owner),
    Related(String),
}

impl Default for VideoSource {
    fn default() -> Self {
        Self::Author(Owner::default())
    }
}

pub struct AuthorProfile {
    pub name: String,
    pub face: String,
    pub level: i64,
    pub followers: i64,
}

pub struct VideoStats {
    pub views: String,
    pub likes: String,
    pub danmaku: String,
}

pub struct VideoPage {
    pub items: Vec<Item>,
    pub has_more: bool,
}

#[derive(Clone, Default)]
pub struct Seg {
    pub url: String,
    pub len: f64,
}

#[derive(Clone, Default)]
pub struct PlayInfo {
    pub aid: i64,
    pub bvid: String,
    pub owner: Owner,
    pub title: String,
    pub segs: Vec<Seg>,
    pub duration: f64,
    pub cid: i64,
}

#[derive(Clone, Default)]
pub struct Account {
    pub logged: bool,
    pub name: String,
    pub mid: String,
    pub level: i64,
    pub vip: String,
    pub face: String,
}

#[derive(Clone, Default)]
pub struct Comment {
    pub user: String,
    pub text: String,
}

#[derive(Clone, Default)]
pub struct Detail {
    pub title: String,
    pub pic: String,
    pub duration: f64,
    pub owner: Owner,
    pub views: i64,
    pub likes: i64,
    pub coins: i64,
    pub favorites: i64,
    pub danmaku: i64,
    pub desc: String,
    pub comments: Vec<Comment>,
}

pub fn mmss(sec: f64) -> String {
    let s = if sec.is_finite() && sec > 0.0 {
        sec as i64
    } else {
        0
    };
    let h = s / 3600;
    let m = (s % 3600) / 60;
    let ss = s % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, ss)
    } else {
        format!("{}:{:02}", m, ss)
    }
}

pub fn dur_of(v: &serde_json::Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0)
}

pub fn fmt_num(n: i64) -> String {
    if n >= 100_000_000 {
        format!("{:.1}亿", n as f64 / 1e8)
    } else if n >= 10_000 {
        format!("{:.1}万", n as f64 / 1e4)
    } else {
        n.to_string()
    }
}

#[derive(Clone, Copy)]
pub enum VideoAction {
    Like(bool),
    Triple,
}

pub struct ReactionResult {
    pub triple_complete: bool,
    pub liked: bool,
    pub message: String,
}

pub struct CommentPage {
    pub items: Vec<Comment>,
    pub has_more: bool,
}

pub struct DynamicPage {
    pub items: Vec<Item>,
    pub offset: String,
    pub has_more: bool,
}

pub struct Following {
    pub owner: Owner,
    pub signature: String,
}

pub struct FollowingPage {
    pub items: Vec<Following>,
    pub has_more: bool,
}
