use crate::cache;
use crate::model::{self, Account, Comment, Detail, Item, PlayInfo, Seg};
use crate::net;
use serde_json::Value;

/// 统一生成友好的接口错误文案，保留业务码与原始错误信息
fn api_err(name: &str, v: &Value) -> String {
    let code = v["code"].as_i64().unwrap_or(-1);
    let msg = v["message"].as_str().unwrap_or("");
    if msg.is_empty() {
        format!("{}加载失败\ncode={}", name, code)
    } else {
        format!("{}加载失败\ncode={} {}", name, code, msg)
    }
}

pub fn popular(cookies: Option<&str>, pn: u32) -> Result<Vec<Item>, String> {
    let url = format!(
        "https://api.bilibili.com/x/web-interface/popular?ps=20&pn={}",
        pn
    );
    let v = cache::api_json(&url, cookies, cache::TTL_LIST)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("热门推荐", &v));
    }
    Ok(video_items(&v["data"]["list"]))
}

fn video_items(value: &Value) -> Vec<Item> {
    let mut out = Vec::new();
    if let Some(list) = value.as_array() {
        for it in list {
            let bvid = it["bvid"].as_str().unwrap_or("").to_string();
            if bvid.is_empty() {
                continue;
            }
            let title = it["title"].as_str().unwrap_or("").to_string();
            let name = it["owner"]["name"].as_str().unwrap_or("");
            let d = model::dur_of(&it["duration"]);
            let pic = it["pic"].as_str().unwrap_or("").to_string();
            out.push(Item {
                title,
                meta: format!("{} · {}", name, model::mmss(d)),
                bvid,
                pic,
            });
        }
    }
    out
}

pub fn rcmd(cookies: Option<&str>, page: u32) -> Result<Vec<Item>, String> {
    let base = vec![
        ("ps".to_string(), "20".to_string()),
        ("fresh_type".to_string(), "4".to_string()),
        ("feed_version".to_string(), "V8".to_string()),
        ("fresh_idx".to_string(), page.to_string()),
        ("fresh_idx_1h".to_string(), page.to_string()),
        ("fetch_row".to_string(), page.to_string()),
        ("brush".to_string(), "0".to_string()),
        ("homepage_ver".to_string(), "1".to_string()),
        ("y_num".to_string(), "5".to_string()),
        ("last_y_num".to_string(), "5".to_string()),
        ("web_location".to_string(), "1430650".to_string()),
    ];
    let endpoint = "https://api.bilibili.com/x/web-interface/wbi/index/top/feed/rcmd";
    let cache_key = format!("{}?{}", endpoint, net::query_string(&base));
    let v = cache::api_json_with(&cache_key, cookies, cache::TTL_LIST, || {
        let mixin = net::fetch_mixin()?;
        let signed = net::wbi_sign(&base, &mixin);
        let url = format!("{}?{}", endpoint, net::query_string(&signed));
        net::curl_get(&url, cookies)
    })?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("推荐", &v));
    }
    Ok(video_items(&v["data"]["item"]))
}

pub fn history(cookies: &str) -> Result<Vec<Item>, String> {
    let v = cache::api_json(
        "https://api.bilibili.com/x/web-interface/history/cursor?max=0&view_at=0&business=",
        Some(cookies),
        cache::TTL_LIST,
    )?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("历史记录", &v));
    }
    let mut out = Vec::new();
    if let Some(list) = v["data"]["list"].as_array() {
        for it in list {
            let bvid = it["history"]["bvid"].as_str().unwrap_or("").to_string();
            if bvid.is_empty() {
                continue;
            }
            let lt = it["long_title"].as_str().unwrap_or("");
            let title = if !lt.is_empty() {
                lt.to_string()
            } else {
                it["title"].as_str().unwrap_or("").to_string()
            };
            let name = it["author_name"].as_str().unwrap_or("");
            let d = model::dur_of(&it["history"]["duration"]);
            let pic = it["cover"].as_str().unwrap_or("").to_string();
            out.push(Item {
                title,
                meta: format!("{} · {}", name, model::mmss(d)),
                bvid,
                pic,
            });
        }
    }
    Ok(out)
}

pub fn fav(cookies: &str, mid: &str) -> Result<Vec<Item>, String> {
    let furl = format!(
        "https://api.bilibili.com/x/v3/fav/folder/created/list-all?up_mid={}&web_location=333.1387",
        mid
    );
    let fv = cache::api_json(&furl, Some(cookies), cache::TTL_LIST)?;
    if fv["code"].as_i64() != Some(0) {
        return Err(api_err("收藏夹信息", &fv));
    }
    let folders = fv["data"]["list"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let fid = folders
        .iter()
        .find(|f| f["media_count"].as_i64().unwrap_or(0) > 0)
        .or_else(|| folders.first())
        .and_then(|f| f["id"].as_i64())
        .ok_or("无可用收藏夹")?;
    let lurl = format!("https://api.bilibili.com/x/v3/fav/resource/list?media_id={}&pn=1&ps=20&order=mtime&type=0&tid=0&platform=web", fid);
    let lv = cache::api_json(&lurl, Some(cookies), cache::TTL_LIST)?;
    if lv["code"].as_i64() != Some(0) {
        return Err(api_err("收藏列表", &lv));
    }
    let mut out = Vec::new();
    if let Some(list) = lv["data"]["medias"].as_array() {
        for it in list {
            let bvid = it["bvid"].as_str().unwrap_or("").to_string();
            if bvid.is_empty() {
                continue;
            }
            let title = it["title"].as_str().unwrap_or("").to_string();
            let name = it["upper"]["name"].as_str().unwrap_or("");
            let d = model::dur_of(&it["duration"]);
            let pic = it["cover"].as_str().unwrap_or("").to_string();
            out.push(Item {
                title,
                meta: format!("{} · {}", name, model::mmss(d)),
                bvid,
                pic,
            });
        }
    }
    Ok(out)
}

pub fn dynamic(cookies: &str) -> Result<Vec<Item>, String> {
    let url = "https://api.bilibili.com/x/polymer/web-dynamic/v1/feed/all?type=video&page=1&features=itemOpusStyle,listOnlyfans,opusBigCover,onlyfansVote";
    let v = cache::api_json(url, Some(cookies), cache::TTL_LIST)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("动态", &v));
    }
    let mut out = Vec::new();
    if let Some(items) = v["data"]["items"].as_array() {
        for it in items {
            if it["type"].as_str() != Some("DYNAMIC_TYPE_AV") {
                continue;
            }
            let archive = &it["modules"]["module_dynamic"]["major"]["archive"];
            if !archive.is_object() {
                continue;
            }
            let bvid = archive["bvid"].as_str().unwrap_or("");
            if bvid.is_empty() {
                continue;
            }
            let title = archive["title"].as_str().unwrap_or("").to_string();
            let author = it["modules"]["module_author"]["name"]
                .as_str()
                .unwrap_or("");
            let dtext = archive["duration_text"].as_str().unwrap_or("");
            let mut meta = author.to_string();
            if !dtext.is_empty() {
                meta = format!("{} · {}", author, dtext);
            }
            let pic = archive["cover"].as_str().unwrap_or("").to_string();
            out.push(Item {
                title,
                meta,
                bvid: bvid.to_string(),
                pic,
            });
        }
    }
    Ok(out)
}

pub fn play_info(bvid: &str, cookies: Option<&str>) -> Result<PlayInfo, String> {
    let vurl = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let v = cache::api_json(&vurl, cookies, cache::TTL_DETAIL)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("视频信息", &v));
    }
    let d = &v["data"];
    let cid = d["cid"].as_i64().ok_or("no cid")?;
    let title = d["title"].as_str().unwrap_or("").to_string();
    let view_dur = model::dur_of(&d["duration"]);

    let purl = format!(
        "https://api.bilibili.com/x/player/playurl?bvid={}&cid={}&qn=16&fnval=1&fourk=0&platform=html5&high_quality=1",
        bvid, cid
    );
    let p = cache::api_json(&purl, cookies, cache::TTL_PLAYURL)?;
    if p["code"].as_i64() != Some(0) {
        return Err(api_err("播放地址", &p));
    }
    let durl = p["data"]["durl"].as_array();
    let mut segs = Vec::new();
    if let Some(arr) = durl {
        for s in arr {
            let url = s["url"].as_str().unwrap_or("").to_string();
            if url.is_empty() {
                continue;
            }
            let ms = model::dur_of(&s["length"]);
            segs.push(Seg {
                url,
                len: ms / 1000.0,
            });
        }
    }
    if segs.is_empty() {
        return Err("无可用视频流（可能需要登录或为受限内容）".into());
    }
    let sum: f64 = segs.iter().map(|s| s.len).sum();
    let duration = if view_dur > 0.0 { view_dur } else { sum };
    Ok(PlayInfo {
        title,
        segs,
        duration,
        cid,
    })
}

// 拉取视频弹幕 XML 并解析为条目
pub fn danmaku_list(cid: i64, cookies: Option<&str>) -> Result<Vec<crate::danmaku::Dm>, String> {
    let url = format!("https://api.bilibili.com/x/v1/dm/list.so?oid={}", cid);
    let xml = net::curl_get(&url, cookies)?;
    Ok(parse_danmaku(&xml))
}

fn parse_danmaku(xml: &str) -> Vec<crate::danmaku::Dm> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<d p=\"") {
        let s = &rest[i + 6..];
        let q = match s.find('"') {
            Some(q) => q,
            None => break,
        };
        let params = &s[..q];
        let after = &s[q + 1..];
        let close = match after.find("</d>") {
            Some(c) => c,
            None => break,
        };
        let inner = &after[..close];
        let body = match inner.find('>') {
            Some(g) => &inner[g + 1..],
            None => inner,
        };
        let text = unescape_xml(body);
        if let Some(dm) = parse_params(params, text) {
            out.push(dm);
        }
        rest = &after[close + 4..];
    }
    out
}

fn parse_params(p: &str, text: String) -> Option<crate::danmaku::Dm> {
    let mut it = p.split(',');
    let t: f64 = it.next()?.parse().ok()?;
    let mode: u8 = it.next()?.parse().ok()?;
    let _size = it.next();
    let color: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(16777215);
    Some(crate::danmaku::Dm {
        t,
        mode,
        color,
        text,
    })
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

pub fn account(cookies: Option<&str>) -> Result<Account, String> {
    let v = net::get_json("https://api.bilibili.com/x/web-interface/nav", cookies)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("账号信息", &v));
    }
    let d = &v["data"];
    Ok(Account {
        logged: d["isLogin"].as_bool().unwrap_or(false),
        name: d["uname"].as_str().unwrap_or("").to_string(),
        mid: d["mid"].as_i64().map(|x| x.to_string()).unwrap_or_default(),
        level: d["level_info"]["current_level"].as_i64().unwrap_or(0),
        vip: if d["vipStatus"].as_i64().unwrap_or(0) > 0 {
            "大会员".to_string()
        } else {
            String::new()
        },
        face: d["face"].as_str().unwrap_or("").to_string(),
    })
}

pub fn video_detail(bvid: &str, cookies: Option<&str>) -> Result<Detail, String> {
    let vurl = format!(
        "https://api.bilibili.com/x/web-interface/view?bvid={}",
        bvid
    );
    let v = cache::api_json(&vurl, cookies, cache::TTL_DETAIL)?;
    if v["code"].as_i64() != Some(0) {
        return Err(api_err("视频信息", &v));
    }
    let d = &v["data"];
    let aid = d["aid"].as_i64().unwrap_or(0);
    let stat = &d["stat"];

    let mut comments = Vec::new();
    if aid > 0 {
        const MAX_C: usize = 200;
        let mut pn = 1u32;
        while comments.len() < MAX_C && pn <= 12 {
            let curl = format!(
                "https://api.bilibili.com/x/v2/reply?type=1&oid={}&pn={}&ps=20&sort=2&nohot=1",
                aid, pn
            );
            let mut got = 0usize;
            if let Ok(rv) = cache::api_json(&curl, cookies, cache::TTL_REPLY) {
                if let Some(arr) = rv["data"]["replies"].as_array() {
                    for r in arr {
                        let user = r["member"]["uname"].as_str().unwrap_or("").to_string();
                        let text = r["content"]["message"].as_str().unwrap_or("").to_string();
                        if !text.is_empty() {
                            comments.push(Comment { user, text });
                            got += 1;
                            if comments.len() >= MAX_C {
                                break;
                            }
                        }
                    }
                }
            }
            if got == 0 {
                break;
            }
            pn += 1;
        }
    }

    Ok(Detail {
        title: d["title"].as_str().unwrap_or("").to_string(),
        pic: d["pic"].as_str().unwrap_or("").to_string(),
        duration: model::dur_of(&d["duration"]),
        owner: d["owner"]["name"].as_str().unwrap_or("").to_string(),
        views: stat["view"].as_i64().unwrap_or(0),
        likes: stat["like"].as_i64().unwrap_or(0),
        coins: stat["coin"].as_i64().unwrap_or(0),
        desc: d["desc"].as_str().unwrap_or("").to_string(),
        comments,
    })
}
