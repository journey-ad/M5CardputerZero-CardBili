use super::{api_err, cache, net, wbi};
use crate::model::{Chapter, VideoPart};
use serde_json::Value;

pub(super) fn parts(data: &Value) -> Vec<VideoPart> {
    data["pages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| {
            Some(VideoPart {
                cid: part["cid"].as_i64()?,
                title: part["part"].as_str().unwrap_or_default().into(),
                duration: crate::model::dur_of(&part["duration"]),
            })
        })
        .collect()
}

pub fn chapters(bvid: &str, cid: i64, cookies: Option<&str>) -> Result<Vec<Chapter>, String> {
    let params = [
        ("bvid".into(), bvid.into()),
        ("cid".into(), cid.to_string()),
    ];
    let endpoint = "https://api.bilibili.com/x/player/wbi/v2";
    let key = format!("{endpoint}?{}", net::query_string(&params));
    let value = cache::api_json_with(&key, cookies, cache::TTL_DETAIL, || {
        let signed = wbi::wbi_sign(&params, &wbi::fetch_mixin()?);
        net::curl_get(
            &format!("{endpoint}?{}", net::query_string(&signed)),
            cookies,
        )
    })?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("章节", &value));
    }
    Ok(value["data"]["view_points"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|point| point["type"].as_i64() == Some(2))
        .filter_map(|point| {
            Some(Chapter {
                title: point["content"].as_str()?.into(),
                start: point["from"].as_f64()?,
            })
        })
        .collect())
}

pub fn watching(bvid: &str, cid: i64, cookies: Option<&str>) -> Result<String, String> {
    let value = net::get_json(
        &format!("https://api.bilibili.com/x/player/online/total?bvid={bvid}&cid={cid}"),
        cookies,
    )?;
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("在看人数", &value));
    }
    Ok(value["data"]["total"]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(|total| format!("{total}人在看"))
        .unwrap_or_default())
}
