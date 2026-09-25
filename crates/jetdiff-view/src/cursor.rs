//! Markert linje for hele diffen, på den siden som har fokus. Siden er master for sync
//! scroll, som `getCurrentSide` i `TwosideTextDiffViewer`.

use jetdiff_core::side::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub side: Side,
    /// Logisk linje. På en kollapset fold er det første linje i folden.
    pub line: usize,
}

impl Cursor {
    pub fn new(side: Side, line: usize) -> Self {
        Self { side, line }
    }
}
