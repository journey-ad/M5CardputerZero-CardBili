use crate::net;

pub fn qr_generate() -> Result<(String, String), String> {
    let v = net::get_json(
        "https://passport.bilibili.com/x/passport-login/web/qrcode/generate",
        None,
    )?;
    if v["code"].as_i64() != Some(0) {
        return Err(format!(
            "二维码生成失败\ncode={} {}",
            v["code"], v["message"]
        ));
    }
    let d = &v["data"];
    let url = d["url"].as_str().unwrap_or("").to_string();
    let key = d["qrcode_key"].as_str().unwrap_or("").to_string();
    if url.is_empty() || key.is_empty() {
        return Err("二维码生成失败\n返回为空".into());
    }
    Ok((url, key))
}

pub enum Poll {
    Success(String),
    Scanned,
    Pending,
    Expired,
    Err(String),
}

const WANT_CK: [&str; 5] = [
    "SESSDATA",
    "bili_jct",
    "DedeUserID",
    "DedeUserID__ckMd5",
    "buvid3",
];

fn put_ck(map: &mut Vec<(String, String)>, k: &str, v: &str) {
    let k = k.trim();
    let v = v.trim();
    if k.is_empty() || v.is_empty() || !WANT_CK.contains(&k) {
        return;
    }
    if let Some(e) = map.iter_mut().find(|(n, _)| n.as_str() == k) {
        e.1 = v.to_string();
    } else {
        map.push((k.to_string(), v.to_string()));
    }
}

fn read_jar(jar: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(txt) = std::fs::read_to_string(jar) {
        for raw in txt.lines() {
            let line = raw.strip_prefix("#HttpOnly_").unwrap_or(raw);
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 7 {
                put_ck(&mut out, f[5], f[6]);
            }
        }
    }
    out
}

fn cookies_from_url(url: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(q) = url.split('?').nth(1) {
        for kv in q.split('&') {
            if let Some((k, v)) = kv.split_once('=') {
                put_ck(&mut out, k, v);
            }
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
            if let Some((k, v)) = first.split_once('=') {
                put_ck(&mut out, k, v);
            }
        }
    }
    out
}

fn join_ck(map: &[(String, String)]) -> String {
    map.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn qr_poll(key: &str, jar: &str) -> Poll {
    let url = format!(
        "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?{}",
        net::query_string(&[("qrcode_key".into(), key.into())])
    );
    let hdr = format!("{}.hdr", jar);
    let body = match net::get_with_cookie_jar(&url, jar, &hdr, 25) {
        Ok(body) => body,
        Err(error) => return Poll::Err(error),
    };
    let v = match net::parse_json(&url, &body) {
        Ok(v) => v,
        Err(e) => return Poll::Err(format!("登录状态解析失败\n{e}")),
    };
    let d = &v["data"];
    let code = d["code"].as_i64().unwrap_or(-1);
    crate::diagnostics::write(format_args!("[登录] 二维码状态 code={code}"));
    match code {
        0 => {
            let mut map: Vec<(String, String)> = Vec::new();
            for (k, val) in cookies_from_url(d["url"].as_str().unwrap_or("")) {
                put_ck(&mut map, &k, &val);
            }
            for (k, val) in read_jar(jar) {
                put_ck(&mut map, &k, &val);
            }
            let hdrtxt = std::fs::read_to_string(&hdr).unwrap_or_default();
            for (k, val) in parse_set_cookie(&hdrtxt) {
                put_ck(&mut map, &k, &val);
            }
            // SESSDATA 缺失时请求 crossDomain 获取 Cookie，再读取 Cookie 文件
            if !map.iter().any(|(k, _)| k == "SESSDATA") {
                if let Some(u) = d["url"].as_str() {
                    if !u.is_empty() {
                        let h2 = format!("{}.x", jar);
                        let _ = net::get_with_cookie_jar(u, jar, &h2, 20);
                        let t2 = std::fs::read_to_string(&h2).unwrap_or_default();
                        for (k, val) in parse_set_cookie(&t2) {
                            put_ck(&mut map, &k, &val);
                        }
                    }
                }
                for (k, val) in read_jar(jar) {
                    put_ck(&mut map, &k, &val);
                }
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
        other => Poll::Err(format!("登录状态异常\ncode={} {}", other, d["message"])),
    }
}
