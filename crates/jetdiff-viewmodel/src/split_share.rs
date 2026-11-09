//! The share of the width Left gets in Split, set by dragging the diff divider. It is a share and not
//! a width, so it follows window size and zoom.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitShare(f32);

impl Default for SplitShare {
    fn default() -> Self {
        Self(0.5)
    }
}

impl SplitShare {
    /// The share that puts the middle of the diff divider at `x`, in a split `width` wide from 0.
    /// No min width, so a side can get zero.
    pub fn at(x: f32, width: f32, divider_width: f32) -> Self {
        let sides_width = width - divider_width;
        if sides_width <= 0. {
            return Self::default();
        }
        Self(((x - divider_width / 2.) / sides_width).clamp(0., 1.))
    }

    /// How much Left gets of the width of both sides together.
    pub fn left_side_width(self, sides_width: f32) -> f32 {
        sides_width * self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_middle_of_the_divider_follows_the_mouse() {
        let share = SplitShare::at(310., 1020., 20.);
        assert_eq!(share.left_side_width(1000.), 300.);
        assert_eq!(SplitShare::at(510., 1020., 20.), SplitShare::default());
    }

    #[test]
    fn a_side_can_get_zero() {
        assert_eq!(SplitShare::at(-50., 1020., 20.).left_side_width(1000.), 0.);
        assert_eq!(
            SplitShare::at(2000., 1020., 20.).left_side_width(1000.),
            1000.
        );
    }

    #[test]
    fn no_room_for_the_sides_keeps_the_middle() {
        assert_eq!(SplitShare::at(5., 10., 20.), SplitShare::default());
    }
}
