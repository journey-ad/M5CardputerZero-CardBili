use super::*;

#[test]
fn user_id_requires_complete_cookie_name_and_skips_empty_values() {
    assert_eq!(
        parse_mid("SESSDATA=dummy; DedeUserID=123; bili_jct=dummy"),
        "123"
    );
    assert_eq!(parse_mid("DedeUserID=; DedeUserID=456"), "456");
    assert_eq!(parse_mid("otherDedeUserID=123; DedeUserID_extra=456"), "");
    assert_eq!(parse_mid(""), "");
}

#[test]
fn key_release_normalizes_direction_and_hold_aliases() {
    for (keys, expected) in [
        (&["LEFT", "z"][..], "LEFT_UP"),
        (&["RIGHT", "c"][..], "RIGHT_UP"),
        (&["UP", "f"][..], "UP_UP"),
        // W / E 在播放页是快退 / 快进，松开名由界面层按页面换算
        (&["w"][..], "W_UP"),
        (&["e"][..], "E_UP"),
        (&["d"][..], "D_UP"),
        (&["DOWN", "x", "s"][..], "DOWN_UP"),
        (&["l", "5"][..], "LIKE_UP"),
        (&["6"][..], "6_UP"),
        (&["SPACE"][..], "SPACE_UP"),
        (&["7"][..], "7_UP"),
        (&["8"][..], "8_UP"),
    ] {
        for key in keys {
            assert_eq!(key_release(key), Some(expected));
        }
    }
    assert_eq!(key_release("ENTER"), None);
}

#[test]
fn estimated_width_counts_emoji_sequences_as_one_glyph() {
    assert_eq!(est_w("", 12.0), 0.0);
    assert!((est_w("Ab中", 12.0) - 25.2).abs() < 1e-9);
    assert_eq!(est_w("😀", 12.0), 14.0);
}
