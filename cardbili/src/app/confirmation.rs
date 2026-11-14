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
        self.ui.set_confirm_title(title.into());
        self.ui.set_confirm_message(message.into());
        self.ui.set_confirm_label(label.into());
        self.ui.set_confirm_selected(false);
        self.ui.set_confirm_open(true);
    }

    pub(super) fn confirmation_key(&mut self, key: &str) {
        match key {
            "LEFT" | "z" => self.ui.set_confirm_selected(false),
            "RIGHT" | "c" => self.ui.set_confirm_selected(true),
            "TAB" | "BACKTAB" => {
                self.ui
                    .set_confirm_selected(!self.ui.get_confirm_selected());
            }
            "ENTER" => self.finish_confirmation(self.ui.get_confirm_selected()),
            "ESC" | "ESC_HOLD" | "BACKSPACE" | "8" => self.finish_confirmation(false),
            _ => {}
        }
    }

    pub(super) fn finish_confirmation(&mut self, accepted: bool) {
        let Some(action) = self.confirmation.take() else {
            return;
        };
        self.ui.set_confirm_open(false);
        crate::diagnostics::write(format_args!("[确认] {action:?} 确认={accepted}"));
        if accepted {
            match action {
                Action::ClearCache => self.clear_cache(),
            }
        }
    }
}
