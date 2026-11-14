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

fn join_ck(map: &[(String, String)]) -> String {
    map.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn qr_poll(key: &str, client: &ureq::Agent) -> Poll {
    let url = format!(
        "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?{}",
        net::query_string(&[("qrcode_key".into(), key.into())])
    );
    let body = match net::get_with_agent(client, &url, 25) {
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
            let mut map = cookies_from_url(d["url"].as_str().unwrap_or(""));
            let has_session = map.iter().any(|(name, _)| name == "SESSDATA")
                || client
                    .cookie_jar_lock()
                    .iter()
                    .any(|cookie| cookie.name() == "SESSDATA");
            // crossDomain 响应与重定向中的 Cookie 由登录客户端收集
            if !has_session {
                if let Some(url) = d["url"].as_str().filter(|url| !url.is_empty()) {
                    let _ = net::get_with_agent(client, url, 20);
                }
            }
            for cookie in client.cookie_jar_lock().iter() {
                put_ck(&mut map, cookie.name(), cookie.value());
            }
            Poll::Success(join_ck(&map))
        }
        86101 => Poll::Pending,
        86090 => Poll::Scanned,
        86038 => Poll::Expired,
        other => Poll::Err(format!("登录状态异常\ncode={} {}", other, d["message"])),
    }
}
