//! Where the diff is scrolled on screen right now. `DiffState` computes where the scroll should
//! end, for example after `n`. Here what is shown is moved there with smooth scroll, or at once
//! when it shouldn't be animated.

use std::time::Instant;

use crate::anim::ScrollAnimation;
use crate::scroll::ScrollState;

pub struct ScrollAnimator {
    duration_ms: f32,
    /// The terminal can't show part of a row or column.
    whole_cells: bool,
    /// During an animation it lags behind the target.
    shown: ScrollState,
    animation: Option<(ScrollAnimation, Instant)>,
}

impl ScrollAnimator {
    pub fn new(duration_ms: f32) -> Self {
        Self {
            duration_ms,
            whole_cells: false,
            shown: ScrollState::default(),
            animation: None,
        }
    }

    /// What is shown is always whole rows and columns.
    pub fn in_whole_cells(duration_ms: f32) -> Self {
        Self {
            whole_cells: true,
            ..Self::new(duration_ms)
        }
    }

    pub fn shown(&self) -> ScrollState {
        self.shown
    }

    /// Straight to the target, without animation.
    pub fn jump(&mut self, target: ScrollState) {
        self.animation = None;
        self.shown = self.snap(target);
    }

    /// From what is shown to the target. In the middle of an animation the new one starts where the previous one is.
    pub fn animate(&mut self, target: ScrollState, now: Instant) {
        self.animation = (self.shown != self.snap(target)).then(|| {
            (
                ScrollAnimation::new(self.shown, target, self.duration_ms),
                now,
            )
        });
    }

    pub fn is_animating(&self) -> bool {
        self.animation.is_some()
    }

    /// A new target for what is already happening: an animation carries on there from what is
    /// shown, otherwise it jumps.
    pub fn retarget(&mut self, target: ScrollState, now: Instant) {
        match &self.animation {
            // A cursor move within the view must not start the animation over
            Some((animation, _)) if animation.target() == target => {}
            Some(_) => self.animate(target, now),
            None => self.jump(target),
        }
    }

    /// Stops an animation where it is and makes that the target, before anything else changes the scroll.
    pub fn stop(&mut self, target: &mut ScrollState) {
        if self.animation.take().is_some() {
            *target = self.shown;
        }
    }

    /// Moves what is shown to where the animation is at `now`. `true` when what is shown changed.
    pub fn step(&mut self, now: Instant) -> bool {
        let Some((animation, start)) = &self.animation else {
            return false;
        };
        let elapsed_ms = now.duration_since(*start).as_secs_f32() * 1000.;
        let (sampled, done) = animation.sample(elapsed_ms);
        if done {
            self.animation = None;
        }
        let shown = self.snap(sampled);
        let moved = shown != self.shown;
        self.shown = shown;
        moved
    }

    fn snap(&self, scroll: ScrollState) -> ScrollState {
        if !self.whole_cells {
            return scroll;
        }
        ScrollState {
            y: scroll.y.map(f32::round),
            x: scroll.x.round(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const DURATION_MS: f32 = 250.;

    fn at(y: f32) -> ScrollState {
        ScrollState { y: [y, y], x: 0. }
    }

    fn after(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    fn animator_at(y: f32) -> ScrollAnimator {
        let mut scroll = ScrollAnimator::in_whole_cells(DURATION_MS);
        scroll.jump(at(y));
        scroll
    }

    /// The rows shown from `start` to the end, sampled every millisecond.
    fn frames(scroll: &mut ScrollAnimator, start: Instant) -> Vec<f32> {
        let mut frames = Vec::new();
        for ms in 0..=DURATION_MS as u64 {
            if scroll.step(after(start, ms)) {
                frames.push(scroll.shown().y[0]);
            }
        }
        frames
    }

    #[test]
    fn stop_moves_the_target_only_during_animation() {
        let mut scroll = ScrollAnimator::new(DURATION_MS);
        scroll.jump(at(100.));
        let mut target = at(500.);
        scroll.stop(&mut target);
        assert_eq!(target, at(500.));
        scroll.animate(target, Instant::now());
        scroll.stop(&mut target);
        assert_eq!(target, at(100.));
        assert!(!scroll.is_animating());
    }

    #[test]
    fn retarget_carries_on_only_during_animation() {
        let mut scroll = animator_at(0.);
        let start = Instant::now();
        scroll.retarget(at(50.), start);
        assert_eq!(scroll.shown(), at(50.));
        scroll.animate(at(100.), start);
        scroll.retarget(at(80.), after(start, 1));
        assert!(scroll.is_animating());
        assert_eq!(scroll.shown(), at(50.));
        scroll.retarget(at(80.), after(start, 200));
        assert!(scroll.step(after(start, 200)));
    }

    #[test]
    fn unchanged_scroll_is_not_animated() {
        let mut scroll = ScrollAnimator::new(DURATION_MS);
        scroll.jump(at(100.));
        scroll.animate(at(100.), Instant::now());
        assert!(!scroll.is_animating());
    }

    #[test]
    fn a_jump_of_three_rows_is_three_frames() {
        let mut scroll = animator_at(10.);
        let start = Instant::now();
        scroll.animate(at(13.), start);
        assert_eq!(frames(&mut scroll, start), [11., 12., 13.]);
        assert!(!scroll.is_animating());
    }

    #[test]
    fn a_long_jump_moves_in_steps_and_lands() {
        let mut scroll = animator_at(0.);
        let start = Instant::now();
        scroll.animate(at(200.), start);
        let frames = frames(&mut scroll, start);
        assert!(frames.len() > 1);
        assert!(frames.is_sorted());
        assert_eq!(frames.last(), Some(&200.));
    }

    #[test]
    fn a_new_jump_starts_at_the_row_shown() {
        let mut scroll = animator_at(0.);
        let start = Instant::now();
        scroll.animate(at(100.), start);
        scroll.step(after(start, 50));
        let shown = scroll.shown();
        assert!(shown.y[0] > 0. && shown.y[0] < 100.);
        let restart = after(start, 50);
        scroll.animate(at(300.), restart);
        assert_eq!(scroll.shown(), shown);
        scroll.step(after(restart, 1));
        assert!(scroll.shown().y[0] >= shown.y[0]);
    }

    #[test]
    fn less_than_half_a_row_is_not_animated() {
        let mut scroll = animator_at(10.);
        scroll.animate(at(10.4), Instant::now());
        assert!(!scroll.is_animating());
    }
}
