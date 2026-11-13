use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use md5::{Md5, Digest};

pub const UA: &str = "Mozilla/5.0 (X11; Linux aarch64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const REFERER: &str = "https://www.bilibili.com/";

pub fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn curl_args(url: &str, cookies: Option<&str>, jar: Option<&str>) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-s".into(), "-L".into(), "--max-time".into(), "25".into(),
        "-A".into(), UA.into(), "-e".into(), REFERER.into(),
        "-H".into(), "Origin: https://www.bilibili.com".into(),
        "-H".into(), "Accept: application/json, text/plain, */*".into(),
    ];
    if let Some(ck) = cookies {
        if !ck.is_empty() { a.push("-H".into()); a.push(format!("Cookie: {}", ck)); }
    }
    if let Some(j) = jar {
        a.push("-c".into()); a.push(j.into());
        a.push("-b".into()); a.push(j.into());
    }
    a.push(url.into());
    a
}

fn tidy(s: &str) -> String {
    s.trim().replace("curl: ", "").replace("curl:", "").trim().to_string()
}

fn curl_out(url: &str, cookies: Option<&str>, jar: Option<&str>) -> Result<String, String> {
    let out = Command::new("curl").args(curl_args(url, cookies, jar)).output()
        .map_err(|e| format!("网络请求启动失败（{e}）"))?;
    if !out.status.success() {
        return Err(format!("网络请求失败（状态 {:?}）：{}", out.status, tidy(&String::from_utf8_lossy(&out.stderr))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn curl_get(url: &str, cookies: Option<&str>) -> Result<String, String> {
    curl_out(url, cookies, None)
}

pub fn curl_bytes(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl").args(curl_args(url, None, None)).output()
        .map_err(|e| format!("网络请求启动失败（{e}）"))?;
    if !out.status.success() {
        return Err(format!("网络请求失败（状态 {:?}）", out.status));
    }
    Ok(out.stdout)
}

pub fn get_json(url: &str, cookies: Option<&str>) -> Result<serde_json::Value, String> {
    let s = curl_get(url, cookies)?;
    serde_json::from_str(&s).map_err(|e| format!("数据解析失败（{e}）：{}", s.chars().take(200).collect::<String>()))
}

fn pct(s: &str) -> String {
    let mut o = String::new();
    for &b in s.as_bytes() {
        let c = b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            o.push(c);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

const MIXIN_TAB: [usize; 32] = [46,47,18,2,53,8,23,32,15,50,10,31,58,3,45,35,27,43,5,49,33,9,42,19,29,28,14,39,12,38,41,13];

fn stem(u: &str) -> String {
    let f = u.rsplit('/').next().unwrap_or(u);
    f.split('.').next().unwrap_or(f).to_string()
}

pub fn mixin_key(img: &str, sub: &str) -> String {
    let orig = format!("{}{}", stem(img), stem(sub));
    let bytes = orig.as_bytes();
    MIXIN_TAB.iter().filter_map(|&i| bytes.get(i).map(|&b| b as char)).collect()
}

pub fn fetch_mixin() -> Result<String, String> {
    let v = get_json("https://api.bilibili.com/x/web-interface/nav", None)?;
    let wi = &v["data"]["wbi_img"];
    let img = wi["img_url"].as_str().ok_or("签名信息不完整（缺少 img_url）")?;
    let sub = wi["sub_url"].as_str().ok_or("签名信息不完整（缺少 sub_url）")?;
    Ok(mixin_key(img, sub))
}

/// 对 params 做 WBI 签名，返回带 wts/w_rid 的新参数表。
pub fn wbi_sign(params: &[(String, String)], mk: &str) -> Vec<(String, String)> {
    let mut p: Vec<(String, String)> = params.to_vec();
    p.push(("wts".into(), now_secs().to_string()));
    p.sort_by(|a, b| a.0.cmp(&b.0));
    let q = p.iter().map(|(k, v)| {
        let vf: String = v.chars().filter(|c| !"!'()*".contains(*c)).collect();
        format!("{}={}", pct(k), pct(&vf))
    }).collect::<Vec<_>>().join("&");
    let mut h = Md5::new();
    h.update(q.as_bytes());
    h.update(mk.as_bytes());
    let rid = format!("{:x}", h.finalize());
    p.push(("w_rid".into(), rid));
    p
}

pub fn query_string(params: &[(String, String)]) -> String {
    params.iter().map(|(k, v)| format!("{}={}", pct(k), pct(v))).collect::<Vec<_>>().join("&")
}

// ---------- QR login ----------
pub fn qr_generate() -> Result<(String, String), String> {
    let v = get_json("https://passport.bilibili.com/x/passport-login/web/qrcode/generate", None)?;
    if v["code"].as_i64() != Some(0) { return Err(format!("二维码生成失败（code={} {}）", v["code"], v["message"])); }
    let d = &v["data"];
    let url = d["url"].as_str().unwrap_or("").to_string();
    let key = d["qrcode_key"].as_str().unwrap_or("").to_string();
    if url.is_empty() || key.is_empty() { return Err("二维码生成失败（返回为空）".into()); }
    Ok((url, key))
}

pub enum Poll {
    Success(String),
    Scanned,
    Pending,
    Expired,
    Err(String),
}

const WANT_CK: [&str; 5] = ["SESSDATA", "bili_jct", "DedeUserID", "DedeUserID__ckMd5", "buvid3"];

fn put_ck(map: &mut Vec<(String, String)>, k: &str, v: &str) {
    let k = k.trim();
    let v = v.trim();
    if k.is_empty() || v.is_empty() || !WANT_CK.contains(&k) { return; }
    if let Some(e) = map.iter_mut().find(|(n, _)| n.as_str() == k) { e.1 = v.to_string(); }
    else { map.push((k.to_string(), v.to_string())); }
}

fn read_jar(jar: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(txt) = std::fs::read_to_string(jar) {
        for raw in txt.lines() {
            let line = raw.strip_prefix("#HttpOnly_").unwrap_or(raw);
            if line.trim().is_empty() || line.starts_with('#') { continue; }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 7 { put_ck(&mut out, f[5], f[6]); }
        }
    }
    out
}

fn cookies_from_url(url: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(q) = url.split('?').nth(1) {
        for kv in q.split('&') {
            if let Some((k, v)) = kv.split_once('=') { put_ck(&mut out, k, v); }
        }
    }
    out
}

fn parse_set_cookie(hdr: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in hdr.lines() {
        let b = line.as_bytes();
        if b.len() >= 11 && b[..11].eq_ignore_ascii_case(b"set-cookie:") {
            let first = line[11..].split(';').next().unwrap_or("");
            if let Some((k, v)) = first.split_once('=') { put_ck(&mut out, k, v); }
        }
    }
    out
}

fn join_ck(map: &[(String, String)]) -> String {
    map.iter().map(|(k, v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("; ")
}

pub fn qr_poll(key: &str, jar: &str) -> Poll {
    let url = format!("https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}", pct(key));
    let hdr = format!("{}.hdr", jar);
    let args = vec![
        "-s".to_string(), "-L".into(), "--max-time".into(), "25".into(),
        "-A".into(), UA.into(), "-e".into(), REFERER.into(),
        "-H".into(), "Origin: https://www.bilibili.com".into(),
        "-c".into(), jar.into(), "-b".into(), jar.into(),
        "-D".into(), hdr.clone(), url,
    ];
    let out = match Command::new("curl").args(&args).output() {
        Ok(o) => o,
        Err(e) => return Poll::Err(format!("网络请求启动失败（{e}）")),
    };
    if !out.status.success() {
        return Poll::Err(format!("网络请求失败（状态 {:?}）：{}", out.status, tidy(&String::from_utf8_lossy(&out.stderr))));
    }
    let body = String::from_utf8_lossy(&out.stdout).into_owned();
    let v: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return Poll::Err(format!("登录状态解析失败（{e}）：{}", body.chars().take(160).collect::<String>())),
    };
    let d = &v["data"];
    let code = d["code"].as_i64().unwrap_or(-1);
    match code {
        0 => {
            let mut map: Vec<(String, String)> = Vec::new();
            for (k, val) in cookies_from_url(d["url"].as_str().unwrap_or("")) { put_ck(&mut map, &k, &val); }
            for (k, val) in read_jar(jar) { put_ck(&mut map, &k, &val); }
            let hdrtxt = std::fs::read_to_string(&hdr).unwrap_or_default();
            for (k, val) in parse_set_cookie(&hdrtxt) { put_ck(&mut map, &k, &val); }
            // 缺 SESSDATA：GET crossDomain（会重新下发 Cookie），再读一次 jar
            if !map.iter().any(|(k, _)| k == "SESSDATA") {
                if let Some(u) = d["url"].as_str() {
                    if !u.is_empty() {
                        let h2 = format!("{}.x", jar);
                        let a2 = vec![
                            "-s".to_string(), "-L".into(), "--max-time".into(), "20".into(),
                            "-A".into(), UA.into(), "-e".into(), REFERER.into(),
                            "-c".into(), jar.into(), "-b".into(), jar.into(),
                            "-D".into(), h2.clone(), u.to_string(),
                        ];
                        let _ = Command::new("curl").args(&a2).output();
                        let t2 = std::fs::read_to_string(&h2).unwrap_or_default();
                        for (k, val) in parse_set_cookie(&t2) { put_ck(&mut map, &k, &val); }
                    }
                }
                for (k, val) in read_jar(jar) { put_ck(&mut map, &k, &val); }
            }
            if std::env::var("BILI_LOGIN_DEBUG").is_ok() {
                eprintln!("LOGIN url={}", d["url"].as_str().unwrap_or(""));
                eprintln!("LOGIN hdr={}", hdrtxt.chars().take(600).collect::<String>());
            }
            Poll::Success(join_ck(&map))
        }
        86101 => Poll::Pending,
        86090 => Poll::Scanned,
        86038 => Poll::Expired,
        other => Poll::Err(format!("登录状态异常（code={} {}）", other, d["message"])),
    }
}
