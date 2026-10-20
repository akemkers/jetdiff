//! Hvor diffen er scrollet på skjermen akkurat nå. `DiffState` regner ut hvor scroll skal
//! ende, for eksempel etter `n`. Her flyttes det som vises dit over 170 ms, eller med en
//! gang når det ikke skal animeres.

use std::time::Instant;

use jetdiff_viewmodel::{ScrollAnimation, ScrollState};

/// Varighet på animert hopp til endring.
const NAV_ANIMATION_MS: f32 = 170.;

#[derive(Default)]
pub struct ScrollAnimator {
    /// Under en animasjon ligger den etter målet.
    shown: ScrollState,
    animation: Option<(ScrollAnimation, Instant)>,
}

impl ScrollAnimator {
    pub fn shown(&self) -> ScrollState {
        self.shown
    }

    /// Rett til målet, uten animasjon.
    pub fn jump(&mut self, target: ScrollState) {
        self.animation = None;
        self.shown = target;
    }

    /// Fra det som vises til målet. Midt i en animasjon starter den nye der den gamle står.
    pub fn animate(&mut self, target: ScrollState) {
        if self.shown == target {
            self.animation = None;
            return;
        }
        let animation = ScrollAnimation::new(self.shown, target, NAV_ANIMATION_MS);
        self.animation = Some((animation, Instant::now()));
    }

    /// Stopper en animasjon der den står. Gir posisjonen, som da må bli det nye målet.
    pub fn stop(&mut self) -> Option<ScrollState> {
        self.animation.take().map(|_| self.shown)
    }

    /// Flytter det som vises til der animasjonen er nå. `true` mens den ikke er ferdig.
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
    fn stopp_gir_posisjonen_bare_under_animasjon() {
        let mut scroll = ScrollAnimator::default();
        scroll.jump(at(100.));
        assert_eq!(scroll.stop(), None);
        scroll.animate(at(500.));
        assert_eq!(scroll.stop(), Some(at(100.)));
        assert_eq!(scroll.stop(), None);
    }

    #[test]
    fn uendret_scroll_animeres_ikke() {
        let mut scroll = ScrollAnimator::default();
        scroll.jump(at(100.));
        scroll.animate(at(100.));
        assert!(!scroll.step());
    }
}
