//! Grabbing and dragging the diff divider with the mouse. crossterm has no double click, so two presses close
//! together count as one.

use std::time::{Duration, Instant};

const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// A press this many columns beside the diff divider also grabs it, since one column is hard to hit.
const GRAB_COLUMNS: u16 = 1;

#[derive(Default)]
pub struct DiffDividerGrab {
    dragging: bool,
    /// The first press of what may become a double click.
    last_press: Option<Instant>,
    /// How far right of the divider it was grabbed, so it doesn't jump to the mouse.
    grab_offset: i16,
}

/// How far right of the diff divider in column `divider` a press in column `x` grabs it. `None` when
/// it is too far away.
pub fn grab_offset(divider: u16, x: u16) -> Option<i16> {
    (x.abs_diff(divider) <= GRAB_COLUMNS).then(|| x as i16 - divider as i16)
}

impl DiffDividerGrab {
    /// A press that grabs the diff divider, `grab_offset` right of it. `true` when it is the second
    /// press of a double click.
    pub fn press(&mut self, now: Instant, grab_offset: i16) -> bool {
        self.dragging = true;
        self.grab_offset = grab_offset;
        let double_click = self
            .last_press
            .is_some_and(|last_press| now.duration_since(last_press) <= DOUBLE_CLICK);
        self.last_press = (!double_click).then_some(now);
        double_click
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// The column the divider is dragged to with the mouse in column `x`. The divider moved, so the
    /// next press starts a new drag and not a double click.
    pub fn drag_to(&mut self, x: u16) -> u16 {
        self.last_press = None;
        x.saturating_add_signed(-self.grab_offset)
    }

    pub fn release(&mut self) {
        self.dragging = false;
    }

    /// A press somewhere else. Also ends a drag whose release was outside the terminal.
    pub fn press_elsewhere(&mut self) {
        self.release();
        self.last_press = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_presses_close_together_are_a_double_click() {
        let start = Instant::now();
        let mut grab = DiffDividerGrab::default();
        assert!(!grab.press(start, 0));
        assert!(grab.is_dragging());
        grab.release();
        assert!(!grab.is_dragging());
        assert!(grab.press(start + Duration::from_millis(300), 0));
        // A third press starts over
        assert!(!grab.press(start + Duration::from_millis(400), 0));
    }

    #[test]
    fn presses_far_apart_are_not_a_double_click() {
        let start = Instant::now();
        let mut grab = DiffDividerGrab::default();
        assert!(!grab.press(start, 0));
        assert!(!grab.press(start + Duration::from_millis(500), 0));
        assert!(grab.press(start + Duration::from_millis(600), 0));
    }

    #[test]
    fn a_press_beside_the_divider_grabs_it() {
        assert_eq!(grab_offset(20, 19), Some(-1));
        assert_eq!(grab_offset(20, 20), Some(0));
        assert_eq!(grab_offset(20, 21), Some(1));
        assert_eq!(grab_offset(20, 22), None);
        assert_eq!(grab_offset(0, 1), Some(1));
        let mut grab = DiffDividerGrab::default();
        grab.press(Instant::now(), 1);
        // Grabbed one column right of it, so the divider stays one column left of the mouse
        assert_eq!(grab.drag_to(31), 30);
    }

    #[test]
    fn a_press_after_a_drag_is_not_a_double_click() {
        let start = Instant::now();
        let mut grab = DiffDividerGrab::default();
        assert!(!grab.press(start, 0));
        assert_eq!(grab.drag_to(10), 10);
        grab.release();
        assert!(!grab.press(start + Duration::from_millis(300), 0));
    }

    #[test]
    fn a_press_elsewhere_in_between_is_not_a_double_click() {
        let start = Instant::now();
        let mut grab = DiffDividerGrab::default();
        assert!(!grab.press(start, 0));
        grab.press_elsewhere();
        assert!(!grab.is_dragging());
        assert!(!grab.press(start + Duration::from_millis(100), 0));
    }
}
