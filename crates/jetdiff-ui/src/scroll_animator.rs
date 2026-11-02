//! Where the diff is scrolled on screen right now. `DiffState` computes where the scroll should
//! end, for example after `n`. Here what is shown is moved there over 170 ms, or at
//! once when it shouldn't be animated.

use std::time::Instant;

use jetdiff_viewmodel::{ScrollAnimation, ScrollState};

/// Duration of an animated jump to a hunk.
const NAV_ANIMATION_MS: f32 = 170.;

#[derive(Default)]
pub struct ScrollAnimator {
    /// During an animation it lags behind the target.
    shown: ScrollState,
    animation: Option<(ScrollAnimation, Instant)>,
}

impl ScrollAnimator {
    pub fn shown(&self) -> ScrollState {
        self.shown
    }

    /// Straight to the target, without animation.
    pub fn jump(&mut self, target: ScrollState) {
        self.animation = None;
        self.shown = target;
    }

    /// From what is shown to the target. In the middle of an animation the new one starts where the previous one is.
    pub fn animate(&mut self, target: ScrollState) {
        if self.shown == target {
            self.animation = None;
            return;
        }
        let animation = ScrollAnimation::new(self.shown, target, NAV_ANIMATION_MS);
        self.animation = Some((animation, Instant::now()));
    }

    pub fn is_animating(&self) -> bool {
        self.animation.is_some()
    }

    /// Stops an animation where it is. Gives the position, which must then become the new target.
    pub fn stop(&mut self) -> Option<ScrollState> {
        self.animation.take().map(|_| self.shown)
    }

    /// Moves what is shown to where the animation is now. `true` while it isn't done.
    pub fn step(&mut self) -> bool {
        let Some((animation, start)) = &self.animation else {
            return false;
        };
        let elapsed_ms = start.elapsed().as_secs_f32() * 1000.;
        let (shown, done) = animation.sample(elapsed_ms);
        self.shown = shown;
        if done {
            self.animation = None;
        }
        !done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: f32) -> ScrollState {
        ScrollState { y: [y, y], x: 0. }
    }

    #[test]
    fn stop_gives_position_only_during_animation() {
        let mut scroll = ScrollAnimator::default();
        scroll.jump(at(100.));
        assert_eq!(scroll.stop(), None);
        scroll.animate(at(500.));
        assert_eq!(scroll.stop(), Some(at(100.)));
        assert_eq!(scroll.stop(), None);
    }

    #[test]
    fn unchanged_scroll_is_not_animated() {
        let mut scroll = ScrollAnimator::default();
        scroll.jump(at(100.));
        scroll.animate(at(100.));
        assert!(!scroll.step());
    }
}
