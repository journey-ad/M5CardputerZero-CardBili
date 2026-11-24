use super::{api_err, cache, net};
use crate::model::{Comment, CommentEmote, CommentPage, ReactionResult, VideoAction};

pub fn liked(bvid: &str, cookies: &str) -> Result<bool, String> {
    let value = net::get_json(
        &format!("https://api.bilibili.com/x/web-interface/archive/has/like?bvid={bvid}"),
        Some(cookies),
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err(crate::i18n::tr("点赞状态"), &value));
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
        VideoAction::Like(true) => crate::i18n::tr("点赞"),
        VideoAction::Like(false) => crate::i18n::tr("取消点赞"),
        VideoAction::Triple => crate::i18n::tr("一键三连"),
        VideoAction::Coin => crate::i18n::tr("投币"),
        VideoAction::Favorite(_) => crate::i18n::tr("收藏"),
    };
    crate::diagnostics::write(format_args!("[互动] {name} bvid={bvid}"));
    let csrf = csrf(cookies)?;
    let mut params = vec![("csrf".into(), csrf.into())];
    let endpoint = match action {
        VideoAction::Like(liked) => {
            params.push(("bvid".into(), bvid.into()));
            params.push(("like".into(), if liked { "1" } else { "2" }.into()));
            "https://api.bilibili.com/x/web-interface/archive/like"
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
            "https://api.bilibili.com/x/web-interface/archive/like/triple"
        }
        VideoAction::Coin => {
            params.extend([
                ("aid".into(), aid.to_string()),
                ("multiply".into(), "1".into()),
                ("select_like".into(), "0".into()),
            ]);
            "https://api.bilibili.com/x/web-interface/coin/add"
        }
        VideoAction::Favorite(folder) => {
            params.extend([
                ("resources".into(), format!("{aid}:2")),
                ("add_media_ids".into(), folder.to_string()),
                ("del_media_ids".into(), String::new()),
            ]);
            "https://api.bilibili.com/x/v3/fav/resource/batch-deal"
        }
    };
    let value = net::post_form_json(
        endpoint,
        cookies,
        &format!("https://www.bilibili.com/video/{bvid}"),
        &params,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err(crate::i18n::tr("视频互动"), &value));
    }
    match action {
        VideoAction::Like(liked) => Ok(ReactionResult {
            triple_complete: false,
            liked: Some(liked),
            message: if liked {
                crate::i18n::tr("已点赞")
            } else {
                crate::i18n::tr("已取消点赞")
            }
            .into(),
        }),
        VideoAction::Triple => {
            let data = &value["data"];
            let liked = data["like"].as_bool().unwrap_or(false);
            let coined = data["coin"].as_bool().unwrap_or(false);
            let favored = data["fav"].as_bool().unwrap_or(false);
            let message = if liked && coined && favored {
                crate::i18n::tr("三连成功").into()
            } else {
                let mut completed = Vec::new();
                if liked {
                    completed.push(crate::i18n::tr("已点赞"));
                }
                if coined {
                    completed.push(crate::i18n::tr("已投币"));
                }
                if favored {
                    completed.push(crate::i18n::tr("已收藏"));
                }
                if completed.is_empty() {
                    crate::i18n::tr("三连未完成，请稍后重试").into()
                } else {
                    completed.join(" · ")
                }
            };
            Ok(ReactionResult {
                liked: Some(liked),
                message,
                triple_complete: liked && coined && favored,
            })
        }
        VideoAction::Coin | VideoAction::Favorite(_) => Ok(ReactionResult {
            triple_complete: false,
            liked: None,
            message: if matches!(action, VideoAction::Coin) {
                crate::i18n::tr("已投币")
            } else {
                crate::i18n::tr("已收藏")
            }.into(),
        }),
    }
}

pub(super) fn comments_url(aid: i64, page: u32) -> String {
    format!(
        "https://api.bilibili.com/x/v2/reply?type=1&oid={aid}&pn={page}&ps=20&sort=2&nohot=1"
    )
}

pub fn comments(aid: i64, page: u32, cookies: Option<&str>) -> Result<CommentPage, String> {
    let url = comments_url(aid, page);
    let value = cache::api_json(&url, cookies, cache::TTL_REPLY)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err(crate::i18n::tr("评论"), &value));
    }
    let items: Vec<Comment> = value["data"]["replies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|reply| {
            let content = &reply["content"];
            let text = super::unescape_xml(content["message"].as_str()?);
            // 正文里的 [名称] 占位与 content.emote 的图片地址一一对应
            let emotes = content["emote"]
                .as_object()
                .map(|map| {
                    map.iter()
                        .filter_map(|(name, emote)| {
                            let url = emote["url"].as_str()?.to_string();
                            // 普通小表情使用 12px，装扮等大表情使用 24px
                            let small = emote["type"].as_u64() == Some(1)
                                && emote["meta"]["size"].as_u64().unwrap_or(1) <= 1;
                            let size = if small { 12 } else { 24 };
                            Some(CommentEmote {
                                name: name.clone(),
                                url,
                                size,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Comment {
                user: super::unescape_xml(reply["member"]["uname"].as_str().unwrap_or_default()).into(),
                text,
                created_at: reply["ctime"].as_i64().unwrap_or_default(),
                likes: reply["like"].as_i64().unwrap_or_default(),
                emotes,
            })
        })
        .collect();
    let has_more = value["data"]["page"]["count"]
        .as_u64()
        .map(|count| u64::from(page) * 20 < count)
        .unwrap_or(items.len() == 20);
    Ok(CommentPage { items, has_more })
}

pub(super) fn csrf(cookies: &str) -> Result<String, String> {
    net::session::cookie(cookies, "bili_jct")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| crate::i18n::tr("登录信息已失效，请重新登录").into())
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
        return Err(api_err(crate::i18n::tr("观看历史"), &value));
    }
    cache::invalidate_api(super::HISTORY_URL, cookies);
    Ok(())
}
