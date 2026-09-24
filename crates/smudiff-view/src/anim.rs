//! Animert scroll mot et mål. `SyncScrollSupport.doMakeVisible` animerer master og lar
//! slave følge masterens progresjon via ankeret, så begge sider lander samtidig selv om
//! avstandene er ulike. Tid er millisekunder, så modellen kan testes uten klokke.

use crate::scroll::ScrollState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollAnimation {
    from: ScrollState,
    to: ScrollState,
    duration_ms: f32,
}

impl ScrollAnimation {
    pub fn new(from: ScrollState, to: ScrollState, duration_ms: f32) -> Self {
        Self {
            from,
            to,
            duration_ms,
        }
    }

    /// Posisjon etter `elapsed_ms`, og om animasjonen er ferdig. Slave lerpes med samme
    /// progresjon som master, som ankeret i `syncVerticalScroll` gjør. Står master stille
    /// flytter IntelliJ slave etterpå, her skjer det i samme bevegelse.
    pub fn sample(&self, elapsed_ms: f32) -> (ScrollState, bool) {
        if elapsed_ms >= self.duration_ms || self.duration_ms <= 0. {
            return (self.to, true);
        }
        let p = ease_out(elapsed_ms.max(0.) / self.duration_ms);
        let lerp = |a: f32, b: f32| a + (b - a) * p;
        let state = ScrollState {
            y: [
                lerp(self.from.y[0], self.to.y[0]),
                lerp(self.from.y[1], self.to.y[1]),
            ],
            x: lerp(self.from.x, self.to.x),
        };
        (state, false)
    }
}

/// Kubisk ease-out: rask start, myk landing.
fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    1. - (1. - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(y: [f32; 2]) -> ScrollState {
        ScrollState { y, x: 0. }
    }

    #[test]
    fn starter_og_lander_eksakt() {
        let from = ScrollState {
            y: [100., 300.],
            x: 40.,
        };
        let a = ScrollAnimation::new(from, target([700., 1200.]), 160.);
        assert_eq!(a.sample(0.), (from, false));
        let (end, done) = a.sample(160.);
        assert!(done);
        assert_eq!(end.y, [700., 1200.]);
        assert_eq!(end.x, 0.);
        assert!(a.sample(1000.).1);
    }

    #[test]
    fn slave_folger_masterens_progresjon() {
        let from = ScrollState {
            y: [0., 100.],
            x: 0.,
        };
        let a = ScrollAnimation::new(from, target([1000., 200.]), 200.);
        let mut prev = -1.;
        for ms in (0..200).step_by(10) {
            let (s, done) = a.sample(ms as f32);
            assert!(!done);
            let pm = s.y[0] / 1000.;
            let ps = (s.y[1] - 100.) / 100.;
            assert!((pm - ps).abs() < 1e-5);
            assert!(pm > prev);
            prev = pm;
        }
    }

    #[test]
    fn ease_out_er_raskest_i_starten() {
        assert_eq!(ease_out(0.), 0.);
        assert_eq!(ease_out(1.), 1.);
        assert!(ease_out(0.5) > 0.8);
        assert!(ease_out(0.1) - ease_out(0.) > ease_out(1.) - ease_out(0.9));
    }

    #[test]
    fn null_varighet_er_ferdig_med_en_gang() {
        let a = ScrollAnimation::new(ScrollState::default(), target([5., 6.]), 0.);
        assert_eq!(a.sample(0.).0.y, [5., 6.]);
    }
}
