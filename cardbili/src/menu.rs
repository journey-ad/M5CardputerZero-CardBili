#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Refresh,
    Login,
    Help,
    Back,
    Works,
    Author,
    Related,
    Play,
    Detail,
    Retry,
    TogglePlay,
    Like,
    Speed,
    Danmaku,
    Comments,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Refresh => "刷新",
            Self::Login => "登录",
            Self::Help => "说明",
            Self::Back => "返回",
            Self::Works => "作品",
            Self::Author => "作者",
            Self::Related => "相关",
            Self::Play => "播放",
            Self::Detail => "详情",
            Self::Retry => "重试",
            Self::TogglePlay => "播放",
            Self::Like => "点赞",
            Self::Speed => "倍速",
            Self::Danmaku => "弹幕",
            Self::Comments => "评论",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Refresh | Self::Retry => "r",
            Self::Login => "l",
            Self::Help => "h",
            Self::Back => "ESC",
            Self::Works => "u",
            Self::Author => "u",
            Self::Related => "r",
            Self::Play => "ENTER",
            Self::Detail | Self::TogglePlay => "SPACE",
            Self::Like => "l",
            Self::Speed => "b",
            Self::Danmaku => "d",
            Self::Comments => "r",
        }
    }
}

// 五个槽位依次对应实体 4–8 键，首页菜单使用全部槽位
pub fn actions(
    page: i32,
    tab: usize,
    mine_detail: i32,
    help: bool,
    logged: bool,
    position: i32,
    retry: bool,
) -> [Option<Action>; 5] {
    use Action::*;
    let mut slots = [None; 5];
    let first = usize::from(page != 0 && position == 1);
    let last = if page != 0 && position == 2 { 3 } else { 4 };
    if help {
        slots[last] = Some(Back);
        return slots;
    }
    match page {
        0 => {
            if tab < 4 {
                slots[first] = Some(Refresh);
                if !logged {
                    slots[first + 1] = Some(Login);
                }
            }
            if tab < 3 || mine_detail >= 0 {
                slots[2] = Some(Play);
                slots[3] = Some(Detail);
            }
            slots[4] = Some(if mine_detail >= 0 { Back } else { Help });
        }
        1 => {
            return [
                Some(TogglePlay),
                Some(Like),
                Some(Speed),
                Some(Danmaku),
                Some(Comments),
            ]
        }
        3 => {
            slots[1] = Some(Author);
            slots[2] = Some(Related);
            slots[last] = Some(Back);
        }
        2 => {
            slots[first] = Some(Refresh);
            slots[last] = Some(Back);
        }
        4 => slots[last] = Some(Back),
        7 => {
            slots[first] = Some(if retry { Retry } else { Works });
            slots[last] = Some(Back);
        }
        6 => {
            if retry {
                slots[first] = Some(Retry);
            }
            slots[last] = Some(Back);
        }
        5 => {
            slots[first] = Some(if retry { Retry } else { Play });
            slots[first + 1] = Some(Detail);
            slots[last] = Some(Back);
        }
        _ => {}
    }
    slots
}
