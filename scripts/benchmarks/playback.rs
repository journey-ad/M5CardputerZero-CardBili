// 此模块由基准脚本注入临时源码，正式构建保持原有播放路径
use super::*;
use std::cell::RefCell;

struct Samples {
    started: Instant,
    visible: bool,
    next_message: usize,
    values: [Vec<f64>; 5],
    last_frame: Option<Instant>,
}

thread_local! {
    static SAMPLES: RefCell<Samples> = RefCell::new(Samples {
        started: Instant::now(),
        visible: std::env::var("BENCH_CONTROLS").as_deref() == Ok("visible"),
        next_message: 0,
        values: std::array::from_fn(|_| Vec::with_capacity(5000)),
        last_frame: None,
    });
}

pub(super) fn start(app: &mut App) {
    let path = std::env::var("BENCH_MEDIA").unwrap();
    let info = model::PlayInfo {
        title: "音视频性能基准：固定片段与中文表情混排 😀🎉 连续播放测试".into(),
        video_url: path.clone(),
        audio_url: path,
        duration: 80.0,
        live: true,
        ..Default::default()
    };
    app.page = Page::Player;
    app.loading = false;
    app.status.clear();
    app.cookies = None;
    app.settings.record_history = false;
    app.settings.danmaku_enabled = true;
    app.live = Some(model::LiveInfo {
        room_id: 1,
        title: info.title.clone(),
        uname: "基准测试".into(),
        online: "1.2万".into(),
        play_url: info.video_url.clone(),
        started_at: crate::net::now_secs() - 3600,
    });
    let ui = app.ui.global::<crate::Player>();
    ui.set_live(true);
    ui.set_watching("1.2万".into());
    app.player = Some(player::Player::start(
        &info,
        String::new(),
        1,
        app.tx.clone(),
        player::PlaybackState::default(),
    ));
    app.deadline = Some(Instant::now() + Duration::from_secs(35));
    SAMPLES.with(|samples| samples.borrow_mut().started = Instant::now());
}

pub(super) fn tick(app: &mut App) {
    SAMPLES.with(|samples| {
        let mut samples = samples.borrow_mut();
        if samples.visible {
            app.last_input = Instant::now();
        }
        if let Some(player) = &app.player {
            let position = player.position(app.frames_in_seg);
            if position >= samples.next_message as f64 * 0.5 {
                // 固定弹幕时间与内容，所有版本使用相同的文字栅格化负载
                app.live_engine.push(
                    danmaku::Dm {
                        t: position,
                        mode: 1,
                        color: 0xffffff,
                        text: format!("固定弹幕 {} 😀 播放性能测试", samples.next_message),
                    },
                    position,
                );
                samples.next_message += 1;
            }
        }
    });
}

// 前 5 秒预热，随后统计 30 秒内的 UI 同步、像素交接、渲染、显示提交和帧间隔
pub(super) fn record(index: usize, start: Instant) {
    SAMPLES.with(|samples| {
        let mut samples = samples.borrow_mut();
        if samples.started.elapsed().as_secs_f64() >= 5.0 {
            samples.values[index].push(start.elapsed().as_secs_f64() * 1000.0);
        }
    });
}

pub(super) fn frame() {
    SAMPLES.with(|samples| {
        let mut samples = samples.borrow_mut();
        let now = Instant::now();
        if samples.started.elapsed().as_secs_f64() >= 5.0 {
            if let Some(previous) = samples.last_frame {
                samples.values[4].push(now.duration_since(previous).as_secs_f64() * 1000.0);
            }
        }
        samples.last_frame = Some(now);
    });
}

pub(super) fn finish() {
    SAMPLES.with(|samples| {
        let mut samples = samples.borrow_mut();
        let names = ["sync", "handoff", "render", "submit", "frame_interval"];
        let mut report = serde_json::Map::new();
        for (name, values) in names.iter().zip(&mut samples.values) {
            values.sort_by(f64::total_cmp);
            if values.is_empty() {
                continue;
            }
            report.insert(
                (*name).into(),
                serde_json::json!({
                    "count": values.len(),
                    "mean_ms": values.iter().sum::<f64>() / values.len() as f64,
                    "p95_ms": values[((values.len() - 1) as f64 * 0.95).ceil() as usize],
                    "max_ms": values.last().unwrap(),
                    "over_100ms": values.iter().filter(|&&value| value > 100.0).count(),
                }),
            );
        }
        println!("BENCH {}", serde_json::Value::Object(report));
    });
}
