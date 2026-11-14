mod messages;
use std::time::Duration;

const TRIPLE_HOLD_DELAY: Duration = Duration::from_millis(300);

mod actions;
pub(super) use actions::Controls;
mod input;
mod tick;
mod triple;
mod view;
pub(super) use triple::TripleFeedback;
