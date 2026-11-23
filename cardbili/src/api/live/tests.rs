use super::*;
use serde_json::json;

fn stream(protocol: &str, format: &str, codec: &str, quality: u64, path: &str) -> Value {
    json!({"protocol_name": protocol, "format": [{"format_name": format, "codec": [{
        "codec_name": codec, "current_qn": quality, "base_url": path,
        "url_info": [{"host": "https://live.example", "extra": "?token=abc"}]
    }]}]})
}

fn data(streams: Vec<Value>) -> Value {
    json!({"playurl_info": {"playurl": {"stream": streams}}})
}

#[test]
fn live_selection_prioritizes_lower_quality_before_format() {
    let value = data(vec![
        stream("http_stream", "flv", "avc", 400, "/high.flv"),
        stream("http_hls", "ts", "avc", 150, "/low.m3u8"),
    ]);
    assert_eq!(
        pick_stream(&value).map(|(url, _)| url).as_deref(),
        Some("https://live.example/low.m3u8?token=abc")
    );
}

#[test]
fn equal_quality_prefers_flv_then_ts_then_fmp4() {
    let fmp4 = stream("http_hls", "fmp4", "avc", 150, "/video.mp4");
    let ts = stream("http_hls", "ts", "avc", 150, "/video.m3u8");
    let flv = stream("http_stream", "flv", "avc", 150, "/video.flv");
    assert_eq!(
        pick_stream(&data(vec![fmp4.clone(), ts.clone(), flv]))
            .map(|(url, _)| url)
            .as_deref(),
        Some("https://live.example/video.flv?token=abc")
    );
    assert_eq!(
        pick_stream(&data(vec![fmp4, ts]))
            .map(|(url, _)| url)
            .as_deref(),
        Some("https://live.example/video.m3u8?token=abc")
    );
}

#[test]
fn live_selection_skips_unsupported_codecs_formats_and_missing_urls() {
    let value = data(vec![
        stream("http_stream", "flv", "hevc", 80, "/video.flv"),
        stream("unknown", "flv", "avc", 80, "/video.flv"),
        stream("http_stream", "flv", "avc", 80, ""),
    ]);
    assert!(pick_stream(&value).is_none());
    assert!(pick_stream(&json!({})).is_none());
}

#[test]
fn live_technical_stats_describe_the_selected_stream() {
    let mut selected = stream("http_stream", "flv", "avc", 150, "/low.flv");
    let codec = &mut selected["format"][0]["codec"][0];
    codec["media_info"] = json!({"width": 1280, "height": 720});
    codec["video_codecs"] = json!({"base": "avc1.640028"});
    codec["audio_codecs"] = json!({"base": "mp4a.40.2"});
    let value = data(vec![
        stream("http_stream", "flv", "avc", 400, "/high.flv"),
        selected,
    ]);
    let (url, tech) = pick_stream(&value).unwrap();
    assert_eq!(url, "https://live.example/low.flv?token=abc");
    assert_eq!(tech.quality, "1280×720");
    assert_eq!(tech.vcodec, "avc1.640028");
    assert_eq!(tech.acodec, "mp4a.40.2");
    assert_eq!(tech.vhost, "live.example");
    assert_eq!(tech.ahost, "live.example");
}

#[test]
fn live_quality_uses_description_when_dimensions_are_unavailable() {
    let mut value = data(vec![stream("http_stream", "flv", "avc", 150, "/video.flv")]);
    value["playurl_info"]["playurl"]["g_qn_desc"] = json!([
        {"qn": 400, "desc": "蓝光"},
        {"qn": 150, "desc": "高清"},
    ]);
    let (_, tech) = pick_stream(&value).unwrap();
    assert_eq!(tech.quality, "高清");
    assert_eq!(tech.vcodec, "avc");
    assert_eq!(tech.acodec, "");
    value["playurl_info"]["playurl"]["g_qn_desc"] = json!([]);
    assert_eq!(pick_stream(&value).unwrap().1.quality, "");
}

#[test]
fn following_filter_matches_live_status_parent_and_child_area() {
    let room = json!({"live_status": 1, "area_v2_parent_id": 2, "area_v2_id": 21});
    for (parent_id, id, expected) in [
        (0, 0, true),
        (2, 0, true),
        (2, 21, true),
        (1, 0, false),
        (2, 22, false),
    ] {
        let area = model::LiveArea {
            parent_id,
            id,
            ..Default::default()
        };
        assert_eq!(following_in_area(&room, &area), expected);
    }
    assert!(!following_in_area(
        &json!({"live_status": 0}),
        &model::LiveArea::default()
    ));
    assert!(!following_in_area(&json!({}), &model::LiveArea::default()));
}
