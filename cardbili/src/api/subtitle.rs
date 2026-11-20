use crate::model::{SubtitleCue, SubtitleTrack};
use crate::{cache, net};
use serde_json::Value;

// 拉取字幕轨列表，需登录 Cookie
pub fn tracks(bvid: &str, cid: i64, cookies: Option<&str>) -> Result<Vec<SubtitleTrack>, String> {
    let mk = super::wbi::fetch_mixin()?;
    let params = vec![
        ("bvid".to_string(), bvid.to_string()),
        ("cid".to_string(), cid.to_string()),
    ];
    let signed = super::wbi::wbi_sign(&params, &mk);
    let url = format!(
        "https://api.bilibili.com/x/player/wbi/v2?{}",
        net::query_string(&signed)
    );
    let v = cache::api_json(&url, cookies, cache::TTL_PLAYURL)?;
    let list = v["data"]["subtitle"]["subtitles"].as_array();
    let mut out = Vec::new();
    for item in list.into_iter().flatten() {
        let url = match item["subtitle_url"].as_str() {
            Some(u) if !u.is_empty() => u.to_string(),
            _ => continue,
        };
        out.push(SubtitleTrack {
            lan: item["lan"].as_str().unwrap_or("").to_string(),
            lan_doc: item["lan_doc"].as_str().unwrap_or("").to_string(),
            is_ai: item["type"].as_i64() == Some(1),
            url,
        });
    }
    Ok(out)
}

// 下载字幕正文，跳过空文本条目
pub fn cues(url: &str) -> Result<Vec<SubtitleCue>, String> {
    let link = if url.starts_with("//") {
        format!("https:{url}")
    } else {
        url.to_string()
    };
    let body = net::get(&link, None)?;
    let v: Value =
        serde_json::from_str(&body).map_err(|_| crate::i18n::tr("字幕解析失败").to_string())?;
    let mut out = Vec::new();
    for item in v["body"].as_array().into_iter().flatten() {
        let from = item["from"].as_f64().unwrap_or(0.0);
        let to = item["to"].as_f64().unwrap_or(0.0);
        let content = item["content"].as_str().unwrap_or("").to_string();
        if content.trim().is_empty() {
            continue;
        }
        out.push(SubtitleCue { from, to, content });
    }
    Ok(out)
}
