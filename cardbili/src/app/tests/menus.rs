use super::*;

#[test]
fn synchronized_menu_tracks_login_and_help_changes() {
    let mut h = Harness::new();
    h.sync();
    assert_eq!(h.labels(), ["刷新", "登录", "播放", "详情", "说明"]);
    h.app.cookies = Some("DedeUserID=1".into());
    h.sync();
    assert_eq!(h.labels(), ["刷新", "", "播放", "详情", "说明"]);
    h.key("8");
    h.sync();
    assert_eq!(h.labels(), ["", "", "", "", "返回"]);
    h.key("8");
    assert!(h.app.help);
    h.key("8_UP");
    h.sync();
    assert!(!h.app.help);
    assert_eq!(h.labels()[4], "说明");
}

#[test]
fn overlays_replace_background_menu_and_restore_it_on_close() {
    let mut h = Harness::new();
    h.app.ui.global::<crate::Cdn>().set_open(true);
    h.sync();
    assert_eq!(h.labels(), ["测速", "选择", "", "", "返回"]);
    h.app.ui.global::<crate::LiveAreas>().set_open(true);
    h.sync();
    assert_eq!(h.labels(), ["刷新", "选择", "", "", "返回"]);
    h.key("8");
    h.key("8_UP");
    h.sync();
    assert_eq!(h.labels(), ["测速", "选择", "", "", "返回"]);
    h.key("8");
    h.key("8_UP");
    h.sync();
    assert_eq!(h.labels(), ["刷新", "登录", "播放", "详情", "说明"]);
}

#[test]
fn live_player_menu_tracks_super_chat_availability_and_popup() {
    let mut h = Harness::new();
    h.app.page = Page::Player;
    h.app.live = Some(model::LiveInfo {
        room_id: 42,
        ..Default::default()
    });
    h.sync();
    assert_eq!(h.labels(), ["播放", "刷新", "", "弹幕", "更多功能"]);
    h.app.ui.global::<crate::SuperChat>().set_available(true);
    h.sync();
    assert_eq!(h.labels(), ["播放", "刷新", "醒目留言", "弹幕", "更多功能"]);
    h.app.ui.global::<crate::SuperChat>().set_open(true);
    h.sync();
    assert_eq!(h.labels(), ["", "", "", "", "返回"]);
    h.key("8");
    assert!(!h.app.ui.global::<crate::SuperChat>().get_open());
    assert!(!h.app.go_back);
}

#[test]
fn player_more_menu_opens_stats_for_live_and_video() {
    for live in [false, true] {
        let mut h = Harness::new();
        h.app.page = Page::Player;
        h.app.ui.global::<crate::Player>().set_live(live);
        if live {
            h.app.live = Some(model::LiveInfo {
                room_id: 42,
                tech: model::TechStats {
                    quality: "1280×720".into(),
                    vcodec: "avc1.640028".into(),
                    acodec: "mp4a.40.2".into(),
                    vhost: "live.example".into(),
                    ..Default::default()
                },
                ..Default::default()
            });
        } else {
            h.app.current_play = Some(model::PlayInfo {
                bvid: "BV-test".into(),
                cid: 42,
                ..Default::default()
            });
        }
        h.app
            .ui
            .global::<crate::Player>()
            .set_controls_visible(false);
        h.key("8");
        assert!(h.app.ui.global::<crate::Player>().get_menu_open());
        h.key("UP");
        assert_eq!(
            h.app.ui.global::<crate::Player>().get_menu_index(),
            if live { 0 } else { 3 }
        );
        h.key("DOWN");
        assert_eq!(h.app.ui.global::<crate::Player>().get_menu_index(), 0);
        h.key("UP");
        h.key("ENTER");
        assert!(h.app.ui.global::<crate::Player>().get_stats_on());
        assert!(!h.app.ui.global::<crate::Player>().get_menu_open());
        assert!(h.app.requested_live.is_none());
        h.sync();
        let player = h.app.ui.global::<crate::Player>();
        let lines = player.get_stat_head();
        let first = lines.row_data(0).unwrap();
        assert_eq!(first.label, if live { "Room ID" } else { "Video Info" });
        assert_eq!(first.value, if live { "42" } else { "BV-test:42" });
        assert_eq!(player.get_connection_samples().row_count(), 150);
        assert_eq!(player.get_buffer_samples().row_count(), 150);
        assert_eq!(player.get_buffer_value(), "");
        assert_eq!(player.get_connection_value(), "");
        let tail = player.get_stat_tail();
        assert_eq!(tail.row_data(0).unwrap().label, "Debug");
        assert_eq!(tail.row_data(1).unwrap().label, "Player");
        assert_eq!(tail.row_data(2).unwrap().label, "Date");
        if live {
            assert_eq!(lines.row_data(1).unwrap().value, "1280x720");
            assert_eq!(
                lines.row_data(2).unwrap().value,
                "video/avc1.640028, audio/mp4a.40.2"
            );
            assert_eq!(lines.row_data(3).unwrap().value, "live.example");
        } else {
            assert_eq!(lines.row_count(), 1);
            let tech = &mut h.app.current_play.as_mut().unwrap().tech;
            tech.quality = "640×360".into();
            tech.fps = "30.00".into();
            tech.vcodec = "avc".into();
            tech.vbandwidth = 800_000;
            h.app.stats = Default::default();
            h.sync();
            let lines = h.app.ui.global::<crate::Player>().get_stat_head();
            assert_eq!(lines.row_count(), 3);
            assert_eq!(lines.row_data(1).unwrap().value, "640x360@30.00");
            assert_eq!(lines.row_data(2).unwrap().value, "video/avc");
            h.app.current_play.as_mut().unwrap().tech.abandwidth = 128_000;
            h.app.stats = Default::default();
            h.sync();
            let lines = h.app.ui.global::<crate::Player>().get_stat_head();
            assert_eq!(lines.row_data(3).unwrap().label, "Bitrate");
            assert_eq!(lines.row_data(3).unwrap().value, "928 Kbps");
        }
        h.key("ESC");
        assert!(!h.app.ui.global::<crate::Player>().get_stats_on());
        assert!(!h.app.go_back);
    }
}

#[test]
fn live_more_and_stats_buttons_dispatch_pointer_callbacks() {
    use slint::platform::{PointerEventButton, WindowEvent};

    let mut h = Harness::new();
    h.app.page = Page::Player;
    h.app.live = Some(model::LiveInfo::default());
    h.app.ui.global::<crate::Player>().set_live(true);
    h.sync();
    for (x, y) in [(275.0, 158.0), (160.0, 85.0)] {
        h.render();
        let position = slint::LogicalPosition::new(x, y);
        h.window
            .dispatch_event(WindowEvent::PointerMoved { position });
        h.window.dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        h.window.dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
        h.dispatch_callbacks();
        if y > 100.0 {
            assert!(h.app.ui.global::<crate::Player>().get_menu_open());
        }
    }
    assert!(h.app.ui.global::<crate::Player>().get_stats_on());
    assert!(!h.app.ui.global::<crate::Player>().get_menu_open());
    assert!(h.app.requested_live.is_none());
}

#[test]
fn sections_menu_routes_play_and_return_to_the_panel() {
    for page in [Page::Detail, Page::Player] {
        let mut h = Harness::new();
        h.app.page = page;
        h.app.sections.open = true;
        h.sync();
        assert_eq!(h.labels(), ["", "播放", "", "", "返回"]);
        h.key("8");
        h.key("8_UP");
        assert!(!h.app.sections.open);
        assert!(!h.app.ui.global::<crate::Player>().get_menu_open());
        assert!(!h.app.go_back);
    }
}

#[test]
fn detail_menu_shows_sections_and_hides_redundant_author_link() {
    let mut h = Harness::new();
    h.app.page = Page::Detail;
    h.app.sections.chapters = vec![model::Chapter {
        title: "第一章".into(),
        start: 0.0,
    }];
    h.app
        .back_stack
        .push(BackTarget::Collection(CollectionState::default()));
    h.sync();
    assert_eq!(h.labels(), ["选集", "播放", "", "相关", "返回"]);
    h.key("4");
    assert!(h.app.sections.open);
}

#[test]
fn retry_menu_reflects_each_pages_own_error() {
    let mut h = Harness::new();
    h.app.settings.menu_position = MenuPosition::Left;
    for page in [
        Page::Comments,
        Page::Following,
        Page::Favorites,
        Page::Collection,
    ] {
        h.app.page = page;
        h.app.comment_page.error = "failed".into();
        h.app.following.error = "failed".into();
        h.app.favorites.error = "failed".into();
        h.app.collection.error = "failed".into();
        h.sync();
        assert_eq!(h.labels()[0], "");
        assert_eq!(h.labels()[1], "重试", "page={page:?}");
        assert_eq!(h.labels()[4], "返回");
    }
}
