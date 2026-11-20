use crate::model::AudioTrack;
use crate::{cache, net};
use serde_json::Value;

// 拉取音轨列表，需登录 Cookie
pub fn tracks(bvid: &str, cid: i64, cookies: Option<&str>) -> Result<Vec<AudioTrack>, String> {
    let v = wbi_playurl(bvid, cid, "", 0, cookies)?;
    let items = v["data"]["language"]["items"].as_array();
    let mut out = Vec::new();
    for item in items.into_iter().flatten() {
        out.push(AudioTrack {
            lang: item["lang"].as_str().unwrap_or("").to_string(),
            title: item["title"]
                .as_str()
                .or_else(|| item["display_name"].as_str())
                .or_else(|| item["lang"].as_str())
                .unwrap_or("")
                .to_string(),
            production_type: item["production_type"].as_i64().unwrap_or(0) as i32,
        });
    }
    Ok(out)
}

// 按指定音轨请求播放流，返回视频/音频地址、时长与技术统计
pub fn stream_urls(
    bvid: &str,
    cid: i64,
    cdn: &str,
    cookies: Option<&str>,
    track: &AudioTrack,
) -> Result<super::Streams, String> {
    let p = wbi_playurl(bvid, cid, &track.lang, track.production_type, cookies)?;
    super::pick_streams(&p, cdn)
}

fn wbi_playurl(
    bvid: &str,
    cid: i64,
    cur_language: &str,
    production_type: i32,
    cookies: Option<&str>,
) -> Result<Value, String> {
    let mk = super::wbi::fetch_mixin()?;
    let mut params = vec![
        ("bvid".to_string(), bvid.to_string()),
        ("cid".to_string(), cid.to_string()),
        ("fnval".to_string(), "16".to_string()),
        ("fnver".to_string(), "0".to_string()),
        ("fourk".to_string(), "0".to_string()),
        ("client_attr".to_string(), "1".to_string()),
    ];
    if !cur_language.is_empty() {
        params.push(("cur_language".to_string(), cur_language.to_string()));
        params.push((
            "cur_production_type".to_string(),
            production_type.to_string(),
        ));
    }
    let signed = super::wbi::wbi_sign(&params, &mk);
    let url = format!(
        "https://api.bilibili.com/x/player/wbi/playurl?{}",
        net::query_string(&signed)
    );
    let value = cache::api_json(&url, cookies, cache::TTL_PLAYURL)?;
    if value["code"].as_i64() != Some(0) {
        return Err(super::api_err(crate::i18n::tr("播放地址"), &value));
    }
    Ok(value)
}
