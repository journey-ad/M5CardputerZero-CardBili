use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::{diagnostics, net};

// 接口数据缓存有效期，单位为秒
pub const TTL_LIST: u64 = 60;
pub const TTL_DETAIL: u64 = 120;
pub const TTL_PLAYURL: u64 = 120;
pub const TTL_REPLY: u64 = 120;
pub const TTL_ACCOUNT: u64 = 86400;

pub type Pixels = (u32, u32, Vec<u8>);

// 接口和图片共用 1 GiB 缓存空间
pub const MAX_BYTES: u64 = 1024 * 1024 * 1024;

static INIT: Once = Once::new();
static SIZE: Mutex<Option<u64>> = Mutex::new(None);

static SEQ: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static FORCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// 当前线程的强制刷新开关
pub fn set_force(v: bool) {
    FORCE.with(|f| f.set(v));
    diagnostics::write(format_args!("[缓存] 强制刷新={v}"));
}

fn force_on() -> bool {
    FORCE.with(|f| f.get())
}

// 缓存预览阶段可接受的数据最大年龄，单位为秒
const STALE_MAX: u64 = 7 * 86400;

thread_local! {
    static CACHED_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// 当前线程读取缓存预览数据的开关
pub fn set_cached_only(v: bool) {
    CACHED_ONLY.with(|f| f.set(v));
    diagnostics::write(format_args!("[缓存] 缓存预览={v}"));
}

fn cached_only_on() -> bool {
    CACHED_ONLY.with(|f| f.get())
}

fn base() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| "/var/tmp".into()))
        .join(".cache/cardbili")
}

fn api_dir() -> PathBuf {
    base().join("api")
}

fn img_dir() -> PathBuf {
    base().join("img")
}

pub fn init() {
    INIT.call_once(|| {
        for directory in [api_dir(), img_dir()] {
            if let Err(error) = fs::create_dir_all(&directory) {
                diagnostics::write(format_args!(
                    "[缓存] 创建目录失败 {}: {error}",
                    directory.display()
                ));
            }
        }
        let old = std::env::temp_dir().join("cardbili-cache");
        if old != base() && old.exists() {
            let result = migrate(&old);
            diagnostics::write(format_args!(
                "[缓存] 迁移 {} → {} 结果={result:?}",
                old.display(),
                base().display()
            ));
        }
        let result = (|| -> std::io::Result<u64> {
            let mut total = entries()?.iter().map(|(_, m)| m.len()).sum();
            enforce_limit(&mut total)?;
            Ok(total)
        })();
        match result {
            Ok(bytes) => *SIZE.lock().unwrap() = Some(bytes),
            Err(error) => diagnostics::write(format_args!("[缓存] 初始化容量失败: {error}")),
        }
        diagnostics::write(format_args!(
            "[缓存] 目录={} 容量上限={MAX_BYTES}",
            base().display()
        ));
    });
}

fn migrate(old: &Path) -> std::io::Result<()> {
    for name in ["api", "img"] {
        let directory = old.join(name);
        if !directory.exists() {
            continue;
        }
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if !metadata.is_file() {
                continue;
            }
            let target = base().join(name).join(entry.file_name());
            let modified = metadata.modified()?;
            let newer = fs::metadata(&target)
                .and_then(|m| m.modified())
                .is_ok_and(|time| time >= modified);
            if !newer {
                let temporary = target.with_extension("migrating");
                let result = (|| -> std::io::Result<()> {
                    fs::copy(entry.path(), &temporary)?;
                    fs::File::options()
                        .write(true)
                        .open(&temporary)?
                        .set_modified(modified)?;
                    fs::rename(&temporary, &target)
                })();
                if result.is_err() {
                    let _ = fs::remove_file(&temporary);
                }
                result?;
            }
            fs::remove_file(entry.path())?;
        }
        fs::remove_dir(directory)?;
    }
    fs::remove_dir(old)
}

fn entries() -> std::io::Result<Vec<(PathBuf, fs::Metadata)>> {
    let mut files = Vec::new();
    for directory in [api_dir(), img_dir()] {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                files.push((entry.path(), metadata));
            }
        }
    }
    Ok(files)
}

pub fn usage() -> std::io::Result<u64> {
    init();
    let mut size = SIZE.lock().unwrap();
    match *size {
        Some(bytes) => Ok(bytes),
        None => {
            let mut bytes = entries()?.iter().map(|(_, m)| m.len()).sum();
            enforce_limit(&mut bytes)?;
            *size = Some(bytes);
            Ok(bytes)
        }
    }
}

pub fn clear() -> std::io::Result<()> {
    init();
    let mut size = SIZE.lock().unwrap();
    let result = entries().and_then(|files| {
        for (path, _) in files {
            fs::remove_file(path)?;
        }
        Ok(())
    });
    *size = if result.is_ok() { Some(0) } else { None };
    diagnostics::write(format_args!("[缓存] 清理结果={result:?}"));
    result
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
    let mut size = SIZE.lock().unwrap();
    let result = (|| -> std::io::Result<()> {
        let total = match *size {
            Some(bytes) => bytes,
            None => entries()?.iter().map(|(_, m)| m.len()).sum(),
        };
        let previous = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let name = path.file_name().unwrap().to_string_lossy();
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()));
        if let Err(error) = fs::write(&tmp, data).and_then(|_| fs::rename(&tmp, path)) {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
        let mut total = total.saturating_sub(previous) + data.len() as u64;
        enforce_limit(&mut total)?;
        *size = Some(total);
        Ok(())
    })();
    match result {
        Ok(()) => diagnostics::write(format_args!(
            "[缓存] 已写入 {} 字节={}",
            path.display(),
            data.len()
        )),
        Err(error) => {
            *size = None;
            diagnostics::write(format_args!("[缓存] 写入失败 {}: {error}", path.display()));
        }
    }
}

// 图片缓存以最后访问时间决定淘汰顺序
fn touch(path: &Path) {
    if let Ok(f) = fs::OpenOptions::new().write(true).open(path) {
        let _ = f.set_modified(SystemTime::now());
    }
}

// 容量超过上限时，按修改时间淘汰最旧的缓存文件
fn enforce_limit(total: &mut u64) -> std::io::Result<()> {
    if *total <= MAX_BYTES {
        return Ok(());
    }
    let mut files = entries()?;
    files.sort_by_key(|(_, metadata)| metadata.modified().unwrap_or(UNIX_EPOCH));
    for (path, metadata) in files {
        if *total <= MAX_BYTES {
            break;
        }
        fs::remove_file(&path)?;
        *total = total.saturating_sub(metadata.len());
        diagnostics::write(format_args!(
            "[缓存] 淘汰 {} 字节={} 剩余={total}",
            path.display(),
            metadata.len()
        ));
    }
    Ok(())
}

// JSON 缓存以写入时间计算有效期
fn read_cached(path: &Path, max_age: u64) -> Option<Value> {
    if now().saturating_sub(mtime(path)) > max_age {
        return None;
    }
    let data = fs::read(path)
        .map_err(|error| {
            diagnostics::write(format_args!("[缓存] 读取失败 {}: {error}", path.display()));
        })
        .ok()?;
    serde_json::from_slice(&data)
        .map_err(|error| {
            diagnostics::write(format_args!(
                "[缓存] JSON 解析失败 {}: {error}",
                path.display()
            ));
        })
        .ok()
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
    let endpoint = cache_key.split(['?', '#']).next().unwrap_or(cache_key);
    // 强制刷新优先于缓存预览，普通请求按有效期读取
    if !force_on() {
        let max_age = if cached_only_on() { STALE_MAX } else { ttl };
        if let Some(v) = read_cached(&path, max_age) {
            diagnostics::write(format_args!(
                "[缓存] 接口命中 {endpoint} {} 有效期={max_age}s",
                path.display()
            ));
            return Ok(v);
        }
        diagnostics::write(format_args!(
            "[缓存] 接口未命中 {endpoint} {} 有效期={max_age}s",
            path.display()
        ));
        if cached_only_on() {
            return Err("缓存未命中".to_string());
        }
    } else {
        diagnostics::write(format_args!(
            "[缓存] 接口强制刷新 {endpoint} {}",
            path.display()
        ));
    }
    let s = fetch()?;
    let v = net::parse_json(cache_key, &s).map_err(|e| {
        format!(
            "数据解析失败（{e}）：{}",
            s.chars().take(200).collect::<String>()
        )
    })?;
    if v["code"].as_i64() == Some(0) {
        write_atomic(&path, s.as_bytes());
    }
    Ok(v)
}

// 带缓存的封面：命中直接返回已缩放像素，未命中下载并回填
pub fn cover(pic: &str) -> Option<Pixels> {
    cached_image(pic, 180, 120)
}

pub fn avatar(url: &str) -> Option<Pixels> {
    cached_image(url, 64, 64)
}

fn cached_image(pic: &str, width: u32, height: u32) -> Option<Pixels> {
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
    let path = img_dir().join(format!(
        "{}.raw",
        key_of(&format!("{url}@{width}x{height}"))
    ));
    if let Ok(data) = fs::read(&path) {
        if data.len() >= 8 {
            let w = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            let h = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
            touch(&path);
            diagnostics::write(format_args!(
                "[缓存] 图片命中 {} 尺寸={w}x{h}",
                path.display()
            ));
            return Some((w, h, data[8..].to_vec()));
        }
    }
    diagnostics::write(format_args!(
        "[缓存] 图片未命中 {} 尺寸={width}x{height}",
        path.display()
    ));
    let bytes = net::curl_bytes(&url).ok()?;
    let img = image::load_from_memory(&bytes)
        .map_err(|error| {
            diagnostics::write(format_args!(
                "[缓存] 图片解码失败 {}: {error}",
                path.display()
            ));
        })
        .ok()?;
    let img = img.thumbnail(width, height).to_rgb8();
    let (w, h) = img.dimensions();
    let raw = img.into_raw();
    let mut buf = Vec::with_capacity(8 + raw.len());
    buf.extend_from_slice(&w.to_le_bytes());
    buf.extend_from_slice(&h.to_le_bytes());
    buf.extend_from_slice(&raw);
    write_atomic(&path, &buf);
    Some((w, h, raw))
}

pub fn invalidate_api(url: &str, cookies: &str) {
    init();
    let tag = format!("u:{}", crate::util::parse_mid(cookies));
    let path = api_dir().join(format!("{}.json", key_of(&format!("{url}\n{tag}"))));
    let mut size = SIZE.lock().unwrap();
    let bytes = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let result = fs::remove_file(&path);
    if result.is_ok() {
        if let Some(total) = size.as_mut() {
            *total = total.saturating_sub(bytes);
        }
    }
    diagnostics::write(format_args!(
        "[缓存] 失效 {} 结果={result:?}",
        path.display()
    ));
}
