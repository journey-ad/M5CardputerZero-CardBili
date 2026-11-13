#[derive(Clone, Default)]
pub struct Item {
    pub title: String,
    pub meta: String,
    pub bvid: String,
    pub pic: String,
}

#[derive(Clone, Default)]
pub struct Seg {
    pub url: String,
    pub len: f64,
}

#[derive(Clone, Default)]
pub struct PlayInfo {
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
    pub owner: String,
    pub views: i64,
    pub likes: i64,
    pub coins: i64,
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
