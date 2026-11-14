use slint::ComponentHandle as _;
use super::*;

#[derive(Debug)]
pub(super) enum Action {
    ClearCache,
    #[cfg(all(feature = "desktop", windows))]
    FfmpegInstall,
    #[cfg(all(feature = "desktop", not(windows)))]
    FfmpegNotice,
}

impl App {
    pub(super) fn confirm(&mut self, action: Action, title: &str, message: &str, label: &str) {
        let confirm = self.ui.global::<crate::Confirm>();
        crate::diagnostics::write(format_args!("[确认] 打开 {action:?}"));
        self.confirmation = Some(action);
        self.direction_hold = None;
        self.return_hold = None;
        confirm.set_title(title.into());
        confirm.set_message(message.into());
        confirm.set_label(label.into());
        confirm.set_selected(false);
        confirm.set_open(true);
    }

    pub(super) fn confirmation_key(&mut self, key: &str) {
        let confirm = self.ui.global::<crate::Confirm>();
        match key {
            "LEFT" | "z" => confirm.set_selected(false),
            "RIGHT" | "c" => confirm.set_selected(true),
            "TAB" | "BACKTAB" => {
                confirm.set_selected(!confirm.get_selected());
            }
            "ENTER" => self.finish_confirmation(confirm.get_selected()),
            "ESC" | "ESC_HOLD" | "BACKSPACE" | "8" => self.finish_confirmation(false),
            _ => {}
        }
    }

    pub(super) fn finish_confirmation(&mut self, accepted: bool) {
        let Some(action) = self.confirmation.take() else {
            return;
        };
        self.ui.global::<crate::Confirm>().set_open(false);
        crate::diagnostics::write(format_args!("[确认] {action:?} 确认={accepted}"));
        if accepted {
            match action {
                Action::ClearCache => self.clear_cache(),
                #[cfg(all(feature = "desktop", windows))]
                Action::FfmpegInstall => {
                    crate::diagnostics::install_ffmpeg();
                }
                #[cfg(all(feature = "desktop", not(windows)))]
                Action::FfmpegNotice => {}
            }
        }
    }
}
