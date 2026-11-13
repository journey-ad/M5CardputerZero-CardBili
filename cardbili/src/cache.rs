use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::net;

// 接口数据缓存有效期，单位为秒
pub const TTL_LIST: u64 = 60;
pub const TTL_DETAIL: u64 = 120;
pub const TTL_PLAYURL: u64 = 120;
pub const TTL_REPLY: u64 = 120;

// 缩略图缓存文件上限
const IMG_MAX: usize = 256;
// 接口缓存文件上限
const API_MAX: usize = 512;

static SEQ: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static FORCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// 当前线程的强制刷新开关
pub fn set_force(v: bool) {
    FORCE.with(|f| f.set(v));
}

fn force_on() -> bool {
    FORCE.with(|f| f.get())
}

// 缓存预览阶段可接受的数据最大年龄，单位为秒
const STALE_MAX: u64 = 86400;

thread_local! {
    static CACHED_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// 当前线程读取缓存预览数据的开关
pub fn set_cached_only(v: bool) {
    CACHED_ONLY.with(|f| f.set(v));
}

fn cached_only_on() -> bool {
    CACHED_ONLY.with(|f| f.get())
}

fn base() -> PathBuf {
    std::env::temp_dir().join("cardbili-cache")
}

fn api_dir() -> PathBuf {
    base().join("api")
}

fn img_dir() -> PathBuf {
    base().join("img")
}

pub fn init() {
    let _ = fs::create_dir_all(api_dir());
    let _ = fs::create_dir_all(img_dir());
}

// 缓存键：对原始字符串取 MD5
fn key_of(input: &str) -> String {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(input.as_bytes());
    format!("{:x}", h.finalize())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn mtime(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// 原子写入：完整写入临时文件后再重命名
fn write_atomic(path: &Path, data: &[u8]) {
    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(".{}.{}.tmp", name, n));
    if fs::write(&tmp, data)
        .and_then(|_| fs::rename(&tmp, path))
        .is_err()
    {
        let _ = fs::remove_file(&tmp);
    }
}

// 图片缓存以最后访问时间决定淘汰顺序
fn touch(path: &Path) {
    if let Ok(f) = fs::OpenOptions::new().write(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

// 文件数超过上限时，删除修改时间最早的文件
fn enforce_limit(dir: &Path, max: usize) {
    let mut items: Vec<(PathBuf, u64)> = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            let is_data = matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("json") | Some("raw")
            );
            if is_data {
                let t = mtime(&p);
                items.push((p, t));
            }
        }
    }
    if items.len() <= max {
        return;
    }
    items.sort_by_key(|(_, t)| *t);
    let remove = items.len() - max;
    for (p, _) in items.into_iter().take(remove) {
        let _ = fs::remove_file(p);
    }
}

// JSON 缓存以写入时间计算有效期
fn read_cached(path: &Path, max_age: u64) -> Option<Value> {
    if now().saturating_sub(mtime(path)) > max_age {
        return None;
    }
    let data = fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

// 带缓存的接口请求
pub fn api_json(url: &str, cookies: Option<&str>, ttl: u64) -> Result<Value, String> {
    api_json_with(url, cookies, ttl, || net::curl_get(url, cookies))
}

// 缓存键使用业务参数，请求签名在缓存未命中时生成
pub fn api_json_with(
    cache_key: &str,
    cookies: Option<&str>,
    ttl: u64,
    fetch: impl FnOnce() -> Result<String, String>,
) -> Result<Value, String> {
    init();
    // 缓存按账号隔离：登录时以 mid 作为业务标识，避免不同账号共用同一接口缓存
    let tag = match cookies {
        Some(c) if !c.is_empty() => format!("u:{}", crate::util::parse_mid(c)),
        _ => "a".to_string(),
    };
    let path = api_dir().join(format!(
        "{}.json",
        key_of(&format!("{}\n{}", cache_key, tag))
    ));
    // 强制刷新优先于缓存预览，普通请求按有效期读取
    if !force_on() {
        let max_age = if cached_only_on() { STALE_MAX } else { ttl };
        if let Some(v) = read_cached(&path, max_age) {
            return Ok(v);
        }
        if cached_only_on() {
            return Err("缓存未命中".to_string());
        }
    }
    let s = fetch()?;
    let v: Value = serde_json::from_str(&s).map_err(|e| {
        format!(
            "数据解析失败（{e}）：{}",
            s.chars().take(200).collect::<String>()
        )
    })?;
    if v["code"].as_i64() == Some(0) {
        write_atomic(&path, s.as_bytes());
        enforce_limit(&api_dir(), API_MAX);
    }
    Ok(v)
}

// 带缓存的封面：命中直接返回已缩放像素，未命中下载并回填
pub fn cover(pic: &str) -> Option<(u32, u32, Vec<u8>)> {
    init();
    let mut url = pic.to_string();
    if let Some(p) = url.find('@') {
        url.truncate(p);
    }
    if url.starts_with("//") {
        url = format!("https:{}", url);
    } else if url.starts_with("http://") {
        url = url.replacen("http://", "https://", 1);
    }
    if url.is_empty() {
        return None;
    }
    let path = img_dir().join(format!("{}.raw", key_of(&url)));
    if let Ok(data) = fs::read(&path) {
        if data.len() >= 8 {
            let w = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            let h = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
            touch(&path);
            return Some((w, h, data[8..].to_vec()));
        }
    }
    let bytes = net::curl_bytes(&url).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = img.thumbnail(180, 120).to_rgb8();
    let (w, h) = img.dimensions();
    let raw = img.into_raw();
    let mut buf = Vec::with_capacity(8 + raw.len());
    buf.extend_from_slice(&w.to_le_bytes());
    buf.extend_from_slice(&h.to_le_bytes());
    buf.extend_from_slice(&raw);
    write_atomic(&path, &buf);
    enforce_limit(&img_dir(), IMG_MAX);
    Some((w, h, raw))
}

pub fn invalidate_api(url: &str, cookies: &str) {
    let tag = format!("u:{}", crate::util::parse_mid(cookies));
    let path = api_dir().join(format!("{}.json", key_of(&format!("{url}\n{tag}"))));
    let _ = fs::remove_file(path);
}
