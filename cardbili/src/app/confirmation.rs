use slint::ComponentHandle as _;
use super::*;

#[derive(Debug)]
pub(super) enum Action {
    ClearCache,
}

impl App {
    pub(super) fn confirm(&mut self, action: Action, title: &str, message: &str, label: &str) {
        crate::diagnostics::write(format_args!("[确认] 打开 {action:?}"));
        self.confirmation = Some(action);
        self.direction_hold = None;
        self.return_hold = None;
        self.ui.global::<crate::Confirm>().set_title(title.into());
        self.ui.global::<crate::Confirm>().set_message(message.into());
        self.ui.global::<crate::Confirm>().set_label(label.into());
        self.ui.global::<crate::Confirm>().set_selected(false);
        self.ui.global::<crate::Confirm>().set_open(true);
    }

    pub(super) fn confirmation_key(&mut self, key: &str) {
        match key {
            "LEFT" | "z" => self.ui.global::<crate::Confirm>().set_selected(false),
            "RIGHT" | "c" => self.ui.global::<crate::Confirm>().set_selected(true),
            "TAB" | "BACKTAB" => {
                self.ui.global::<crate::Confirm>().set_selected(!self.ui.global::<crate::Confirm>().get_selected());
            }
            "ENTER" => self.finish_confirmation(self.ui.global::<crate::Confirm>().get_selected()),
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
            }
        }
    }
}
