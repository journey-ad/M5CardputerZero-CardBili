use super::*;

pub fn folders(mid: &str, cookies: &str) -> Result<Vec<model::FavoriteFolder>, String> {
    let url = format!("https://api.bilibili.com/x/v3/fav/folder/created/list-all?up_mid={mid}&web_location=333.1387");
    let value = cache::api_json(&url, Some(cookies), cache::TTL_LIST)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("收藏夹", &value));
    }
    Ok(value["data"]["list"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|folder| {
            Some(model::FavoriteFolder {
                id: folder["id"].as_i64()?,
                title: folder["title"].as_str().unwrap_or_default().into(),
                count: folder["media_count"].as_u64().unwrap_or_default(),
            })
        })
        .collect())
}

pub fn videos(folder: i64, page: u32, cookies: &str) -> Result<model::VideoPage, String> {
    let url = format!("https://api.bilibili.com/x/v3/fav/resource/list?media_id={folder}&pn={page}&ps=20&order=mtime&type=0&tid=0&platform=web");
    let value = cache::api_json(&url, Some(cookies), cache::TTL_LIST)?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("收藏视频", &value));
    }
    let items = value["data"]["medias"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let bvid = item["bvid"].as_str().filter(|bvid| !bvid.is_empty())?;
            Some(Item {
                bvid: bvid.into(),
                title: item["title"].as_str().unwrap_or_default().into(),
                meta: item["upper"]["name"].as_str().unwrap_or_default().into(),
                duration: model::mmss(model::dur_of(&item["duration"])),
                pic: item["cover"].as_str().unwrap_or_default().into(),
                views: count_text(&item["cnt_info"]["play"]),
                likes: count_text(&item["cnt_info"]["thumb_up"]),
                danmaku: count_text(&item["cnt_info"]["danmaku"]),
            })
        })
        .collect();
    Ok(model::VideoPage {
        items,
        has_more: value["data"]["has_more"].as_bool().unwrap_or(false),
        total: value["data"]["info"]["media_count"].as_u64(),
    })
}
