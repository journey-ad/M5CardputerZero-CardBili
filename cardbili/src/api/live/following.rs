use super::{api_err, model, net, Item, Value};

const PAGE_SIZE: u64 = 20;

pub fn rooms(page: u32, cookies: &str) -> Result<model::VideoPage, String> {
    // 开播状态实时变化，不读取列表缓存。
    let url = format!(
        "https://api.live.bilibili.com/xlive/web-ucenter/user/following?page={page}&page_size={PAGE_SIZE}&ignoreRecord=1&hit_ab=true"
    );
    parse(&net::get_json(&url, Some(cookies))?, page)
}

fn parse(value: &Value, page: u32) -> Result<model::VideoPage, String> {
    if value["code"].as_i64() != Some(0) {
        return Err(api_err("关注的直播", value));
    }
    let data = &value["data"];
    let list = match &data["list"] {
        Value::Array(list) => list.as_slice(),
        Value::Null if data["count"].as_u64() == Some(0) => &[],
        _ => return Err("关注的直播加载失败：列表数据异常".into()),
    };
    let items = list
        .iter()
        .filter(|room| room["live_status"].as_i64() == Some(1))
        .filter_map(|room| {
            let room_id = room["roomid"].as_u64().filter(|id| *id > 0)?;
            Some(Item {
                title: room["title"].as_str().unwrap_or_default().into(),
                meta: room["uname"].as_str().unwrap_or_default().into(),
                bvid: room_id.to_string(),
                pic: room["room_cover"].as_str().unwrap_or_default().into(),
                duration: "直播中".into(),
                ..Default::default()
            })
        })
        .collect();
    // 分页依据原始列表，不能因本页筛选后没有开播主播而提前结束。
    let page_size = data["pageSize"]
        .as_u64()
        .filter(|size| *size > 0)
        .unwrap_or(PAGE_SIZE);
    let has_more = data["live_count"].as_u64() != Some(0)
        && !list.is_empty()
        && data["totalPage"].as_u64().map_or_else(
            || {
                data["count"]
                    .as_u64()
                    .map_or(list.len() as u64 >= page_size, |count| {
                        u64::from(page) * page_size < count
                    })
            },
            |total| u64::from(page) < total,
        );
    Ok(model::VideoPage {
        items,
        has_more,
        total: data["live_count"].as_u64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_live_rooms_are_playable() {
        let result = parse(&json!({"code": 0, "data": {
            "totalPage": 1, "live_count": 1,
            "list": [
                {"roomid": 42, "live_status": 1, "uname": "主播", "title": "直播标题", "room_cover": "https://example.com/cover.jpg"},
                {"roomid": 43, "live_status": 0},
                {"roomid": 44, "live_status": 2},
                {"roomid": 0, "live_status": 1}
            ]
        }}), 1).unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].bvid, "42");
        assert_eq!(result.items[0].meta, "主播");
        assert_eq!(result.items[0].title, "直播标题");
        assert_eq!(result.items[0].pic, "https://example.com/cover.jpg");
        assert!(!result.has_more);
    }

    #[test]
    fn offline_page_does_not_hide_later_live_rooms() {
        let value = json!({"code": 0, "data": {
            "totalPage": 2, "list": [{"roomid": 42, "live_status": 0}]
        }});
        let first = parse(&value, 1).unwrap();
        assert!(first.items.is_empty());
        assert!(first.has_more);
        assert!(!parse(&value, 2).unwrap().has_more);
        let none_live = json!({"code": 0, "data": {
            "totalPage": 50, "live_count": 0,
            "list": [{"roomid": 42, "live_status": 0}]
        }});
        assert!(!parse(&none_live, 1).unwrap().has_more);
    }

    #[test]
    fn pagination_uses_server_page_size_when_total_pages_are_absent() {
        let value = json!({"code": 0, "data": {
            "pageSize": 1, "count": 2,
            "list": [{"roomid": 42, "live_status": 1}]
        }});
        assert!(parse(&value, 1).unwrap().has_more);
        assert!(!parse(&value, 2).unwrap().has_more);
        let empty = json!({"code": 0, "data": {"list": [], "totalPage": 10}});
        assert!(!parse(&empty, 1).unwrap().has_more);
    }

    #[test]
    fn empty_and_failed_responses_are_distinct() {
        let empty = parse(&json!({"code": 0, "data": {"list": [], "count": 0}}), 1).unwrap();
        assert!(empty.items.is_empty());
        assert!(!empty.has_more);
        assert!(parse(&json!({"code": -101, "message": "账号未登录"}), 1)
            .err()
            .unwrap()
            .contains("-101"));
        assert!(parse(&json!({"code": 0, "data": {}}), 1).is_err());
    }
}
