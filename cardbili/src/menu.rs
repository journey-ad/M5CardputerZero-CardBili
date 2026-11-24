use crate::{HomeTab, MenuPosition, Page};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Sections,
    Select,
    SpeedTest,
    Refresh,
    Login,
    Help,
    Back,
    Works,
    Open,
    Author,
    Related,
    Play,
    Detail,
    Retry,
    TogglePlay,
    Like,
    Subtitle,
    Danmaku,
    SuperChat,
    LiveArea,
    More,
    Compose,
    Recognize,
    Send,
    Cancel,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sections => crate::i18n::tr("选集"),
            Self::Select => crate::i18n::tr("选择"),
            Self::SpeedTest => crate::i18n::tr("测速"),
            Self::Refresh => crate::i18n::tr("刷新"),
            Self::Login => crate::i18n::tr("登录"),
            Self::Help => crate::i18n::tr("说明"),
            Self::Back => crate::i18n::tr("返回"),
            Self::Works => crate::i18n::tr("作品"),
            Self::Open => crate::i18n::tr("打开"),
            Self::Author => crate::i18n::tr("作者"),
            Self::Related => crate::i18n::tr("相关"),
            Self::Play => crate::i18n::tr("播放"),
            Self::Detail => crate::i18n::tr("详情"),
            Self::Retry => crate::i18n::tr("重试"),
            Self::TogglePlay => crate::i18n::tr("播放"),
            Self::Like => crate::i18n::tr("点赞"),
            Self::Subtitle => crate::i18n::tr("字幕"),
            Self::Danmaku => crate::i18n::tr("弹幕"),
            Self::SuperChat => crate::i18n::tr("醒目留言"),
            Self::LiveArea => crate::i18n::tr("分区"),
            Self::More => crate::i18n::tr("更多功能"),
            Self::Compose => crate::i18n::tr("发评论"),
            Self::Recognize => crate::i18n::tr("识别"),
            Self::Send => crate::i18n::tr("发送"),
            Self::Cancel => crate::i18n::tr("取消"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Sections => "p",
            Self::Select => "ENTER",
            Self::SpeedTest => "r",
            Self::Refresh | Self::Retry => "r",
            Self::Login => "l",
            Self::Help => "h",
            Self::Back => "ESC",
            Self::Works => "u",
            Self::Author => "u",
            Self::Related => "RELATED",
            Self::Play | Self::Open => "ENTER",
            Self::Detail | Self::TogglePlay => "SPACE",
            Self::Like => "l",
            Self::Subtitle => "SUBTITLE",
            Self::Danmaku => "DANMAKU",
            Self::SuperChat => "SUPER_CHAT",
            Self::LiveArea => "LIVE_AREA",
            Self::More => "m",
            Self::Compose => "COMPOSE",
            Self::Recognize => "6",
            Self::Send => "7",
            Self::Cancel => "8",
        }
    }
}

// 五个槽位依次对应实体 4–8 键，首页菜单使用全部槽位
pub fn actions(
    page: Page,
    tab: HomeTab,
    mine_detail: i32,
    help: bool,
    logged: bool,
    position: MenuPosition,
    retry: bool,
) -> [Option<Action>; 5] {
    use Action::*;
    let mut slots = [None; 5];
    let first = usize::from(page != Page::Home && position == MenuPosition::Left);
    let last = 4;
    if help {
        slots[last] = Some(Back);
        return slots;
    }
    match page {
        Page::Home => {
            if tab != HomeTab::Settings {
                slots[first] = Some(Refresh);
                if !logged {
                    slots[first + 1] = Some(Login);
                }
            }
            if tab.is_video() || mine_detail >= 0 {
                slots[2] = Some(Play);
                if tab != HomeTab::Live {
                    slots[3] = Some(Detail);
                }
            }
            if tab == HomeTab::Live {
                slots[1] = Some(LiveArea);
                if !logged {
                    slots[3] = Some(Login);
                }
            }
            slots[4] = Some(if mine_detail >= 0 { Back } else { Help });
        }
        Page::Player => {
            return [
                Some(TogglePlay),
                Some(Like),
                Some(Subtitle),
                Some(Danmaku),
                Some(More),
            ]
        }
        Page::Detail => return [None, Some(Play), Some(Author), Some(Related), Some(Back)],
        Page::Login => {
            slots[first] = Some(Refresh);
            slots[last] = Some(Back);
        }
        Page::About => slots[last] = Some(Back),
        Page::Favorites => {
            slots[first] = Some(if retry { Retry } else { Open });
            slots[last] = Some(Back);
        }
        Page::Following => {
            slots[first] = Some(if retry { Retry } else { Works });
            slots[last] = Some(Back);
        }
        Page::Comments => {
            slots[3] = Some(Compose);
            if retry {
                slots[first] = Some(Retry);
            }
            slots[last] = Some(Back);
        }
        Page::Collection => {
            slots[first] = Some(if retry { Retry } else { Play });
            slots[first + 1] = Some(Detail);
            slots[last] = Some(Back);
        }
    }
    slots
}

#[cfg(test)]
mod tests;
