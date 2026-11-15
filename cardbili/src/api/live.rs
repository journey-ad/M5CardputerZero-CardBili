use super::{api_err, cache, model, net, Item, Value};

// 直播推荐列表，room_id 写入 bvid 复用视频列表与封面管线
pub fn rooms(page: u32) -> Result<Vec<Item>, String> {
    let url = format!(
        "https://api.live.bilibili.com/room/v1/room/get_user_recommend?page={}&page_size=20",
        page
    );
    let v = cache::api_json(&url, None, cache::TTL_LIST)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("直播列表", &v));
    }
    let mut out = Vec::new();
    for it in v["data"].as_array().into_iter().flatten() {
        let room_id = it["roomid"].as_i64().unwrap_or(0);
        if room_id <= 0 {
            continue;
        }
        let pic = ["user_cover", "system_cover", "keyframe"]
            .iter()
            .find_map(|key| it[*key].as_str().filter(|value| !value.is_empty()))
            .unwrap_or_default();
        out.push(Item {
            title: it["title"].as_str().unwrap_or_default().into(),
            meta: it["uname"].as_str().unwrap_or_default().into(),
            bvid: room_id.to_string(),
            pic: pic.into(),
            views: it["online"].as_i64().map(model::fmt_num),
            ..Default::default()
        });
    }
    Ok(out)
}

pub fn room(room_id: u64, cookies: Option<&str>) -> Result<model::LiveInfo, String> {
    let info_url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v1/index/getH5InfoByRoom?room_id={}",
        room_id
    );
    let v = net::get_json(&info_url, cookies)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("直播间信息", &v));
    }
    let room = &v["data"]["room_info"];
    if room["live_status"].as_i64() != Some(1) {
        return Err("直播间未开播".into());
    }
    let room_id = room["room_id"]
        .as_u64()
        .filter(|id| *id > 0)
        .unwrap_or(room_id);
    let play_url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v2/index/getRoomPlayInfo?room_id={}&protocol=0,1&format=0,1,2&codec=0&qn=150&platform=web&ptype=8&dolby=5&panorama=1",
        room_id
    );
    let p = net::get_json(&play_url, cookies)?;
    if p["code"].as_i64() != Some(0) {
        return Err(api_err("直播播放地址", &p));
    }
    let url = stream_url(&p["data"]).ok_or("暂无兼容的 AVC 直播流，请换个直播间")?;
    crate::diagnostics::write(format_args!(
        "[直播] 房间={} 流={}",
        room_id,
        crate::cdn::host(&url)
    ));
    Ok(model::LiveInfo {
        room_id,
        title: room["title"].as_str().unwrap_or_default().into(),
        uname: v["data"]["anchor_info"]["base_info"]["uname"]
            .as_str()
            .unwrap_or_default()
            .into(),
        online: model::fmt_num(room["online"].as_i64().unwrap_or(0)),
        play_url: url,
        started_at: room["live_start_time"].as_i64().unwrap_or(0),
    })
}

// 先选较低画质，再选连续传输的 FLV，设备解码器支持 AVC
fn stream_url(data: &Value) -> Option<String> {
    let mut best = None;
    for stream in data["playurl_info"]["playurl"]["stream"]
        .as_array()
        .into_iter()
        .flatten()
    {
        for format in stream["format"].as_array().into_iter().flatten() {
            let priority = match (
                stream["protocol_name"].as_str(),
                format["format_name"].as_str(),
            ) {
                (Some("http_stream"), Some("flv")) => 0,
                (Some("http_hls"), Some("ts")) => 1,
                (Some("http_hls"), Some("fmp4")) => 2,
                _ => continue,
            };
            for codec in format["codec"].as_array().into_iter().flatten() {
                if codec["codec_name"].as_str() != Some("avc") {
                    continue;
                }
                let base = codec["base_url"].as_str().unwrap_or_default();
                let quality = codec["current_qn"].as_u64().unwrap_or(u64::MAX);
                for server in codec["url_info"].as_array().into_iter().flatten() {
                    let host = server["host"].as_str().unwrap_or_default();
                    let extra = server["extra"].as_str().unwrap_or_default();
                    if host.is_empty() || base.is_empty() {
                        continue;
                    }
                    let rank = (quality, priority);
                    if best.as_ref().map_or(true, |(previous, _)| rank < *previous) {
                        best = Some((rank, format!("{host}{base}{extra}")));
                    }
                }
            }
        }
    }
    best.map(|(_, url)| url)
}

pub struct DanmakuServer {
    pub host: String,
    pub port: u16,
}

pub fn super_chat_messages(
    room: u64,
    cookies: Option<&str>,
) -> Result<Vec<crate::super_chat::Message>, String> {
    let url = format!(
        "https://api.live.bilibili.com/av/v1/SuperChat/getMessageList?room_id={room}"
    );
    let value = net::get_json(&url, cookies)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("醒目留言", &value));
    }
    Ok(value["data"]["list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(crate::super_chat::Message::parse)
        .collect())
}

pub struct DanmakuInfo {
    pub token: String,
    pub servers: Vec<DanmakuServer>,
}

pub fn danmaku_info(room_id: u64, cookies: Option<&str>) -> Result<DanmakuInfo, String> {
    let params = super::wbi::wbi_sign(
        &[
            ("id".into(), room_id.to_string()),
            ("web_location".into(), "444.8".into()),
        ],
        &super::wbi::fetch_mixin()?,
    );
    let url = format!(
        "https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo?{}",
        net::query_string(&params)
    );
    let value = net::get_json(&url, cookies)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("直播弹幕", &value));
    }
    let data = &value["data"];
    let token = data["token"]
        .as_str()
        .filter(|token| !token.is_empty())
        .ok_or("直播弹幕认证信息为空")?
        .to_string();
    let servers = data["host_list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|server| {
            Some(DanmakuServer {
                host: server["host"].as_str()?.to_string(),
                port: u16::try_from(server["wss_port"].as_u64()?).ok()?,
            })
        })
        .collect::<Vec<_>>();
    if servers.is_empty() {
        return Err("暂无可用的直播弹幕服务器".into());
    }
    Ok(DanmakuInfo { token, servers })
}
