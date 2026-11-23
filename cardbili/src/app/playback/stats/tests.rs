use super::*;
use std::time::Duration;

#[test]
fn samples_use_decoder_bytes_and_measured_queue_duration() {
    let mut stats = StatsState::default();
    let now = Instant::now();
    assert_eq!(stats.record(now, Some((1, 1000)), Some(0.1)), None);
    assert_eq!(
        stats.record(now + Duration::from_secs(2), Some((1, 5000)), Some(0.4)),
        Some(2000.0)
    );
    assert_eq!(stats.connection[2], Some(2000.0));
    assert_eq!(stats.buffer[2], Some(0.4));
    assert_eq!(stats.cursor, 4);
    assert_eq!(
        stats.record(now + Duration::from_secs(3), Some((1, 5000)), Some(0.0)),
        Some(0.0)
    );
    assert_eq!(stats.connection[4], Some(0.0));
    assert_eq!(stats.buffer[4], Some(0.0));
    assert_eq!(bitrate_text(0.0), "0 Kbps");
}

#[test]
fn decoder_restart_and_missing_telemetry_reset_the_speed_baseline() {
    let mut stats = StatsState::default();
    let now = Instant::now();
    stats.record(now, Some((1, 5000)), Some(0.5));
    assert_eq!(
        stats.record(now + Duration::from_secs(1), Some((2, 100)), Some(0.0)),
        None
    );
    assert_eq!(
        stats.record(now + Duration::from_secs(2), Some((2, 1100)), Some(0.2)),
        Some(1000.0)
    );
    assert_eq!(stats.record(now + Duration::from_secs(3), None, None), None);
    assert_eq!(
        stats.record(now + Duration::from_secs(4), Some((2, 3100)), Some(0.1)),
        None
    );
    assert_eq!(stats.buffer[6], None);
}

#[test]
fn graph_wraps_with_a_visible_cursor_and_preserves_empty_samples() {
    let mut stats = StatsState::default();
    let now = Instant::now();
    for index in 0..=SLOTS / 2 {
        stats.record(
            now + Duration::from_secs(index as u64),
            Some((1, index as u64 * 1000)),
            Some(0.5),
        );
    }
    let graph = build(
        &stats.buffer,
        stats.cursor,
        &BUFFER_THRESHOLDS,
        &BUFFER_COLORS,
    );
    assert_eq!(stats.cursor, 2);
    assert_eq!(graph.row_data(0).unwrap().fill, CHART_HEIGHT);
    assert_eq!(graph.row_data(2).unwrap().fill, 0.0);
}
