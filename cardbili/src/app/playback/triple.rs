use slint::ComponentHandle as _;
use super::super::*;
use super::TRIPLE_HOLD_DELAY;

pub(in crate::app) enum TripleFeedback {
    Pending(Instant),
    Complete(Instant),
}

impl App {
    pub(in crate::app) fn finish_triple(&mut self, complete: bool) {
        if matches!(self.controls.triple, Some(TripleFeedback::Pending(_))) {
            self.controls.triple = complete.then(|| TripleFeedback::Complete(Instant::now()));
        }
    }

    pub(in crate::app) fn sync_triple(&mut self) {
        let player = self.ui.global::<crate::Player>();
        let status = self.ui.global::<crate::Status>();
        let (phase, charge, tick) = match self.controls.triple {
            Some(TripleFeedback::Complete(since))
                if since.elapsed() >= Duration::from_millis(700) =>
            {
                self.controls.triple = None;
                (0, 0, 0)
            }
            Some(TripleFeedback::Complete(since)) => (3, 16, since.elapsed().as_millis() / 100),
            Some(TripleFeedback::Pending(since)) => (2, 16, since.elapsed().as_millis() / 100),
            None => match self.controls.like_hold {
                Some((since, false)) if since.elapsed() >= TRIPLE_HOLD_DELAY => {
                    let millis = (since.elapsed() - TRIPLE_HOLD_DELAY).as_millis();
                    (1, (millis / 125).min(16) as i32, millis / 100)
                }
                _ => (0, 0, 0),
            },
        };
        if phase == 1 && player.get_triple_phase() != 1 {
            status.set_bar(crate::i18n::tr("长按一键三连").into());
            status.set_bar_sticky(true);
        }
        player.set_triple_phase(phase);
        player.set_triple_charge(charge);
        player.set_triple_tick((tick % 12) as i32);
    }
}
