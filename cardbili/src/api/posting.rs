use super::{api_err, net};
use crate::model::Comment;

// 发送参数参考 PiliPlus 的 shootDanmaku、sendLiveMsg 和 replyAdd
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Video {
        cid: i64,
        bvid: String,
        progress_ms: u64,
    },
    Live {
        room_id: u64,
    },
    Comment {
        aid: i64,
    },
}

impl Target {
    pub fn title(&self) -> &'static str {
        match self {
            Self::Comment { .. } => crate::i18n::tr("发评论"),
            _ => crate::i18n::tr("发弹幕"),
        }
    }
}

fn fields(
    target: &Target,
    text: &str,
    csrf: &str,
    timestamp: std::time::Duration,
) -> Vec<(String, String)> {
    let mut fields = vec![("csrf".into(), csrf.into())];
    match target {
        Target::Video {
            cid,
            bvid,
            progress_ms,
            ..
        } => fields.extend([
            ("type".into(), "1".into()),
            ("oid".into(), cid.to_string()),
            ("bvid".into(), bvid.clone()),
            ("msg".into(), text.into()),
            ("progress".into(), progress_ms.to_string()),
            ("color".into(), "16777215".into()),
            ("fontsize".into(), "25".into()),
            ("pool".into(), "0".into()),
            ("mode".into(), "1".into()),
            ("rnd".into(), timestamp.as_micros().to_string()),
        ]),
        Target::Live { room_id } => fields.extend([
            ("roomid".into(), room_id.to_string()),
            ("msg".into(), text.into()),
            ("color".into(), "16777215".into()),
            ("fontsize".into(), "25".into()),
            ("mode".into(), "1".into()),
            ("bubble".into(), "0".into()),
            ("room_type".into(), "0".into()),
            ("jumpfrom".into(), "0".into()),
            ("reply_mid".into(), "0".into()),
            ("reply_attr".into(), "0".into()),
            ("replay_dmid".into(), String::new()),
            ("statistics".into(), r#"{"appId":100,"platform":5}"#.into()),
            ("reply_type".into(), "0".into()),
            ("reply_uname".into(), String::new()),
            ("rnd".into(), timestamp.as_secs().to_string()),
            ("csrf_token".into(), csrf.into()),
        ]),
        Target::Comment { aid } => fields.extend([
            ("type".into(), "1".into()),
            ("oid".into(), aid.to_string()),
            ("message".into(), text.into()),
        ]),
    }
    fields
}

pub fn send(target: &Target, text: &str, cookies: &str) -> Result<Option<Comment>, String> {
    let csrf = super::playback::csrf(cookies)?;
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let (url, referer): (String, String) = match target {
        Target::Video { bvid, .. } => (
            "https://api.bilibili.com/x/v2/dm/post".into(),
            format!("https://www.bilibili.com/video/{bvid}"),
        ),
        Target::Live { room_id } => {
            let signed = super::wbi::wbi_sign(
                &[("web_location".into(), "444.8".into())],
                &super::wbi::fetch_mixin()?,
            );
            (
                format!(
                    "https://api.live.bilibili.com/msg/send?{}",
                    net::query_string(&signed)
                ),
                format!("https://live.bilibili.com/{room_id}"),
            )
        }
        Target::Comment { aid } => (
            "https://api.bilibili.com/x/v2/reply/add".into(),
            format!("https://www.bilibili.com/video/av{aid}"),
        ),
    };
    let fields = fields(target, text, &csrf, timestamp);
    let value = if matches!(target, Target::Live { .. }) {
        net::post_multipart_json(&url, cookies, &referer, &fields)?
    } else {
        net::post_form_json(&url, cookies, &referer, &fields)?
    };
    if value["code"].as_i64() != Some(0) {
        return Err(api_err(target.title(), &value));
    }
    if let Target::Comment { aid } = target {
        crate::cache::invalidate_api(&super::playback::comments_url(*aid, 1), cookies);
        let reply = &value["data"]["reply"];
        if let Some(text) = reply["content"]["message"].as_str() {
            return Ok(Some(Comment {
                user: reply["member"]["uname"].as_str().unwrap_or_default().into(),
                text: super::unescape_xml(text),
                created_at: reply["ctime"]
                    .as_i64()
                    .unwrap_or(timestamp.as_secs() as i64),
                likes: 0,
                emotes: Vec::new(),
            }));
        }
    }
    Ok(None)
}
