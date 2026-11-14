mod input;
mod messages;
mod mine;
mod settings;
mod view;

// 页签的显示顺序，直播排在动态之后
pub const TAB_ORDER: [crate::HomeTab; 6] = [
    crate::HomeTab::Recommended,
    crate::HomeTab::Popular,
    crate::HomeTab::Dynamic,
    crate::HomeTab::Live,
    crate::HomeTab::Mine,
    crate::HomeTab::Settings,
];

// collection 页复用视频卡片管线时使用的独立数据槽位
pub const COLLECTION_SOURCE: usize = 6;

impl crate::HomeTab {
    // 数据槽位顺序：与列表缓存数组下标一致；键盘切换用的是上面的显示顺序 TAB_ORDER
    const STORAGE_ORDER: [crate::HomeTab; 6] = [
        crate::HomeTab::Recommended,
        crate::HomeTab::Popular,
        crate::HomeTab::Dynamic,
        crate::HomeTab::Mine,
        crate::HomeTab::Settings,
        crate::HomeTab::Live,
    ];

    pub fn index(self) -> usize {
        Self::STORAGE_ORDER
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }

    pub fn from_index(index: usize) -> Self {
        Self::STORAGE_ORDER[index.min(Self::STORAGE_ORDER.len() - 1)]
    }

    pub fn cycle(self, step: i32) -> Self {
        let position = TAB_ORDER
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default() as i32;
        TAB_ORDER[(position + step).rem_euclid(TAB_ORDER.len() as i32) as usize]
    }

    pub fn is_video(self) -> bool {
        matches!(
            self,
            Self::Recommended | Self::Popular | Self::Dynamic | Self::Live
        )
    }
}
