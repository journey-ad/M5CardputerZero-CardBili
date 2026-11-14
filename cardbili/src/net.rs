use crate::diagnostics;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

pub fn new_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(UA)
        .timeout_global(Some(Duration::from_secs(25)))
        .http_status_as_error(false)
        .build()
        .into()
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(new_agent)
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|s| s.as_secs() as i64)
        .unwrap_or(0)
}

fn request(
    client: &ureq::Agent,
    url: &str,
    cookies: Option<&str>,
    headers: &RequestHeaders<'_>,
    form: Option<&str>,
    timeout: u32,
) -> Result<Vec<u8>, String> {
    let endpoint = url.split(['?', '#']).next().unwrap_or(url);
    let method = if form.is_some() { "POST" } else { "GET" };
    let started = Instant::now();
    diagnostics::write(format_args!("[请求] {method} {endpoint} 开始"));
    let result = (|| -> Result<_, ureq::Error> {
        let mut request = ureq::http::Request::builder()
            .method(method)
            .uri(url)
            .header("User-Agent", headers.user_agent)
            .header("Referer", headers.referer)
            .header("Origin", headers.origin)
            .header("Accept", "application/json, text/plain, */*");
        if let Some(cookie) = cookies.filter(|value| !value.is_empty()) {
            request = request.header("Cookie", cookie);
        }
        if form.is_some() {
            request = request.header("Content-Type", "application/x-www-form-urlencoded");
        }
        let request = client
            .configure_request(request.body(())?)
            .timeout_global(Some(Duration::from_secs(timeout.into())))
            .build();
        let mut response = match form {
            Some(body) => client.run(request.map(|_| body))?,
            None => client.run(request)?,
        };
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(ureq::Error::StatusCode(status));
        }
        let bytes = response.body_mut().with_config().read_to_vec()?;
        Ok((status, bytes))
    })();
    match result {
        Ok((status, bytes)) => {
            diagnostics::write(format_args!(
                "[请求] {method} {endpoint} 完成 HTTP={status} 字节={} 耗时={}ms",
                bytes.len(),
                started.elapsed().as_millis()
            ));
            Ok(bytes)
        }
        Err(error) => {
            let message = error.to_string().replace(url, endpoint);
            diagnostics::write(format_args!(
                "[请求] {method} {endpoint} 失败 耗时={}ms: {message}",
                started.elapsed().as_millis()
            ));
            Err(format!("网络请求失败\n{message}"))
        }
    }
}

pub fn get_with_headers(
    url: &str,
    cookies: Option<&str>,
    headers: &RequestHeaders<'_>,
) -> Result<String, String> {
    request(agent(), url, cookies, headers, None, 25)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
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
    let body = request(
        agent(),
        url,
        Some(cookies),
        &headers,
        Some(&query_string(params)),
        timeout,
    )?;
    parse_json(url, &String::from_utf8_lossy(&body))
        .map_err(|error| format!("数据解析失败\n{error}"))
}

pub fn get(url: &str, cookies: Option<&str>) -> Result<String, String> {
    get_with_headers(url, cookies, &DEFAULT_HEADERS)
}

pub fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    request(agent(), url, None, &DEFAULT_HEADERS, None, 25)
}

pub fn get_with_agent(client: &ureq::Agent, url: &str, timeout: u32) -> Result<String, String> {
    request(client, url, None, &DEFAULT_HEADERS, None, timeout)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

pub fn get_json(url: &str, cookies: Option<&str>) -> Result<serde_json::Value, String> {
    let text = get(url, cookies)?;
    parse_json(url, &text).map_err(|error| format!("数据解析失败\n{error}"))
}

pub fn parse_json(url: &str, body: &str) -> Result<serde_json::Value, serde_json::Error> {
    let endpoint = url.split(['?', '#']).next().unwrap_or(url);
    let result = serde_json::from_str::<serde_json::Value>(body);
    match &result {
        Ok(value) => diagnostics::write(format_args!(
            "[接口] {endpoint} 业务码={:?}",
            value["code"].as_i64()
        )),
        Err(error) => diagnostics::write(format_args!("[接口] {endpoint} JSON 解析失败: {error}")),
    }
    result
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
