mod input;
mod mine;
mod settings;
mod view;

impl crate::HomeTab {
    pub fn index(self) -> usize {
        match self {
            Self::Recommended => 0,
            Self::Popular => 1,
            Self::Dynamic => 2,
            Self::Mine => 3,
            Self::Settings => 4,
        }
    }

    pub fn from_index(index: usize) -> Self {
        [
            Self::Recommended,
            Self::Popular,
            Self::Dynamic,
            Self::Mine,
            Self::Settings,
        ][index.min(4)]
    }

    pub fn is_video(self) -> bool {
        matches!(self, Self::Recommended | Self::Popular | Self::Dynamic)
    }
}
