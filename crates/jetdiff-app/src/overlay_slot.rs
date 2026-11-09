//! What lies over the diff: the help, or a rollback waiting for Enter. Only one at a time.

use crate::actions::RollbackRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Rollback(RollbackRequest),
    Help,
}

#[derive(Debug, Default)]
pub struct OverlaySlot(Option<Overlay>);

impl OverlaySlot {
    pub fn current(&self) -> Option<&Overlay> {
        self.0.as_ref()
    }

    pub fn ask_rollback(&mut self, request: RollbackRequest) {
        self.0 = Some(Overlay::Rollback(request));
    }

    /// While a rollback waits for an answer, the next key is the answer: Enter confirms, anything
    /// else cancels. Either way the dialog closes.
    pub fn take_rollback(&mut self) -> Option<RollbackRequest> {
        match self.0.take() {
            Some(Overlay::Rollback(request)) => Some(request),
            other => {
                self.0 = other;
                None
            }
        }
    }

    /// The hunk waiting for Enter is from content that is replaced.
    pub fn cancel_rollback(&mut self) {
        self.take_rollback();
    }

    /// Also cancels a rollback, which otherwise doesn't see the key.
    pub fn toggle_help(&mut self) {
        self.0 = match self.0 {
            Some(Overlay::Help) => None,
            _ => Some(Overlay::Help),
        };
    }

    /// `false` when help is not shown, so Esc passes through to the rollback.
    pub fn hide_help(&mut self) -> bool {
        if self.0 != Some(Overlay::Help) {
            return false;
        }
        self.0 = None;
        true
    }

    pub fn close(&mut self) {
        self.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> RollbackRequest {
        RollbackRequest {
            before: "a\n".into(),
            lines: [0..1, 0..1],
        }
    }

    #[test]
    fn a_rollback_is_answered_once() {
        let mut slot = OverlaySlot::default();
        slot.ask_rollback(request());
        assert_eq!(slot.take_rollback(), Some(request()));
        assert_eq!(slot.current(), None);
        assert_eq!(slot.take_rollback(), None);
    }

    #[test]
    fn new_content_cancels_the_rollback_but_not_help() {
        let mut slot = OverlaySlot::default();
        slot.ask_rollback(request());
        slot.cancel_rollback();
        assert_eq!(slot.current(), None);
        slot.toggle_help();
        slot.cancel_rollback();
        assert_eq!(slot.current(), Some(&Overlay::Help));
    }

    #[test]
    fn esc_passes_through_unless_help_is_shown() {
        let mut slot = OverlaySlot::default();
        slot.ask_rollback(request());
        assert!(!slot.hide_help());
        slot.toggle_help();
        assert!(slot.hide_help());
        assert_eq!(slot.current(), None);
    }
}
