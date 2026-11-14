use super::{api_err, cache, net};
use crate::model::{Comment, CommentPage, ReactionResult, VideoAction};

pub fn liked(bvid: &str, cookies: &str) -> Result<bool, String> {
    let value = net::get_json(
        &format!("https://api.bilibili.com/x/web-interface/archive/has/like?bvid={bvid}"),
        Some(cookies),
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("点赞状态", &value));
    }
    Ok(value["data"].as_i64() == Some(1))
}

pub fn react(
    aid: i64,
    bvid: &str,
    cookies: &str,
    action: VideoAction,
) -> Result<ReactionResult, String> {
    let name = match action {
        VideoAction::Like(true) => "点赞",
        VideoAction::Like(false) => "取消点赞",
        VideoAction::Triple => "一键三连",
    };
    crate::diagnostics::write(format_args!("[互动] {name} bvid={bvid}"));
    let csrf = csrf(cookies)?;
    let mut params = vec![("csrf".into(), csrf.into())];
    let endpoint = match action {
        VideoAction::Like(liked) => {
            params.push(("bvid".into(), bvid.into()));
            params.push(("like".into(), if liked { "1" } else { "2" }.into()));
            "like"
        }
        VideoAction::Triple => {
            params.extend([
                ("aid".into(), aid.to_string()),
                ("eab_x".into(), "2".into()),
                ("ramval".into(), "0".into()),
                ("source".into(), "web_normal".into()),
                ("ga".into(), "1".into()),
                ("spmid".into(), "333.788.0.0".into()),
                ("statistics".into(), r#"{"appId":100,"platform":5}"#.into()),
            ]);
            "like/triple"
        }
    };
    let value = net::post_form_json(
        &format!("https://api.bilibili.com/x/web-interface/archive/{endpoint}"),
        cookies,
        &format!("https://www.bilibili.com/video/{bvid}"),
        &params,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("视频互动", &value));
    }
    match action {
        VideoAction::Like(liked) => Ok(ReactionResult {
            triple_complete: false,
            liked,
            message: if liked {
                "已点赞"
            } else {
                "已取消点赞"
            }
            .into(),
        }),
        VideoAction::Triple => {
            let data = &value["data"];
            let liked = data["like"].as_bool().unwrap_or(false);
            let coined = data["coin"].as_bool().unwrap_or(false);
            let favored = data["fav"].as_bool().unwrap_or(false);
            let message = if liked && coined && favored {
                "三连成功".into()
            } else {
                let mut completed = Vec::new();
                if liked {
                    completed.push("已点赞");
                }
                if coined {
                    completed.push("已投币");
                }
                if favored {
                    completed.push("已收藏");
                }
                if completed.is_empty() {
                    "三连未完成，请稍后重试".into()
                } else {
                    completed.join(" · ")
                }
            };
            Ok(ReactionResult {
                liked,
                message,
                triple_complete: liked && coined && favored,
            })
        }
    }
}

pub fn comments(aid: i64, page: u32, cookies: Option<&str>) -> Result<CommentPage, String> {
    let url = format!(
        "https://api.bilibili.com/x/v2/reply?type=1&oid={aid}&pn={page}&ps=20&sort=2&nohot=1"
    );
    let value = cache::api_json(&url, cookies, cache::TTL_REPLY)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("评论", &value));
    }
    let items: Vec<Comment> = value["data"]["replies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|reply| {
            let text = reply["content"]["message"].as_str()?.to_string();
            Some(Comment {
                user: reply["member"]["uname"].as_str().unwrap_or_default().into(),
                text,
            })
        })
        .collect();
    let has_more = value["data"]["page"]["count"]
        .as_u64()
        .map(|count| u64::from(page) * 20 < count)
        .unwrap_or(items.len() == 20);
    Ok(CommentPage { items, has_more })
}

fn csrf(cookies: &str) -> Result<String, String> {
    net::session::cookie(cookies, "bili_jct")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "登录信息已失效，请重新登录".into())
}

pub fn heartbeat(bvid: &str, cid: i64, progress: i64, cookies: &str) -> Result<(), String> {
    crate::diagnostics::write(format_args!(
        "[历史] 上报 bvid={bvid} cid={cid} 进度={progress}s"
    ));
    let params = vec![
        ("bvid".into(), bvid.into()),
        ("cid".into(), cid.to_string()),
        ("type".into(), "3".into()),
        ("played_time".into(), progress.to_string()),
        ("csrf".into(), csrf(cookies)?.into()),
    ];
    let value = net::post_form_json_timeout(
        "https://api.bilibili.com/x/click-interface/web/heartbeat",
        cookies,
        &format!("https://www.bilibili.com/video/{bvid}"),
        &params,
        4,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("观看历史", &value));
    }
    cache::invalidate_api(super::HISTORY_URL, cookies);
    Ok(())
}
