use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::net;

// 接口数据缓存有效期（秒）
pub const TTL_LIST: u64 = 300;
pub const TTL_DETAIL: u64 = 600;
pub const TTL_PLAYURL: u64 = 300;
pub const TTL_REPLY: u64 = 300;

// 缩略图缓存文件上限
const IMG_MAX: usize = 256;
// 接口缓存文件上限
const API_MAX: usize = 512;

static SEQ: AtomicUsize = AtomicUsize::new(0);

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
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn mtime(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// 原子写入：先写临时文件再改名，避免并发读到半截内容
fn write_atomic(path: &Path, data: &[u8]) {
    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_file_name(format!(".{}.{}.tmp", name, n));
    if fs::write(&tmp, data).is_ok() && fs::rename(&tmp, path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

// 命中缓存时刷新修改时间，作为 LRU 依据
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
            let is_data = matches!(p.extension().and_then(|x| x.to_str()), Some("json") | Some("raw"));
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

// 带缓存的接口请求：ttl 秒内直接返回本地 JSON
pub fn api_json(url: &str, cookies: Option<&str>, ttl: u64) -> Result<Value, String> {
    init();
    let tag = if cookies.map(|c| !c.is_empty()).unwrap_or(false) { "u" } else { "a" };
    let path = api_dir().join(format!("{}.json", key_of(&format!("{}\n{}", url, tag))));
    if let Ok(s) = fs::read_to_string(&path) {
        if now().saturating_sub(mtime(&path)) <= ttl {
            if let Ok(v) = serde_json::from_str::<Value>(&s) {
                touch(&path);
                return Ok(v);
            }
        }
    }
    let s = net::curl_get(url, cookies)?;
    let v: Value = serde_json::from_str(&s)
        .map_err(|e| format!("数据解析失败（{e}）：{}", s.chars().take(200).collect::<String>()))?;
    write_atomic(&path, s.as_bytes());
    enforce_limit(&api_dir(), API_MAX);
    Ok(v)
}

// 带缓存的封面：命中直接返回已缩放像素，未命中下载并回填
pub fn cover(pic: &str) -> Option<(u32, u32, Vec<u8>)> {
    init();
    let mut url = pic.to_string();
    if let Some(p) = url.find('@') { url.truncate(p); }
    if url.starts_with("//") { url = format!("https:{}", url); }
    else if url.starts_with("http://") { url = url.replacen("http://", "https://", 1); }
    if url.is_empty() { return None; }
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
