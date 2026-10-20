//! Split eller unified for samme fil. Det UI-et gjør likt i begge visningene går gjennom
//! her, så det ikke trenger å vite hvilken som vises.

use std::rc::Rc;

use crate::command::Command;
use crate::cursor::Cursor;
use crate::folding::Folding;
use crate::layout::Viewport;
use crate::model::FileDiff;
use crate::scroll::ScrollState;
use crate::state::{DiffState, Horizontal, Outcome};
use crate::unified::{UnifiedDiff, UnifiedState};

#[derive(Debug, Clone)]
pub enum DiffMode {
    Split(DiffState),
    Unified(UnifiedState),
}

impl DiffMode {
    /// `folding` brukes bare i split.
    pub fn new(diff: Rc<FileDiff>, folding: Rc<Folding>, unified: bool) -> Self {
        if unified {
            Self::Unified(UnifiedState::new(Rc::new(UnifiedDiff::new(diff))))
        } else {
            Self::Split(DiffState::new(diff, folding))
        }
    }

    /// Bytter visning med cursoren på samme linje og høyde. `folding` brukes når det
    /// byttes til split, og lages på nytt fordi unified ikke har folding.
    pub fn toggle(&mut self, folding: Rc<Folding>, viewport: Viewport) {
        *self = match self {
            Self::Split(state) => Self::Unified(UnifiedState::from_split(state, viewport)),
            Self::Unified(state) => Self::Split(state.to_split(folding, viewport)),
        };
    }

    pub fn is_unified(&self) -> bool {
        matches!(self, Self::Unified(_))
    }

    pub fn diff(&self) -> &Rc<FileDiff> {
        match self {
            Self::Split(state) => &state.diff,
            Self::Unified(state) => &state.diff.diff,
        }
    }

    /// Målet for scroll. UI-et animerer dit.
    pub fn scroll(&self) -> ScrollState {
        match self {
            Self::Split(state) => state.scroll,
            Self::Unified(state) => state.scroll,
        }
    }

    pub fn scroll_mut(&mut self) -> &mut ScrollState {
        match self {
            Self::Split(state) => &mut state.scroll,
            Self::Unified(state) => &mut state.scroll,
        }
    }

    pub fn init(&mut self, viewport: Viewport) {
        match self {
            Self::Split(state) => state.init(viewport),
            Self::Unified(state) => state.init(viewport),
        }
    }

    pub fn execute(
        &mut self,
        command: Command,
        viewport: Viewport,
        horizontal: Horizontal,
    ) -> Outcome {
        match self {
            Self::Split(state) => state.execute(command, viewport, horizontal),
            Self::Unified(state) => state.execute(command, viewport, horizontal),
        }
    }

    pub fn resize(&mut self, viewport: Viewport) {
        match self {
            Self::Split(state) => state.resize(viewport),
            Self::Unified(state) => state.resize(viewport),
        }
    }

    pub fn rescale(&mut self, old_line_height: f32, viewport: Viewport, x_scale: f32) {
        match self {
            Self::Split(state) => state.rescale(old_line_height, viewport, x_scale),
            Self::Unified(state) => state.rescale(old_line_height, viewport, x_scale),
        }
    }

    /// Markert linje i nyeste versjon, for `y`.
    pub fn location(&self) -> Cursor {
        match self {
            Self::Split(state) => state.location(),
            Self::Unified(state) => state.location(),
        }
    }

    /// Blokken cursoren står i, for rollback.
    pub fn cursor_block(&self) -> Option<usize> {
        match self {
            Self::Split(state) => state.cursor_block(),
            Self::Unified(state) => state.cursor_block(),
        }
    }

    /// Samme fil med nytt innhold. `folding` brukes bare i split.
    pub fn replace_diff(&mut self, diff: Rc<FileDiff>, folding: Rc<Folding>, viewport: Viewport) {
        match self {
            Self::Split(state) => state.replace_diff(diff, folding, viewport),
            Self::Unified(state) => state.replace_diff(diff, viewport),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jetdiff_core::ComparisonPolicy;
    use jetdiff_core::side::Side;

    const VIEWPORT: Viewport = Viewport {
        height: 200.,
        line_height: 10.,
    };

    #[test]
    fn bytte_fram_og_tilbake() {
        let old: String = (0..100).map(|i| format!("a{i}\n")).collect();
        let new = old.replace("a50\n", "x\n");
        let diff = Rc::new(FileDiff::new(&old, &new, ComparisonPolicy::Default).unwrap());
        let folding = || Rc::new(Folding::none(&diff));
        let mut mode = DiffMode::new(diff.clone(), folding(), false);
        mode.init(VIEWPORT);
        let before = mode.location();

        mode.toggle(folding(), VIEWPORT);
        assert!(mode.is_unified());
        assert_eq!(mode.location(), before);
        assert_eq!(mode.cursor_block(), Some(0));

        mode.toggle(folding(), VIEWPORT);
        assert!(!mode.is_unified());
        assert_eq!(mode.location(), Cursor::new(Side::Right, 50));
    }
}
