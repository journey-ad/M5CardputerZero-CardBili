use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const UA: &str = "Mozilla/5.0 (X11; Linux aarch64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
pub const REFERER: &str = "https://www.bilibili.com/";

pub struct RequestHeaders<'a> {
    pub user_agent: &'a str,
    pub referer: &'a str,
    pub origin: &'a str,
}

const DEFAULT_HEADERS: RequestHeaders<'static> = RequestHeaders {
    user_agent: UA,
    referer: REFERER,
    origin: "https://www.bilibili.com",
};

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn curl_args(url: &str, cookies: Option<&str>, headers: &RequestHeaders<'_>) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "-s".into(),
        "-L".into(),
        "--compressed".into(),
        "--max-time".into(),
        "25".into(),
        "-A".into(),
        headers.user_agent.into(),
        "-e".into(),
        headers.referer.into(),
        "-H".into(),
        format!("Origin: {}", headers.origin),
        "-H".into(),
        "Accept: application/json, text/plain, */*".into(),
    ];
    if let Some(ck) = cookies {
        if !ck.is_empty() {
            a.push("-H".into());
            a.push(format!("Cookie: {}", ck));
        }
    }
    a.push(url.into());
    a
}

fn tidy(s: &str) -> String {
    s.trim()
        .replace("curl: ", "")
        .replace("curl:", "")
        .trim()
        .to_string()
}

pub fn curl_get_with_headers(
    url: &str,
    cookies: Option<&str>,
    headers: &RequestHeaders<'_>,
) -> Result<String, String> {
    curl_text(curl_args(url, cookies, headers))
}

pub fn post_form_json(
    url: &str,
    cookies: &str,
    referer: &str,
    params: &[(String, String)],
) -> Result<serde_json::Value, String> {
    post_form_json_timeout(url, cookies, referer, params, 25)
}

pub fn post_form_json_timeout(
    url: &str,
    cookies: &str,
    referer: &str,
    params: &[(String, String)],
    timeout: u32,
) -> Result<serde_json::Value, String> {
    let headers = RequestHeaders {
        user_agent: UA,
        referer,
        origin: "https://www.bilibili.com",
    };
    let mut args = curl_args(url, Some(cookies), &headers);
    args.extend([
        "--max-time".into(),
        timeout.to_string(),
        "--data".into(),
        query_string(params),
    ]);
    let body = curl_text(args)?;
    serde_json::from_str(&body).map_err(|error| format!("数据解析失败\n{error}"))
}

fn curl_text(args: Vec<String>) -> Result<String, String> {
    let out = Command::new("curl")
        .args(args)
        .output()
        .map_err(|e| format!("网络请求启动失败\n{e}"))?;
    if !out.status.success() {
        let code = out
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".into());
        let t = tidy(&String::from_utf8_lossy(&out.stderr));
        let tail = if t.is_empty() {
            String::new()
        } else {
            format!(" {}", t)
        };
        return Err(format!("网络连接失败\nexit {}{}", code, tail));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn curl_get(url: &str, cookies: Option<&str>) -> Result<String, String> {
    curl_get_with_headers(url, cookies, &DEFAULT_HEADERS)
}

pub fn curl_bytes(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args(curl_args(url, None, &DEFAULT_HEADERS))
        .output()
        .map_err(|e| format!("网络请求启动失败\n{e}"))?;
    if !out.status.success() {
        let code = out
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".into());
        return Err(format!("网络连接失败\nexit {}", code));
    }
    Ok(out.stdout)
}

pub fn get_json(url: &str, cookies: Option<&str>) -> Result<serde_json::Value, String> {
    let s = curl_get(url, cookies)?;
    serde_json::from_str(&s).map_err(|e| format!("数据解析失败\n{e}"))
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

pub fn query_string(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(k, v)| format!("{}={}", pct(k), pct(v)))
        .collect::<Vec<_>>()
        .join("&")
}

pub fn get_with_cookie_jar(
    url: &str,
    jar: &str,
    headers_path: &str,
    timeout: u32,
) -> Result<String, String> {
    let mut args = curl_args(url, None, &DEFAULT_HEADERS);
    args.extend([
        "--max-time".into(),
        timeout.to_string(),
        "-c".into(),
        jar.into(),
        "-b".into(),
        jar.into(),
        "-D".into(),
        headers_path.into(),
    ]);
    curl_text(args)
}
