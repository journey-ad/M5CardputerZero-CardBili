use super::*;
use std::time::Duration;

#[test]
fn graphs_start_together_at_the_left_edge_and_record_zero_samples() {
    let mut stats = StatsState::default();
    let now = Instant::now();
    assert_eq!(stats.record(now, None, Some(0.0)), None);
    assert_eq!(stats.cursor, None);
    assert_eq!(stats.record(now, Some((1, 1000)), Some(0.1)), None);
    assert_eq!(stats.cursor, None);
    assert_eq!(
        stats.record(now + Duration::from_secs(1), Some((1, 1000)), Some(0.0)),
        Some(0.0)
    );
    assert_eq!(stats.cursor, None);
    assert_eq!(
        stats.record(now + Duration::from_secs(2), Some((1, 5000)), Some(0.0)),
        Some(4000.0)
    );
    assert_eq!(stats.cursor, None);
    assert!(stats.connection.iter().all(Option::is_none));
    assert!(stats.buffer.iter().all(Option::is_none));
    assert_eq!(
        stats.record(now + Duration::from_secs(3), Some((1, 6000)), Some(0.4)),
        Some(1000.0)
    );
    assert_eq!(stats.connection[..2], [Some(1000.0); 2]);
    assert_eq!(stats.buffer[..2], [Some(0.4); 2]);
    assert_eq!(stats.cursor, Some(2));
    assert_eq!(
        stats.record(now + Duration::from_secs(4), Some((1, 6000)), Some(0.0)),
        Some(0.0)
    );
    assert_eq!(stats.connection[2..4], [Some(0.0); 2]);
    assert_eq!(stats.buffer[2..4], [Some(0.0); 2]);
    assert_eq!(stats.cursor, Some(4));
    assert_eq!(bitrate_text(0.0), "0 Kbps");

    let mut stats = StatsState::default();
    stats.record(now, Some((1, 1000)), Some(1.0));
    stats.record(now + Duration::from_secs(1), Some((1, 1000)), Some(1.0));
    assert_eq!(stats.cursor, Some(2));
    assert_eq!(stats.connection[0], Some(0.0));
    assert_eq!(stats.buffer[0], Some(1.0));
}

#[test]
fn decoder_restart_and_missing_telemetry_reset_the_speed_baseline() {
    let mut stats = StatsState::default();
    let now = Instant::now();
    stats.record(now, Some((1, 5000)), Some(0.5));
    assert_eq!(
        stats.record(now + Duration::from_secs(1), Some((1, 6000)), Some(0.4)),
        Some(1000.0)
    );
    assert_eq!(
        stats.record(now + Duration::from_secs(2), Some((2, 100)), Some(0.0)),
        None
    );
    assert_eq!(
        stats.record(now + Duration::from_secs(3), Some((2, 1100)), Some(0.2)),
        Some(1000.0)
    );
    assert_eq!(stats.record(now + Duration::from_secs(4), None, None), None);
    assert_eq!(
        stats.record(now + Duration::from_secs(5), Some((2, 3100)), Some(0.1)),
        None
    );
    assert_eq!(stats.buffer[6], None);
    assert_eq!(stats.buffer[0], Some(0.4));
    assert_eq!(stats.buffer[2], Some(0.0));
    assert_eq!(stats.buffer[4], Some(0.2));
    assert_eq!(stats.connection[0], Some(1000.0));
    assert_eq!(stats.connection[2], None);
    assert_eq!(stats.connection[4], Some(1000.0));
    assert_eq!(stats.connection[6], None);
    assert_eq!(stats.connection[8], None);
    assert_eq!(stats.buffer[8], Some(0.1));
    assert_eq!(stats.cursor, Some(10));
}

#[test]
fn graph_wraps_with_a_visible_cursor_and_preserves_empty_samples() {
    let mut stats = StatsState::default();
    let now = Instant::now();
    for index in 0..=SLOTS / 2 + 1 {
        stats.record(
            now + Duration::from_secs(index as u64),
            Some((1, index as u64 * 1000)),
            Some(20.0),
        );
    }
    let graph = build(
        &stats.buffer,
        stats.cursor.unwrap(),
        &BUFFER_THRESHOLDS,
        &BUFFER_COLORS,
    );
    assert_eq!(stats.cursor, Some(2));
    assert_eq!(graph.row_data(0).unwrap().fill, CHART_HEIGHT);
    assert_eq!(graph.row_data(2).unwrap().fill, 0.0);
    let speed_graph = build(
        &stats.connection,
        stats.cursor.unwrap(),
        &BANDWIDTH_THRESHOLDS,
        &BANDWIDTH_COLORS,
    );
    assert!(speed_graph.row_data(0).unwrap().fill > 0.0);
    assert_eq!(speed_graph.row_data(2).unwrap().fill, 0.0);
}
